//! The encrypted side of an official server: bookings sealed by the lobby (with the key both
//! derive from the official token), and players' channels, opened with the key the lobby gave
//! their ticket. Nothing reaches the game unless it proved it came from the right key.
use super::*;
use crate::crypto::Channel;

impl Server {
    /// A sealed booking from the lobby: take it if genuine, fresh and we're free.
    pub(super) fn booking(&mut self, packet: &[u8]) {
        let Some(key) = &self.server_key else { return };
        let Some((sent, body)) = key.open(packet) else { return };
        let Some(setup) = decode::<MatchSetup>(&body) else { return };
        // An older booking replayed (or a resend of one we took) changes nothing.
        if sent <= self.last_booking || self.reserved.is_some() || self.draining.is_some() {
            return;
        }
        self.last_booking = sent;
        self.reserve(setup);
        self.heartbeat(); // tell the lobby right away that we took it
    }

    /// Decrypt a player's packet: over their channel, or (a new player) with the key of the
    /// ticket it names. A new channel is kept only once their join is accepted (see Hello).
    pub(super) fn unseal(&mut self, from: SocketAddr, packet: &[u8]) -> Option<Vec<u8>> {
        if self.socket.is_secure(from) {
            return self.socket.open(from, packet);
        }
        let ticket = crate::crypto::channel_id(packet)?;
        let key = self.reserved.as_ref()?.setup.keys.iter().find(|(t, _)| *t == ticket)?.1;
        let mut ch = Channel::new(&key, false);
        let plain = ch.open(packet)?;
        self.joining = Some((from, ticket, ch));
        Some(plain)
    }
}
