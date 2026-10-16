//! The game's side of the lobby.
use super::*;

/// The game's side of the lobby: call `tick` every frame; it signs in, sends commands, polls
/// every second, and keeps the latest state. Never blocks.
pub struct LobbyClient {
    udp: UdpSocket,
    lobby: SocketAddr,
    name: String,
    session: Option<u64>,
    last_sent: Option<Instant>,
    pub state: Option<LobbyState>,
    /// Kept until taken: the match to join (server, ticket).
    pub found: Option<(SocketAddr, u64, [u8; 32])>,
    /// Kept until taken: the lobby's last message for the player.
    pub notice: String,
    /// Makes a Steam login for each sign-in (None = anonymous).
    pub steam: Option<Box<dyn FnMut() -> Option<SteamLogin> + Send + Sync>>,
    /// The login for the sign-in in progress (a Steam ticket is used once).
    login: Option<SteamLogin>,
    /// The lobby's public key (built into release builds; None: ask the lobby, for tests and
    /// development only).
    pub lobby_key: Option<[u8; 32]>,
    /// The key is built in (never replaced by what the network says).
    pub builtin_key: bool,
    /// The sign-in in progress (our half of the key exchange and its sealed hello), the
    /// lobby's cookie for our address, and the session channel once signed in.
    hello: Option<(crate::crypto::ClientHello, Vec<u8>)>,
    cookie: u64,
    channel: Option<crate::crypto::Channel>,
    /// The last command, until the lobby answers (every command gets a state back): resent
    /// so a lost packet can't lose a click. (command, sent at, tries)
    unanswered: Option<(Command, Instant, u8)>,
    /// Hellos sent since we last heard the lobby (a key learned from the lobby itself is
    /// forgotten after a few: it may have restarted with a new one).
    unheard: u32,
    /// Ping measuring: when it last started, and the servers still to answer (address,
    /// region, sent at) / the answers so far.
    pinged: Option<Instant>,
    pinging: Vec<(SocketAddr, String, Instant)>,
    pings: Vec<(String, u16)>,
}

/// Pings are measured again this often (and whenever the regions change).
const PING_EVERY: Duration = Duration::from_secs(60);
/// Servers that haven't answered by then count as unreachable.
const PING_WAIT: Duration = Duration::from_secs(2);

