//! Matchmaking: filling two teams from queued parties and booking a free official server,
//! close to the players. Each player measures their ping to every region; a party may play
//! in a region where all of them have a good ping, and "good" widens the longer they wait.
use super::*;
use crate::server::MatchSetup;

/// The worst ping a party accepts at first, how much it widens per second of waiting, and
/// the most it ever widens to.
const PING_START: f32 = 60.0;
const PING_WIDEN_PER_SECOND: f32 = 2.0;
const PING_MAX: f32 = 200.0;

impl Master {
    /// The ping a party accepts after waiting this long.
    fn ping_limit(waited: Duration) -> f32 {
        (PING_START + waited.as_secs_f32() * PING_WIDEN_PER_SECOND).min(PING_MAX)
    }

    /// The worst ping of these players to a region (None: someone hasn't measured it, or
    /// can't reach it; anyone who measured nothing at all goes anywhere).
    fn worst_ping(&self, members: &[u32], region: &str) -> Option<f32> {
        let mut worst = 0.0f32;
        for m in members {
            let p = self.players.values().find(|p| p.id == *m)?;
            if p.pings.is_empty() {
                continue;
            }
            worst = worst.max(*p.pings.get(region)? as f32);
        }
        Some(worst)
    }

    /// Regions with a free official server, best ping for these players first.
    fn regions_for(&self, members: &[u32]) -> Vec<(String, f32)> {
        let mut regions: Vec<(String, f32)> = Vec::new();
        for s in self.servers.values().filter(|s| s.official && !s.busy) {
            if regions.iter().any(|(r, _)| *r == s.region) {
                continue;
            }
            if let Some(ping) = self.worst_ping(members, &s.region) {
                regions.push((s.region.clone(), ping));
            }
        }
        regions.sort_by(|a, b| a.1.total_cmp(&b.1));
        regions
    }

    /// Reserve a free official server in `region` (any region: None) for these players and
    /// hand out tickets. False if none.
    pub(super) fn start_match(&mut self, players: Vec<(u32, Group)>, knife_round: bool, region: Option<&str>) -> bool {
        let free = self.servers.iter().find(|(_, s)| s.official && !s.busy && region.is_none_or(|r| s.region == r));
        let Some((addr, public, key, region)) = free.map(|(a, s)| (*a, s.public, s.key, s.region.clone())) else { return false };
        // A seat number and a fresh key per player: the server learns the keys from this
        // (sealed) booking, the player from the lobby (encrypted); they never travel in clear.
        let tickets: Vec<(u32, u64, Group, [u8; 32])> = players.iter().map(|(id, g)| (*id, self.rand() | 1, *g, crate::crypto::new_key())).collect();
        let setup = MatchSetup { players: tickets.iter().map(|(_, t, g, _)| (*t, *g)).collect(), knife_round, keys: tickets.iter().map(|(_, t, _, k)| (*t, *k)).collect() };
        // Sealed with the server's token key: only it can read it, nobody can forge one.
        let msg = self.keys[key].seal(&encode(&setup));
        let _ = self.udp.send_to(&msg, addr);
        self.booking.insert(addr, (msg, Instant::now(), Instant::now()));
        self.servers.get_mut(&addr).unwrap().busy = true;
        println!("match booked on {public} ({region}): {} players{}", players.len(), if knife_round { ", custom" } else { "" });
        for (id, ticket, _, k) in tickets {
            if let Some(p) = self.players.values_mut().find(|p| p.id == id) {
                p.found = Some((public, ticket, k));
                self.seats.insert(ticket, (p.steam, p.name.clone(), Instant::now()));
            }
            if let Some(party) = self.parties.values_mut().find(|q| q.members.iter().any(|(m, _)| *m == id)) {
                party.queued = None;
            }
        }
        true
    }

    /// A custom game: the best region for the whole party, else any free server.
    pub(super) fn start_custom(&mut self, members: Vec<(u32, Group)>) -> bool {
        let ids: Vec<u32> = members.iter().map(|(m, _)| *m).collect();
        let best = self.regions_for(&ids).into_iter().next().map(|(r, _)| r);
        self.start_match(members.clone(), true, best.as_deref()) || self.start_match(members, true, None)
    }

    /// Casual: per region, fill two teams from the parties that accept it (each party stays on
    /// one team), oldest first.
    pub(super) fn matchmake(&mut self) {
        let side = self.match_size / 2;
        let mut queued: Vec<(Instant, Vec<u32>)> = self.parties.values().filter_map(|p| Some((p.queued?, p.members.iter().map(|(m, _)| *m).collect()))).collect();
        queued.sort_by_key(|q| q.0);
        let mut regions: Vec<String> = self.servers.values().filter(|s| s.official && !s.busy).map(|s| s.region.clone()).collect();
        regions.sort();
        regions.dedup();
        for region in regions {
            let (mut a, mut b): (Vec<u32>, Vec<u32>) = (Vec::new(), Vec::new());
            for (since, members) in &queued {
                let ok = self.worst_ping(members, &region).is_some_and(|p| p <= Self::ping_limit(since.elapsed()));
                if !ok {
                    continue;
                }
                if a.len() + members.len() <= side {
                    a.extend(members);
                } else if b.len() + members.len() <= side {
                    b.extend(members);
                }
                if a.len() == side && b.len() == side {
                    let players: Vec<(u32, Group)> = a.iter().map(|m| (*m, Group::A)).chain(b.iter().map(|m| (*m, Group::B))).collect();
                    self.start_match(players, false, Some(&region));
                    return; // ponytail: one match per step; plenty at our scale
                }
            }
        }
    }

    /// One official server per region for players to ping.
    pub(super) fn ping_targets(&self) -> Vec<(String, SocketAddr)> {
        let mut out: Vec<(String, SocketAddr)> = Vec::new();
        for s in self.servers.values().filter(|s| s.official) {
            if !out.iter().any(|(r, _)| *r == s.region) {
                out.push((s.region.clone(), s.public));
            }
        }
        out.sort();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ping_limit_widens_with_waiting() {
        assert_eq!(Master::ping_limit(Duration::ZERO), 60.0);
        assert_eq!(Master::ping_limit(Duration::from_secs(30)), 120.0);
        assert_eq!(Master::ping_limit(Duration::from_secs(600)), 200.0);
    }
}
