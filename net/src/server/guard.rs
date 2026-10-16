//! Protection against floods and spoofing, before any real work is done on a packet:
//! oversized packets are dropped, every IP gets a packet budget (small until it's a player),
//! and joining needs a cookie only a real address can echo back, so spoofed addresses can't
//! take player slots or memory. (Big volumetric floods must be filtered by the host.)
use super::*;
use std::net::IpAddr;

/// Bigger datagrams are never ours (a snapshot-sized input packet is a few hundred bytes).
pub(super) const MAX_PACKET: usize = 1400;
/// Packets per second: per player (inputs at 128/s, voice, a little spare; an IP with
/// several players gets that much per player) / per IP for everyone else.
const PLAYER_RATE: f32 = 600.0;
const STRANGER_RATE: f32 = 20.0;
/// Players from one IP (a LAN party behind one router is fine, a flood of slots isn't).
/// Loopback is exempt: it can't be spoofed from outside, and tests run many players from it.
pub(super) const MAX_PER_IP: usize = 6;
/// IPs remembered at once; past this, strangers are ignored until the table drains.
const MAX_TRACKED: usize = 8192;

pub(super) struct Guard {
    secret: u64,
    /// (IP, from a player?) -> (budget left, last tick it was topped up). Players and
    /// strangers on one IP have separate budgets, so players can't starve joins and the
    /// other way round.
    buckets: HashMap<(IpAddr, bool), (f32, u32)>,
}

impl Guard {
    pub(super) fn new() -> Guard {
        // Unpredictable enough for cookies: start time and process id, mixed.
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        Guard { secret: mix(t ^ ((std::process::id() as u64) << 32)), buckets: HashMap::new() }
    }

    /// May this packet be looked at? `players`: how many players are in the game from this
    /// IP (0 = a stranger's packet: the address isn't in the game).
    pub(super) fn allow(&mut self, from: SocketAddr, len: usize, players: usize, tick: u32) -> bool {
        if len > MAX_PACKET {
            return false;
        }
        let player = players > 0;
        let rate = if player { PLAYER_RATE * players as f32 } else { STRANGER_RATE };
        let key = (from.ip(), player);
        if !player && self.buckets.len() >= MAX_TRACKED && !self.buckets.contains_key(&key) {
            return false;
        }
        let (budget, last) = self.buckets.entry(key).or_insert((rate, tick));
        *budget = (*budget + (tick - *last) as f32 * rate / TICK_RATE as f32).min(rate);
        *last = tick;
        if *budget < 1.0 {
            return false;
        }
        *budget -= 1.0;
        true
    }

    /// Forget IPs that have been quiet for a few seconds.
    pub(super) fn tidy(&mut self, tick: u32) {
        self.buckets.retain(|_, (_, last)| tick - *last < 5 * TICK_RATE);
    }

    /// The join cookie for an address in a 10-second window (stateless: nothing is stored).
    pub(super) fn cookie(&self, addr: SocketAddr, tick: u32, previous: bool) -> u64 {
        let window = (tick / (10 * TICK_RATE)) as u64 - previous as u64;
        let a = match addr.ip() {
            IpAddr::V4(v) => u32::from(v) as u64,
            IpAddr::V6(v) => (u128::from(v) as u64) ^ ((u128::from(v) >> 64) as u64),
        };
        mix(self.secret ^ mix(a ^ ((addr.port() as u64) << 48) ^ window.rotate_left(17)))
    }

    pub(super) fn cookie_ok(&self, addr: SocketAddr, tick: u32, cookie: u64) -> bool {
        cookie != 0 && (cookie == self.cookie(addr, tick, false) || (tick >= 10 * TICK_RATE && cookie == self.cookie(addr, tick, true)))
    }
}

/// splitmix64 finaliser: a cheap, well-mixed 64-bit hash step.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn players_behind_one_ip_each_get_a_budget_and_joins_still_get_through() {
        let mut g = Guard::new();
        let ip = |port| SocketAddr::from(([10, 0, 0, 1], port));
        // Five players behind one router, 128 inputs a second each, for a second.
        let mut dropped = 0;
        for tick in 0..TICK_RATE {
            for p in 0..5 {
                if !g.allow(ip(1000 + p), 100, 5, tick) {
                    dropped += 1;
                }
            }
        }
        assert_eq!(dropped, 0);
        // A friend joining from the same router isn't starved by them.
        assert!(g.allow(ip(2000), 100, 0, TICK_RATE));
    }
}
