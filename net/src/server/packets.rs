//! Reading what arrived: the lobby's sealed bookings, then (within each address's budget)
//! players' packets, encrypted on official servers, handed to `handle`.
use super::*;

impl Server {
    pub(super) fn read_packets(&mut self) {
        let tick = self.tick;
        for (from, bytes) in self.socket.recv_all() {
            // The lobby's bookings have their own way in, so strangers can't crowd them out
            // (its address can be faked; the key it seals them with can't).
            if bytes.first() == Some(&crate::crypto::SERVER) {
                if self.lobbies.contains(&from) && bytes.len() <= guard::MAX_PACKET {
                    self.booking(&bytes);
                }
                continue;
            }
            // Budget first: floods and junk cost nothing past this line.
            let players = if self.clients.contains_key(&from) { self.clients.keys().filter(|a| a.ip() == from.ip()).count() } else { 0 };
            if !self.guard.allow(from, bytes.len(), players, tick) {
                continue;
            }
            let plain = if bytes.first() == Some(&crate::crypto::CHANNEL) {
                match self.unseal(from, &bytes) {
                    Some(p) => p,
                    None => continue,
                }
            } else if self.official() {
                // Official servers talk encrypted only; in the clear: just region pings.
                if let Some(msg @ ClientMsg::Info { .. }) = decode::<ClientMsg>(&bytes) {
                    self.handle(from, msg);
                }
                continue;
            } else {
                bytes
            };
            if let Some(msg) = decode::<ClientMsg>(&plain) {
                self.handle(from, msg);
            }
            self.joining = None;
        }
        if tick % TICK_RATE == 0 {
            let clients: std::collections::HashSet<SocketAddr> = self.clients.keys().copied().collect();
            self.socket.keep_channels(|a| clients.contains(a));
            // Deathmatch seats: kept while their player is here, else for a minute.
            let here: std::collections::HashSet<u64> = self.clients.values().map(|c| c.ticket).collect();
            self.seats.retain(|t, (_, at)| here.contains(t) || tick - *at < 60 * TICK_RATE);
        }
    }
}
