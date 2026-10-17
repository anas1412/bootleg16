//! The server's tick rate: how many simulation steps it runs per second.
//!
//! 128 is the default. A server runs at 64 or 128 and tells clients in the
//! Welcome message. Time-based values are written in seconds and converted
//! with `TickRate::secs`, so they mean the same thing at either rate.

/// Rates a server may run at.
pub const TICK_RATES: [u32; 2] = [64, 128];
pub const DEFAULT_TICK_RATE: u32 = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickRate(u32);

impl TickRate {
    /// `None` unless `hz` is one of `TICK_RATES`.
    pub fn new(hz: u32) -> Option<Self> {
        TICK_RATES.contains(&hz).then_some(Self(hz))
    }

    /// Steps per second.
    pub fn hz(self) -> u32 {
        self.0
    }

    /// Length of one step, in seconds.
    pub fn tick(self) -> f32 {
        1.0 / self.0 as f32
    }

    /// Whole ticks in `seconds`, rounded to the nearest tick.
    pub fn secs(self, seconds: f32) -> u32 {
        (seconds * self.0 as f32).round() as u32
    }
}

impl Default for TickRate {
    fn default() -> Self {
        Self(DEFAULT_TICK_RATE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_128() {
        assert_eq!(TickRate::default().hz(), 128);
    }

    #[test]
    fn only_64_and_128_are_allowed() {
        assert!(TickRate::new(64).is_some());
        assert!(TickRate::new(128).is_some());
        assert!(TickRate::new(100).is_none());
        assert!(TickRate::new(0).is_none());
    }

    #[test]
    fn seconds_match_the_old_constants_at_128() {
        let r = TickRate::default();
        assert_eq!(r.secs(5.0), 5 * 128); // TIMEOUT_TICKS
        assert_eq!(r.secs(0.5), 128 / 2); // MAX_UNLAG_TICKS
        assert_eq!(r.secs(0.25), 128 / 4);
    }

    #[test]
    fn seconds_scale_with_the_rate() {
        let r = TickRate::new(64).unwrap();
        assert_eq!(r.secs(5.0), 320);
        assert_eq!(r.secs(0.5), 32);
    }

    #[test]
    fn tick_is_one_over_hz() {
        assert!((TickRate::new(64).unwrap().tick() - 1.0 / 64.0).abs() < 1e-7);
        assert!((TickRate::default().tick() - 1.0 / 128.0).abs() < 1e-7);
    }
}
