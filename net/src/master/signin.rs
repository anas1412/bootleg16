//! Signing players in: the sign-in cookie, the encrypted handshake (see `crypto`), Steam
//! checks, and the session packets that follow.
use super::*;

impl Master {
    /// Our public key (build it into the game: `BOOTLEG16_LOBBY_KEY`).
    pub fn public_key(&self) -> [u8; 32] {
        self.key.public()
    }

    /// Use this long-term key (kept in a file, so the built-in public key stays valid).
    pub fn set_key(&mut self, key: crate::crypto::LobbyKey) {
        self.key = key;
    }

    /// A sign-in: check the cookie (cheap) before the key exchange (not cheap), then read
    /// who it is and start their session.
    pub(super) fn hello(&mut self, from: SocketAddr, cookie: u64, public: [u8; 32], sealed: &[u8]) {
        let now = self.started.elapsed().as_secs();
        if !self.cookies.check(from, now, cookie) {
            let _ = self.udp.send_to(&encode(&LobbyMsg::Cookie { cookie: self.cookies.make(from, now) }), from);
            return;
        }
        let Some((payload, fresh, channel)) = crate::crypto::lobby_hello(&self.key, &public, sealed) else { return };
        let Some(SignIn { name, steam, pad }) = decode::<SignIn>(&payload) else { return };
        if pad.len() < PAD {
            return;
        }
        let refuse = |m: &Master, reason: &str| _ = m.udp.send_to(&encode(&LobbyMsg::Refused { reason: reason.into() }), from);
        if steam.is_none() && self.require_steam {
            return refuse(self, "Start Steam to play online.");
        }
        if self.players.len() >= MAX_PLAYERS || (!from.ip().is_loopback() && self.players.values().filter(|p| p.addr.ip() == from.ip()).count() >= SESSIONS_PER_IP) {
            return refuse(self, "Too many players from your address, or the lobby is full. Try again later.");
        }
        let session = self.rand();
        let id = self.next_id;
        self.next_id += 1;
        let name: String = name.chars().filter(|c| !c.is_control()).take(24).collect();
        let party = self.new_party(id);
        self.players.insert(session, Player { channel, id, name, steam: 0, addr: from, party, seen: Instant::now(), found: None, notice: String::new(), invites: Vec::new(), pings: HashMap::new(), maps: None });
        if let Some(login) = steam {
            match self.steam_mode {
                // Unchecked: never let a claim push anyone else out.
                SteamMode::Trust if self.players.values().any(|p| p.steam == login.steam_id) => {}
                SteamMode::Trust => self.steam_checked(login.steam_id, Some(session)),
                SteamMode::Verify => self.steam_checks.push((session, login.steam_id, login.ticket)),
            }
        }
        let p = self.players.get_mut(&session).unwrap();
        let sealed = p.channel.seal(session, &session.to_le_bytes());
        let _ = self.udp.send_to(&encode(&LobbyMsg::Welcome { public: fresh, sealed }), from);
    }

    /// A packet in a player's session: their command, answered with their state. The key
    /// proves who it is, so a player whose address changed (a phone between networks) carries on.
    pub(super) fn from_player(&mut self, from: SocketAddr, packet: &[u8]) {
        let Some(session) = crate::crypto::channel_id(packet) else { return };
        let Some(p) = self.players.get_mut(&session) else {
            let _ = self.udp.send_to(&encode(&LobbyMsg::Expired), from);
            return;
        };
        let Some(plain) = p.channel.open(packet) else { return };
        let Some(LobbyMsg::Command { cmd, pad }) = decode::<LobbyMsg>(&plain) else { return };
        if pad.len() < PAD {
            return;
        }
        p.addr = from;
        self.command(session, cmd);
        if let Some(state) = self.state_of(session) {
            let p = self.players.get_mut(&session).unwrap();
            let sealed = p.channel.seal(session, &encode(&LobbyMsg::State(state)));
            let _ = self.udp.send_to(&sealed, from);
        }
    }

    /// Steam logins to check (`SteamMode::Verify`): (session, Steam id, ticket).
    pub fn take_steam_checks(&mut self) -> Vec<(u64, u64, Vec<u8>)> {
        std::mem::take(&mut self.steam_checks)
    }

    /// Steam's answer for a login: `session` is who passed (None = the ticket was bad).
    /// Signing in somewhere else ends your older session.
    pub fn steam_checked(&mut self, steam_id: u64, session: Option<u64>) {
        let Some(session) = session.filter(|s| self.players.contains_key(s)) else { return };
        let older: Vec<(u32, u32)> = self.players.iter().filter(|(s, p)| p.steam == steam_id && **s != session).map(|(_, p)| (p.id, p.party)).collect();
        for (id, party) in older {
            self.leave(id, party);
            self.players.retain(|_, p| p.id != id);
        }
        self.players.get_mut(&session).unwrap().steam = steam_id;
    }

    /// A Steam login was refused: the player stays signed in without Steam, and is told.
    pub fn steam_refused(&mut self, session: u64) {
        if self.require_steam {
            let Some(p) = self.players.remove(&session) else { return };
            self.leave(p.id, p.party);
            let _ = self.udp.send_to(&encode(&LobbyMsg::Refused { reason: "Steam couldn't confirm your sign-in. Restart Steam and try again.".into() }), p.addr);
        } else if let Some(p) = self.players.get_mut(&session) {
            p.notice = "Steam sign-in failed: playing without Steam.".into();
        }
    }
}
