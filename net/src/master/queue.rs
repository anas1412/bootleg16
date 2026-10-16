//! Booking servers close to the players (the matching itself is in `matchmaker`). Each player measures their ping to every region; a party may play
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
    pub(super) fn ping_limit(waited: Duration) -> f32 {
        (PING_START + waited.as_secs_f32() * PING_WIDEN_PER_SECOND).min(PING_MAX)
    }

    /// The worst ping of these players to a region (None: someone hasn't measured it, or
    /// can't reach it; anyone who measured nothing at all goes anywhere).
    pub(super) fn worst_ping(&self, members: &[u32], region: &str) -> Option<f32> {
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
        for s in self.servers.values().filter(|s| s.official && !s.busy && !s.deathmatch) {
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
    /// The pool maps one player can play (what their game reported; not told: the stock ones).
    fn maps_of(&self, player: u32) -> Vec<String> {
        match self.players.values().find(|p| p.id == player).and_then(|p| p.maps.clone()) {
            Some(m) => m,
            None => crate::config::STOCK_MAPS.iter().map(|m| m.to_string()).collect(),
        }
    }

    /// The pool maps all these players have.
    pub(super) fn common_maps(&self, players: &[u32]) -> Vec<String> {
        let mut maps: Vec<String> = crate::config::MAP_POOL.iter().map(|m| m.to_string()).collect();
        for p in players {
            let theirs = self.maps_of(*p);
            maps.retain(|m| theirs.contains(m));
        }
        maps
    }

    /// `map`: this one (a custom game's pick or veto), or None: a random one from the pool,
    /// always one every player and the server have.
    pub(super) fn start_match(&mut self, players: Vec<(u32, Group)>, knife_round: bool, region: Option<&str>, map: Option<&str>) -> bool {
        let ids: Vec<u32> = players.iter().map(|(p, _)| *p).collect();
        let common = self.common_maps(&ids);
        let fits = |s: &GameServer| match map {
            Some(m) => s.maps.iter().any(|x| x == m) || s.maps.is_empty(),
            None => s.maps.is_empty() || s.maps.iter().any(|x| common.contains(x)),
        };
        let free = self.servers.iter().find(|(_, s)| s.official && !s.busy && !s.deathmatch && region.is_none_or(|r| s.region == r) && fits(s));
        let Some((addr, public, key, region, maps)) = free.map(|(a, s)| (*a, s.public, s.key, s.region.clone(), s.maps.clone())) else { return false };
        let playable: Vec<String> = maps.into_iter().filter(|m| common.contains(m)).collect();
        let map = match map {
            Some(m) => m.to_string(),
            None if playable.is_empty() => String::new(),
            None => playable[(self.rand() % playable.len() as u64) as usize].clone(),
        };
        // A seat number and a fresh key per player: the server learns the keys from this
        // (sealed) booking, the player from the lobby (encrypted); they never travel in clear.
        let tickets: Vec<(u32, u64, Group, [u8; 32])> = players.iter().map(|(id, g)| (*id, self.rand() | 1, *g, crate::crypto::new_key())).collect();
        let setup = MatchSetup { players: tickets.iter().map(|(_, t, g, _)| (*t, *g)).collect(), knife_round, keys: tickets.iter().map(|(_, t, _, k)| (*t, *k)).collect(), map: map.clone() };
        // Sealed with the server's token key: only it can read it, nobody can forge one.
        let msg = self.keys[key].seal(&encode(&LobbyOrder::Book(setup)));
        let _ = self.udp.send_to(&msg, addr);
        self.booking.insert(addr, (msg, Instant::now(), Instant::now()));
        self.servers.get_mut(&addr).unwrap().busy = true;
        println!("match booked on {public} ({region}, {map}): {} players{}", players.len(), if knife_round { ", custom" } else { "" });
        for (id, ticket, _, k) in tickets {
            if let Some(p) = self.players.values_mut().find(|p| p.id == id) {
                p.found = Some((public, ticket, k));
                let who = if p.steam != 0 { p.steam } else { 1 << 63 | p.id as u64 };
                self.active.insert(who, (public, ticket, k));
                self.seats.insert(ticket, (p.steam, p.name.clone(), Instant::now()));
            }
            if let Some(party) = self.parties.values_mut().find(|q| q.members.iter().any(|(m, _)| *m == id)) {
                party.queued = None;
            }
        }
        true
    }

    /// A seat on the deathmatch server with room where this player pings best: sealed to the
    /// server, the ticket and key to the player (as `found`). False if they're all full.
    pub(super) fn deathmatch_seat(&mut self, player: u32) -> bool {
        for s in self.servers.values_mut() {
            s.seated.retain(|t| t.elapsed() < Duration::from_secs(20));
        }
        // Room on it (players, plus seats given out but not taken yet), best ping first.
        let pick = self
            .servers
            .iter()
            .filter(|(_, s)| s.official && s.deathmatch && (s.players as usize + s.seated.len()) < s.max as usize)
            .map(|(a, s)| (*a, self.worst_ping(&[player], &s.region).unwrap_or(f32::MAX)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(a, _)| a);
        let Some(addr) = pick else { return false };
        let (ticket, key) = (self.rand() | 1, crate::crypto::new_key());
        let s = self.servers.get_mut(&addr).unwrap();
        s.seated.push(Instant::now());
        let (public, k) = (s.public, s.key);
        let order = self.keys[k].seal(&encode(&LobbyOrder::Seat { ticket, key }));
        let _ = self.udp.send_to(&order, addr);
        if let Some(p) = self.players.values_mut().find(|p| p.id == player) {
            p.found = Some((public, ticket, key));
            self.seats.insert(ticket, (p.steam, p.name.clone(), Instant::now()));
        }
        true
    }

    /// A custom game: the best region for the whole party, else any free server.
    pub(super) fn start_custom(&mut self, members: Vec<(u32, Group)>, map: Option<&str>) -> bool {
        let ids: Vec<u32> = members.iter().map(|(m, _)| *m).collect();
        let best = self.regions_for(&ids).into_iter().next().map(|(r, _)| r);
        self.start_match(members.clone(), true, best.as_deref(), map) || self.start_match(members, true, None, map)
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
