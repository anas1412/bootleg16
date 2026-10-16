//! Encryption for everything that crosses the internet, built from audited parts:
//! X25519 key exchange, ChaCha20-Poly1305 authenticated encryption, BLAKE3 key derivation.
//!
//! - `Channel`: a two-way encrypted session (player <-> lobby, player <-> game server). Each
//!   direction has its own key and a counter as nonce; a replay window drops old or repeated
//!   packets. Packets: `[CHANNEL][id: u64][counter: u64][ciphertext + tag]`, where `id` says
//!   whose key to use (a session or ticket number: useless without the key).
//! - `ServerKey`: game server <-> lobby messages, keyed by the shared official token (which is
//!   never sent). Random nonces, and a timestamp inside so old messages can't be replayed.
//! - `LobbyKey` + `client_hello` / `lobby_hello`: the player <-> lobby handshake. The game
//!   knows the lobby's public key (built in), so only the real lobby can answer; both sides
//!   add a fresh key each time (forward secrecy).
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce, XChaCha20Poly1305, XNonce};
use x25519_dalek::{PublicKey, StaticSecret};

/// First byte of a `Channel` packet / a `ServerKey` packet (plain postcard messages never
/// start with these: their first byte is a small variant number).
pub const CHANNEL: u8 = 0xE1;
pub const SERVER: u8 = 0xE2;
/// Bytes a channel adds to a message.
pub const OVERHEAD: usize = 1 + 8 + 8 + 16;

fn random_bytes() -> [u8; 32] {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).expect("the OS random source");
    b
}

/// A fresh random 32-byte key (a match key for one player).
pub fn new_key() -> [u8; 32] {
    random_bytes()
}

fn cipher(key: &[u8; 32]) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new(key.into())
}

fn nonce(counter: u64) -> Nonce {
    let mut n = [0u8; 12];
    n[..8].copy_from_slice(&counter.to_le_bytes());
    n.into()
}

/// Which counters have been seen: the highest, and a bitmap of the 64 below it.
#[derive(Default)]
struct Window {
    top: u64,
    seen: u64,
}

impl Window {
    /// New (and not too old)? Then remember it.
    fn accept(&mut self, n: u64) -> bool {
        if n > self.top || (self.top == 0 && self.seen == 0) {
            let shift = n - self.top;
            self.seen = if shift >= 64 { 0 } else { self.seen << shift };
            self.seen |= 1;
            self.top = n;
            return true;
        }
        let back = self.top - n;
        if back >= 64 || self.seen & (1 << back) != 0 {
            return false;
        }
        self.seen |= 1 << back;
        true
    }
}

/// One side of an encrypted session.
pub struct Channel {
    shared: [u8; 32],
    initiator: bool,
    send: ChaCha20Poly1305,
    recv: ChaCha20Poly1305,
    sent: u64,
    window: Window,
}

impl Channel {
    /// From a shared secret; `initiator`: the side that started (player), so the two
    /// directions get different keys.
    pub fn new(shared: &[u8; 32], initiator: bool) -> Channel {
        let up = blake3::derive_key("bootleg16 2026 channel up", shared);
        let down = blake3::derive_key("bootleg16 2026 channel down", shared);
        let (send, recv) = if initiator { (up, down) } else { (down, up) };
        Channel { shared: *shared, initiator, send: cipher(&send), recv: cipher(&recv), sent: 0, window: Window::default() }
    }

    /// Everything needed to carry the session on elsewhere (a restarted or standby lobby).
    pub fn export(&self) -> ChannelState {
        ChannelState { shared: self.shared, initiator: self.initiator, sent: self.sent, top: self.window.top, seen: self.window.seen }
    }

    /// Carry on a saved session. Its counter jumps far ahead: messages sent after the save
    /// must never have their counter (nonce) used again with this key.
    pub fn import(s: &ChannelState) -> Channel {
        let mut c = Channel::new(&s.shared, s.initiator);
        c.sent = s.sent + RESTORE_GAP;
        c.window = Window { top: s.top, seen: s.seen };
        c
    }

    pub fn seal(&mut self, id: u64, plain: &[u8]) -> Vec<u8> {
        self.sent += 1;
        let mut out = Vec::with_capacity(plain.len() + OVERHEAD);
        out.push(CHANNEL);
        out.extend_from_slice(&id.to_le_bytes());
        out.extend_from_slice(&self.sent.to_le_bytes());
        out.extend(self.send.encrypt(&nonce(self.sent), plain).expect("encrypt"));
        out
    }