impl LobbyClient {
    /// `lobby_key`: set it after this for builds with the lobby's key built in (`builtin_key`).
    pub fn new(lobby: SocketAddr, name: &str) -> std::io::Result<LobbyClient> {
        let udp = UdpSocket::bind(if lobby.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })?;
        udp.set_nonblocking(true)?;
        Ok(LobbyClient { udp, lobby, name: name.into(), session: None, last_sent: None, state: None, found: None, notice: String::new(), steam: None, login: None, lobby_key: None, builtin_key: false, hello: None, cookie: 0, channel: None, unanswered: None, unheard: 0, pinged: None, pinging: Vec::new(), pings: Vec::new() })
    }

    /// Signed in and heard from the lobby.
    pub fn online(&self) -> bool {
        self.state.is_some()
    }

    /// Send a command now (the answer is the new state).
    pub fn send(&mut self, cmd: Command) {
        if let (Some(session), Some(ch)) = (self.session, &mut self.channel) {
            if cmd != Command::Poll {
                let tries = self.unanswered.as_ref().filter(|u| u.0 == cmd).map(|u| u.2 + 1).unwrap_or(0);
                self.unanswered = Some((cmd.clone(), Instant::now(), tries));
            }
            let sealed = ch.seal(session, &encode(&LobbyMsg::Command { cmd, pad: vec![0; PAD] }));
            let _ = self.udp.send_to(&sealed, self.lobby);
            self.last_sent = Some(Instant::now());
        }
    }

    /// Start over: a new key exchange (and Steam ticket) at the next sign-in.
    fn signed_out(&mut self) {
        self.session = None;
        self.channel = None;
        self.hello = None;
        self.login = None;
    }

    /// Say hello (with our cookie, once we have it), or first ask for the lobby's key.
    fn say_hello(&mut self) {
        self.last_sent = Some(Instant::now());
        self.unheard += 1;
        if self.unheard > 4 && !self.builtin_key {
            self.lobby_key = None;
            self.hello = None;
            self.unheard = 0;
        }
        let Some(key) = self.lobby_key else {
            let _ = self.udp.send_to(&encode(&LobbyMsg::KeyPlease { pad: vec![0; 64] }), self.lobby);
            return;
        };
        if self.hello.is_none() {
            if self.login.is_none() {
                self.login = self.steam.as_mut().and_then(|f| f());
            }
            let sign_in = SignIn { name: self.name.clone(), steam: self.login.clone(), pad: vec![0; PAD] };
            self.hello = Some(crate::crypto::client_hello(&key, &encode(&sign_in)));
        }
        let (h, sealed) = self.hello.as_ref().unwrap();
        let _ = self.udp.send_to(&encode(&LobbyMsg::Hello { cookie: self.cookie, public: h.public, sealed: sealed.clone() }), self.lobby);
    }

    pub fn tick(&mut self) {
        let mut buf = [0u8; 2048];
        while let Ok((n, from)) = self.udp.recv_from(&mut buf) {
            if from != self.lobby {
                // A game server answering our ping.
                if let Some(crate::protocol::ServerMsg::Info { .. }) = decode(&buf[..n]) {
                    if let Some(i) = self.pinging.iter().position(|(a, _, _)| *a == from) {
                        let (_, region, sent) = self.pinging.swap_remove(i);
                        self.pings.push((region, sent.elapsed().as_millis().min(999) as u16));
                    }
                }
                continue;
            }
            // In our session: only what our channel opens counts.
            let msg = if buf[0] == crate::crypto::CHANNEL {
                match self.channel.as_mut().and_then(|ch| ch.open(&buf[..n])) {
                    Some(plain) => decode::<LobbyMsg>(&plain),
                    None => continue,
                }
            } else {
                decode::<LobbyMsg>(&buf[..n])
            };
            if msg.is_some() {
                self.unheard = 0;
            }
            match msg {
                Some(LobbyMsg::Key { public }) if self.lobby_key.is_none() => {
                    self.lobby_key = Some(public);
                    self.say_hello();
                }
                Some(LobbyMsg::Cookie { cookie }) if self.session.is_none() => {
                    self.cookie = cookie;
                    self.say_hello();
                }
                Some(LobbyMsg::Welcome { public, sealed }) if self.session.is_none() => {
                    // Finish the key exchange; the session number comes over the new channel.
                    let Some((h, _)) = &self.hello else { continue };
                    let mut ch = h.finish(&public);
                    let Some(session) = ch.open(&sealed).and_then(|b| b.try_into().ok()).map(u64::from_le_bytes) else { continue };
                    self.channel = Some(ch);
                    self.session = Some(session);
                    self.hello = None;
                    self.login = None;
                    self.send(Command::Poll);
                }
                Some(LobbyMsg::Refused { reason }) if self.session.is_none() => {
                    // Try again in a while (Steam may have been started meanwhile).
                    self.notice = reason;
                    self.signed_out();
                    self.last_sent = Some(Instant::now() + Duration::from_secs(9));
                }
                Some(LobbyMsg::Expired) if self.session.is_some() => {
                    // Sign in again right away (a new key exchange and Steam ticket too).
                    self.signed_out();
                    self.last_sent = None;
                }
                Some(LobbyMsg::State(s)) => {
                    self.unanswered = None;
                    let regions_changed = self.state.as_ref().is_none_or(|old| old.regions != s.regions);
                    if regions_changed || self.pinged.is_none_or(|t| t.elapsed() > PING_EVERY) {
                        self.ping(&s.regions);
                    }
                    if s.found.is_some() {
                        self.found = s.found;
                    }
                    if !s.notice.is_empty() {
                        self.notice = s.notice.clone();
                    }
                    self.state = Some(s);
                }
                _ => {}
            }
        }
        // Done measuring (everyone answered, or the rest are unreachable): tell the lobby.
        if self.pinged.is_some_and(|t| t.elapsed() > PING_WAIT || self.pinging.is_empty()) && !self.pings.is_empty() {
            self.pinging.clear();
            let pings = std::mem::take(&mut self.pings);
            self.send(Command::Pings(pings));
        }
        if let Some((cmd, at, tries)) = self.unanswered.clone() {
            if at.elapsed() > Duration::from_millis(400) && tries < 4 {
                self.send(cmd);
            }
        }
        if self.last_sent.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) {
            match self.session {
                Some(_) => self.send(Command::Poll),
                None => self.say_hello(),
            }
        }
    }

    /// Start measuring our ping to one server per region.
    fn ping(&mut self, regions: &[(String, SocketAddr)]) {
        self.pinged = Some(Instant::now());
        self.pings.clear();
        self.pinging.clear();
        for (region, addr) in regions {
            let _ = self.udp.send_to(&encode(&crate::protocol::ClientMsg::Info { pad: vec![0; crate::protocol::INFO_PAD] }), addr);
            self.pinging.push((*addr, region.clone(), Instant::now()));
        }
    }
}
