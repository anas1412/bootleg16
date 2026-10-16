//! Player reports in a reserved match: checked here (both in this match, not yourself, once
//! per player, a few per match) and passed to the lobby, which knows who everyone is (their
//! Steam ids) and files them for review.
use super::*;
use crate::protocol::ReportReason;

/// Reports one player may make in a match.
const MAX_REPORTS: usize = 5;

impl Server {
    pub(super) fn report(&mut self, from: SocketAddr, player: u8, reason: ReportReason) {
        let Some(reporter) = self.clients.get(&from).map(|c| c.ticket) else { return };
        let Some(target) = self.clients.values().find(|c| c.id == player).map(|c| c.ticket) else { return };
        let Some(r) = &mut self.reserved else { return };
        let mine = r.reports.iter().filter(|(by, _)| *by == reporter).count();
        if reporter == 0 || target == 0 || reporter == target || mine >= MAX_REPORTS || r.reports.contains(&(reporter, target)) {
            return;
        }
        r.reports.push((reporter, target));
        let cfg = &self.round.cfg;
        let Some(lobby) = self.lobby else { return };
        let msg = crate::master::LobbyMsg::Report { token: cfg.official_token.clone(), reporter, target, reason, map: self.map.clone(), round: self.round.round };
        self.socket.send(lobby, encode(&msg));
    }
}