    /// The message, if the packet is genuine and new.
    pub fn open(&mut self, packet: &[u8]) -> Option<Vec<u8>> {
        if packet.len() < OVERHEAD || packet[0] != CHANNEL {
            return None;
        }
        let counter = u64::from_le_bytes(packet[9..17].try_into().ok()?);
        let plain = self.recv.decrypt(&nonce(counter), &packet[17..]).ok()?;
        // Only after it proved genuine: a forged packet can't move the window.
        self.window.accept(counter).then_some(plain)
    }
}

/// How far a restored channel's counter jumps (far more than it could send between saves).
const RESTORE_GAP: u64 = 1 << 24;

/// A channel saved (secret: keep it as safe as the keys themselves).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub struct ChannelState {
    shared: [u8; 32],
    initiator: bool,
    sent: u64,
    top: u64,
    seen: u64,
}

/// The `id` of a channel packet (which key it needs).
pub fn channel_id(packet: &[u8]) -> Option<u64> {
    (packet.len() >= OVERHEAD && packet[0] == CHANNEL).then(|| u64::from_le_bytes(packet[1..9].try_into().unwrap()))
}

/// The game server <-> lobby key, from the official token.
pub struct ServerKey {
    cipher: XChaCha20Poly1305,
    /// Says which token, without giving it away.
    pub id: [u8; 8],
}

/// Unix time in seconds.
pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Messages older than this are refused (clocks are NTP-synced; replays are older).
pub const MAX_AGE: u64 = 30;

impl ServerKey {
    pub fn from_token(token: &str) -> ServerKey {
        let key = blake3::derive_key("bootleg16 2026 server key", token.as_bytes());
        let id = blake3::derive_key("bootleg16 2026 server key id", token.as_bytes());
        ServerKey { cipher: XChaCha20Poly1305::new((&key).into()), id: id[..8].try_into().unwrap() }
    }

    /// `[SERVER][id][nonce: 24][ciphertext of (time, message)]`.
    pub fn seal(&self, plain: &[u8]) -> Vec<u8> {
        let n: [u8; 24] = random_bytes()[..24].try_into().unwrap();
        let mut body = now().to_le_bytes().to_vec();
        body.extend_from_slice(plain);
        let mut out = vec![SERVER];
        out.extend_from_slice(&self.id);
        out.extend_from_slice(&n);
        out.extend(self.cipher.encrypt(XNonce::from_slice(&n), body.as_slice()).expect("encrypt"));
        out
    }

    /// The message and when it was sent, if genuine and recent.
    pub fn open(&self, packet: &[u8]) -> Option<(u64, Vec<u8>)> {
        if packet.len() < 1 + 8 + 24 + 16 + 8 || packet[0] != SERVER || packet[1..9] != self.id {
            return None;
        }
        let body = self.cipher.decrypt(XNonce::from_slice(&packet[9..33]), &packet[33..]).ok()?;
        let sent = u64::from_le_bytes(body[..8].try_into().ok()?);
        (now().abs_diff(sent) <= MAX_AGE).then(|| (sent, body[8..].to_vec()))
    }
}

/// The token id a server packet claims (to pick the key).
pub fn server_key_id(packet: &[u8]) -> Option<[u8; 8]> {
    (packet.len() > 9 && packet[0] == SERVER).then(|| packet[1..9].try_into().unwrap())
}

/// The lobby's long-term key pair. Its public half is built into the game.
pub struct LobbyKey(StaticSecret);

impl LobbyKey {
    pub fn generate() -> LobbyKey {
        LobbyKey(StaticSecret::from(random_bytes()))
    }

    pub fn from_hex(hex: &str) -> Option<LobbyKey> {
        Some(LobbyKey(StaticSecret::from(from_hex(hex)?)))
    }

    pub fn to_hex(&self) -> String {
        to_hex(&self.0.to_bytes())
    }

    pub fn public(&self) -> [u8; 32] {
        PublicKey::from(&self.0).to_bytes()
    }
}

