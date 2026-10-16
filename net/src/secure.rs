//! Security helpers shared by the lobby and the game server: secrets from the OS's random
//! source (tickets, sessions, codes, cookies must not be guessable from the ones you see),
//! and per-key packet budgets (token buckets). Encryption is in `crypto`.
use std::collections::HashMap;
use std::hash::Hash;

/// A random u64 from the OS (unpredictable, unlike any seeded generator).
pub fn random() -> u64 {
    getrandom::u64().expect("the OS random source")
}

/// Sign-in cookies: proof that an address is real (it got our answer) before we do any work
/// for it. Stateless: a keyed hash (BLAKE3) of the address and the current 10-second window;
/// the previous window still counts. Unforgeable without the secret, which the cookies don't
/// give away.
pub struct Cookies {
    secret: [u8; 32],
}

impl Cookies {
    pub fn new() -> Cookies {
        Cookies { secret: crate::crypto::new_key() }
    }

    fn at(&self, addr: std::net::SocketAddr, window: u64) -> u64 {
        let mut h = blake3::Hasher::new_keyed(&self.secret);
        h.update(addr.to_string().as_bytes());
        h.update(&window.to_le_bytes());
        u64::from_le_bytes(h.finalize().as_bytes()[..8].try_into().unwrap()) | 1
    }

    /// The cookie for `addr` now (`now`: seconds on any steady clock).
    pub fn make(&self, addr: std::net::SocketAddr, now: u64) -> u64 {
        self.at(addr, now / 10)
    }

    pub fn check(&self, addr: std::net::SocketAddr, now: u64, cookie: u64) -> bool {
        let window = now / 10;
        cookie == self.at(addr, window) || (window > 0 && cookie == self.at(addr, window - 1))
    }
}

impl Default for Cookies {
    fn default() -> Cookies {
        Cookies::new()
    }
}

/// Write a file only we can read (secrets: keys, sessions), atomically: a crash mid-write
/// never leaves half a file, and readers see the old one or the new one.
pub fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let tmp = path.with_extension("tmp");
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    std::fs::rename(tmp, path)
}

/// Packet budgets per key: each key gets `rate` per second, banking up to one second's worth.
/// Past `max_keys` new keys are refused until old ones go quiet (no memory blow-up).
pub struct Buckets<K> {
    buckets: HashMap<K, (f32, f64)>,
    max_keys: usize,
}

impl<K: Hash + Eq + Copy> Buckets<K> {
    pub fn new(max_keys: usize) -> Buckets<K> {
        Buckets { buckets: HashMap::new(), max_keys }
    }

    /// Spend one from `key`'s budget at time `now` (seconds); false = over budget.
    pub fn allow(&mut self, key: K, rate: f32, now: f64) -> bool {
        if self.buckets.len() >= self.max_keys && !self.buckets.contains_key(&key) {
            return false;
        }
        let (budget, last) = self.buckets.entry(key).or_insert((rate, now));
        *budget = (*budget + (now - *last) as f32 * rate).min(rate);
        *last = now;
        if *budget < 1.0 {
            return false;
        }
        *budget -= 1.0;
        true
    }

    /// Forget keys quiet for `secs`.
    pub fn tidy(&mut self, now: f64, secs: f64) {
        self.buckets.retain(|_, (_, last)| now - *last < secs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_and_budgets() {
        assert_ne!(random(), random());
        let mut b = Buckets::new(2);
        let hits = (0..30).filter(|i| b.allow(1, 10.0, *i as f64 * 0.01)).count();
        assert!((10..=13).contains(&hits), "{hits}: one second's budget, a little refill");
        assert!(b.allow(2, 10.0, 0.0));
        assert!(!b.allow(3, 10.0, 0.0), "table full");
        // Cookies: good for this window and the one before, not for another address, and
        // another server's (another secret) don't pass.
        let (c, other) = (Cookies::new(), Cookies::new());
        let (addr, them) = ("1.2.3.4:5".parse().unwrap(), "1.2.3.4:6".parse().unwrap());
        let k = c.make(addr, 100);
        assert!(c.check(addr, 100, k) && c.check(addr, 109, k) && c.check(addr, 115, k));
        assert!(!c.check(addr, 125, k), "two windows later: stale");
        assert!(!c.check(them, 100, k) && !other.check(addr, 100, k));
    }
}
