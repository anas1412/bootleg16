//! Protection against floods and spoofing, before any real work is done on a packet:
//! oversized packets are dropped, every IP gets a packet budget (small until it's a player),
//! and joining needs a cookie only a real address can echo back, so spoofed addresses can't
//! take player slots or memory. (Big volumetric floods must be filtered by the host.)
use super::*;
use std::net::IpAddr;

/// Bigger datagrams are never ours (a snapshot-sized input packet is a few hundred bytes).
pub(super) const MAX_PACKET: usize = 1400;
/// Packets per second per IP: players (inputs at 128/s, voice, a little spare) / everyone else.
const PLAYER_RATE: f32 = 600.0;
const STRANGER_RATE: f32 = 20.0;
/// Players from one IP (a LAN party behind one router is fine, a flood of slots isn't).
pub(super) const MAX_PER_IP: usize = 6;
/// IPs remembered at once; past this, strangers are ignored until the table drains.
const MAX_TRACKED: usize = 8192;

pub(super) struct Guard {
    secret: u64,
    /// IP -> (budget left, last tick it was topped up).
    buckets: HashMap<IpAddr, (f32, u32)>,
}

impl Guard {
    pub(super) fn new() -> Guard {
        // Unpredictable enough for cookies: start time and process id, mixed.
        let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        Guard { secret: mix(t ^ ((std::process::id() as u64) << 32)), buckets: HashMap::new() }
    }

    /// May this packet be looked at? `player`: the address is already in the game.
    pub(super) fn allow(&mut self, from: SocketAddr, len: usize, player: bool, tick: u32) -> bool {
        if len > MAX_PACKET {
            return false;
        }
        let rate = if player { PLAYER_RATE } else { STRANGER_RATE };
        if !player && self.buckets.len() >= MAX_TRACKED && !self.buckets.contains_key(&from.ip()) {
            return false;
        }
        let (budget, last) = self.buckets.entry(from.ip()).or_insert((rate, tick));
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
