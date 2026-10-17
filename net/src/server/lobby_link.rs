//! Telling the lobbies we're here (and whether we're busy), and finding them by name.
use super::*;

impl Server {
    /// Start looking up the lobbies' names (no-op when they're all addresses: parsed directly).
    pub(super) fn look_up_lobby(&mut self) {
        let list = self.round.cfg.lobby.clone();
        if list.split(',').all(|a| a.trim().is_empty() || a.trim().parse::<SocketAddr>().is_ok()) {
            return;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || _ = tx.send(crate::config::addresses(&list, true)));
        self.lookup = Some(rx);
    }

    /// Announce this server to the lobby (if one is set), so it can be given matches.
    pub(super) fn heartbeat(&mut self) {
        let cfg = &self.round.cfg;
        if cfg.lobby.is_empty() {
            return;
        }
        // A name (not an address): look it up again now and then, in the background.
        if let Some(found) = self.lookup.as_ref().and_then(|rx| rx.try_recv().ok()) {
            if !found.is_empty() {
                self.lobbies = found;
            }
            self.lookup = None;
        }
        if self.lookup.is_none() && self.tick % (60 * self.rate.hz()) < self.rate.secs(HEARTBEAT_SECS) {
            self.look_up_lobby();
        }
        let cfg = &self.round.cfg;
        let players = self.clients.values().filter(|c| !c.bot).count() as u8;
        let busy = self.reserved.is_some() || self.draining.is_some();
        let msg = crate::lobby::LobbyMsg::Heartbeat {
            name: cfg.name.clone(),
            map: self.map.clone(),
            players,
            max: cfg.max_players,
            region: cfg.region.clone(),
            busy,
            address: cfg.public_address.clone(),
            deathmatch: self.rules.deathmatch,
            maps: self.maps(),
        };
        // Sealed with the token's key (the token itself never goes out): only the lobby can
        // read it, and nobody can pretend to be us.
        if let Some(key) = &self.server_key {
            let sealed = key.seal(&encode(&msg));
            for lobby in self.lobbies.clone() {
                self.socket.send(lobby, sealed.clone());
            }
        }
    }
}
