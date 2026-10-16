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
    pub found: Option<(SocketAddr, u64)>,
    /// Kept until taken: the lobby's last message for the player.
    pub notice: String,
    /// Makes a Steam login for each sign-in (None = anonymous).
    pub steam: Option<Box<dyn FnMut() -> Option<SteamLogin> + Send + Sync>>,
    /// The login for the sign-in in progress (a Steam ticket is used once).
    login: Option<SteamLogin>,
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
    pub fn new(lobby: SocketAddr, name: &str) -> std::io::Result<LobbyClient> {
        let udp = UdpSocket::bind(if lobby.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })?;
        udp.set_nonblocking(true)?;
        Ok(LobbyClient { udp, lobby, name: name.into(), session: None, last_sent: None, state: None, found: None, notice: String::new(), steam: None, login: None, pinged: None, pinging: Vec::new(), pings: Vec::new() })
    }

    /// Signed in and heard from the lobby.
    pub fn online(&self) -> bool {
        self.state.is_some()
    }

    /// Send a command now (the answer is the new state).
    pub fn send(&mut self, cmd: Command) {
        if let Some(session) = self.session {
            let _ = self.udp.send_to(&encode(&LobbyMsg::Command { session, cmd, pad: vec![0; PAD] }), self.lobby);
            self.last_sent = Some(Instant::now());
        }
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
            match decode::<LobbyMsg>(&buf[..n]) {
                Some(LobbyMsg::Welcome { session }) if self.session.is_none() => {
                    self.session = Some(session);
                    self.login = None;
                    self.send(Command::Poll);
                }
                Some(LobbyMsg::Refused { reason }) => {
                    // Try again in a while (Steam may have been started meanwhile).
                    self.notice = reason;
                    self.session = None;
                    self.login = None;
                    self.last_sent = Some(Instant::now() + Duration::from_secs(9));
                }
                Some(LobbyMsg::Expired) => {
                    // Sign in again right away (a new Steam ticket too).
                    self.session = None;
                    self.last_sent = None;
                }
                Some(LobbyMsg::State(s)) => {
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
        if self.last_sent.is_none_or(|t| t.elapsed() > Duration::from_secs(1)) {
            match self.session {
                Some(_) => self.send(Command::Poll),
                None => {
                    if self.login.is_none() {
                        self.login = self.steam.as_mut().and_then(|f| f());
                    }
                    let _ = self.udp.send_to(&encode(&LobbyMsg::Hello { name: self.name.clone(), steam: self.login.clone(), pad: vec![0; PAD] }), self.lobby);
                    self.last_sent = Some(Instant::now());
                }
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