pub fn to_hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn from_hex(s: &str) -> Option<[u8; 32]> {
    let s = s.trim();
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        *o = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// The player's half of the handshake in progress.
pub struct ClientHello {
    secret: StaticSecret,
    first: [u8; 32],
    /// Our fresh public key (sent in the hello).
    pub public: [u8; 32],
}

/// Start a handshake with the lobby whose public key is `lobby`: (state, the hello's sealed
/// payload). Only the real lobby can read it.
pub fn client_hello(lobby: &[u8; 32], payload: &[u8]) -> (ClientHello, Vec<u8>) {
    let secret = StaticSecret::from(random_bytes());
    let public = PublicKey::from(&secret).to_bytes();
    let first = *secret.diffie_hellman(&PublicKey::from(*lobby)).as_bytes();
    let k = blake3::derive_key("bootleg16 2026 hello", &first);
    let sealed = cipher(&k).encrypt(&nonce(0), payload).expect("encrypt");
    (ClientHello { secret, first, public }, sealed)
}

/// The lobby's side: read a hello; answer with our fresh public key. Returns (payload,
/// our fresh public key, the session channel).
pub fn lobby_hello(key: &LobbyKey, their: &[u8; 32], sealed: &[u8]) -> Option<(Vec<u8>, [u8; 32], Channel)> {
    let their = PublicKey::from(*their);
    let first = *key.0.diffie_hellman(&their).as_bytes();
    let k = blake3::derive_key("bootleg16 2026 hello", &first);
    let payload = cipher(&k).decrypt(&nonce(0), sealed).ok()?;
    let fresh = StaticSecret::from(random_bytes());
    let second = *fresh.diffie_hellman(&their).as_bytes();
    let session = session_key(&first, &second);
    Some((payload, PublicKey::from(&fresh).to_bytes(), Channel::new(&session, false)))
}

impl ClientHello {
    /// The lobby answered with its fresh public key: our session channel.
    pub fn finish(&self, lobby_fresh: &[u8; 32]) -> Channel {
        let second = *self.secret.diffie_hellman(&PublicKey::from(*lobby_fresh)).as_bytes();
        Channel::new(&session_key(&self.first, &second), true)
    }
}

fn session_key(first: &[u8; 32], second: &[u8; 32]) -> [u8; 32] {
    let mut both = first.to_vec();
    both.extend_from_slice(second);
    blake3::derive_key("bootleg16 2026 session", &both)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_work_one_way_and_refuse_forgeries_and_replays() {
        let k = new_key();
        let (mut a, mut b) = (Channel::new(&k, true), Channel::new(&k, false));
        let p = a.seal(7, b"hello");
        assert_eq!(channel_id(&p), Some(7));
        assert_eq!(b.open(&p).as_deref(), Some(&b"hello"[..]));
        assert_eq!(b.open(&p), None, "replayed");
        let mut bad = a.seal(7, b"x");
        *bad.last_mut().unwrap() ^= 1;
        assert_eq!(b.open(&bad), None, "tampered");
        assert_eq!(a.open(&b.seal(7, b"back")).as_deref(), Some(&b"back"[..]));
        // Out of order within the window is fine; a stranger's key isn't.
        let (p1, p2) = (a.seal(7, b"1"), a.seal(7, b"2"));
        assert!(b.open(&p2).is_some() && b.open(&p1).is_some());
        assert_eq!(Channel::new(&new_key(), false).open(&a.seal(7, b"z")), None);
        // Saved and restored (a lobby restart): carries on, never reusing a counter.
        let before = b.sent;
        let mut b2 = Channel::import(&b.export());
        assert!(b2.sent > before + 1000);
        assert_eq!(a.open(&b2.seal(7, b"again")).as_deref(), Some(&b"again"[..]));
        assert!(b2.open(&a.seal(7, b"on")).is_some());
    }

    #[test]
    fn handshake_only_the_real_lobby_can_answer() {
        let lobby = LobbyKey::generate();
        let (hello, sealed) = client_hello(&lobby.public(), b"name");
        let (payload, fresh, mut l) = lobby_hello(&lobby, &hello.public, &sealed).unwrap();
        assert_eq!(payload, b"name");
        let mut c = hello.finish(&fresh);
        assert_eq!(l.open(&c.seal(1, b"cmd")).as_deref(), Some(&b"cmd"[..]));
        assert!(lobby_hello(&LobbyKey::generate(), &hello.public, &sealed).is_none(), "an impostor can't read it");
        assert_eq!(LobbyKey::from_hex(&lobby.to_hex()).unwrap().public(), lobby.public());
    }

    #[test]
    fn server_messages_need_the_token_and_go_stale() {
        let (k, wrong) = (ServerKey::from_token("secret"), ServerKey::from_token("guess"));
        let p = k.seal(b"heartbeat");
        assert_eq!(k.open(&p).map(|(_, m)| m).as_deref(), Some(&b"heartbeat"[..]));
        assert!(wrong.open(&p).is_none());
        assert!(!p.windows(6).any(|w| w == b"secret"), "the token itself is never sent");
    }
}
