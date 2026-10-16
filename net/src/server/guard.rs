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
    cookies: crate::secure::Cookies,
    /// Budgets per IP, separate for players and strangers: players can't starve joins, and a
    /// flood of made-up addresses (filling the strangers' table) can't touch players.
    players: crate::secure::Buckets<IpAddr>,
    strangers: crate::secure::Buckets<IpAddr>,
}

impl Guard {
    pub(super) fn new() -> Guard {
        // From the OS random source: nobody can work out the cookies (and fake addresses).
        Guard { cookies: crate::secure::Cookies::new(), players: crate::secure::Buckets::new(usize::MAX), strangers: crate::secure::Buckets::new(MAX_TRACKED) }
    }

    /// May this packet be looked at? `players`: how many players are in the game from this
    /// IP (0 = a stranger's packet: the address isn't in the game).
    pub(super) fn allow(&mut self, from: SocketAddr, len: usize, players: usize, tick: u32) -> bool {
        if len > MAX_PACKET {
            return false;
        }
        let now = tick as f64 / TICK_RATE as f64;
        if players > 0 {
            self.players.allow(from.ip(), PLAYER_RATE * players as f32, now)
        } else {
            self.strangers.allow(from.ip(), STRANGER_RATE, now)
        }
    }

    /// Forget IPs that have been quiet for a few seconds.
    pub(super) fn tidy(&mut self, tick: u32) {
        let now = tick as f64 / TICK_RATE as f64;
        self.players.tidy(now, 5.0);
        self.strangers.tidy(now, 5.0);
    }

    /// The join cookie for an address now (stateless: nothing is stored).
    pub(super) fn cookie(&self, addr: SocketAddr, tick: u32) -> u64 {
        self.cookies.make(addr, (tick / TICK_RATE) as u64)
    }

    pub(super) fn cookie_ok(&self, addr: SocketAddr, tick: u32, cookie: u64) -> bool {
        self.cookies.check(addr, (tick / TICK_RATE) as u64, cookie)
    }
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
