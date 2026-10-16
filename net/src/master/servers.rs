//! What game servers tell the lobby (sealed with their token's key): heartbeats, results,
//! reports.
use super::*;

impl Master {
    /// Write a report down for review: who, whom (Steam ids and names), why, where.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn file_report(&mut self, reporter: u64, target: u64, reason: crate::protocol::ReportReason, map: &str, round: u32, demo: &str, tick: u32) {
        let (Some(by), Some(of)) = (self.seats.get(&reporter), self.seats.get(&target)) else { return };
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        // Names have no control characters (see Hello), so Debug quoting is valid JSON here.
        let line = format!(
            "{{\"time\":{time},\"reason\":\"{reason:?}\",\"reporter\":{{\"steam\":{},\"name\":{:?}}},\"reported\":{{\"steam\":{},\"name\":{:?}}},\"map\":{map:?},\"round\":{round},\"demo\":{demo:?},\"tick\":{tick}}}",
            by.0, by.1, of.0, of.1
        );
        println!("report: {line}");
        if let Some(path) = &self.reports {
            use std::io::Write;
            match std::fs::OpenOptions::new().create(true).append(true).open(path) {
                Ok(mut f) => _ = writeln!(f, "{line}"),
                Err(e) => println!("can't write reports to {}: {e}", path.display()),
            }
        }
    }
}

impl Master {
    /// A sealed message from a game server: only official ones (holding a token) can make
    /// one, and an old one replayed is ignored.
    pub(super) fn from_server(&mut self, from: SocketAddr, packet: &[u8]) {
        let Some(id) = crate::crypto::server_key_id(packet) else { return };
        let Some(key) = self.keys.iter().position(|k| k.id == id) else { return };
        let Some((sent, body)) = self.keys[key].open(packet) else { return };
        match decode::<LobbyMsg>(&body) {
            Some(LobbyMsg::Heartbeat { busy, address, region, deathmatch, players, max, .. }) => {
                if self.servers.get(&from).is_some_and(|s| sent < s.sent) || (!self.servers.contains_key(&from) && self.servers.len() >= MAX_SERVERS) {
                    return;
                }
                let region: String = region.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(16).collect();
                let public = address.parse().unwrap_or(from);
                if !self.servers.contains_key(&from) {
                    println!("server {public} ({region}) joined the pool");
                }
                if busy {
                    self.booking.remove(&from); // took the match
                }
                // Booked, not confirmed yet: still busy as far as we're concerned.
                let busy = busy || self.booking.contains_key(&from);
                let seated = self.servers.get(&from).map(|s| s.seated.clone()).unwrap_or_default();
                self.servers.insert(from, GameServer { public, region, key, official: true, busy, seen: Instant::now(), sent, deathmatch, players, max, seated });
            }
            Some(LobbyMsg::Result { winners, losers, abandoned }) if self.role == Role::Active => self.match_result(&winners, &losers, &abandoned),
            Some(LobbyMsg::Report { reporter, target, reason, map, round, demo, tick }) if self.role == Role::Active => self.file_report(reporter, target, reason, &map, round, &demo, tick),
            _ => {}
        }
    }
}
