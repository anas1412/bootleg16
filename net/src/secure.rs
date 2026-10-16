//! Security helpers shared by the lobby and the game server: secrets from the OS's random
//! source (tickets, sessions, codes, cookies must not be guessable from the ones you see),
//! and per-key packet budgets (token buckets). Encryption is in `crypto`.
use std::collections::HashMap;
use std::hash::Hash;

/// A random u64 from the OS (unpredictable, unlike any seeded generator).
pub fn random() -> u64 {
    getrandom::u64().expect("the OS random source")
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
    }
}
