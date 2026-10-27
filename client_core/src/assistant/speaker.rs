//! When the assistant may talk (the anti-Navi rules). Pure logic.

/// What kind of line it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Greetings, tips, banter: never while you're alive in a live round.
    Personal,
    /// Team news (a teammate down, your streak): allowed in rounds, rate-limited.
    Relay,
    /// Bomb planted or defused, last alive: allowed in rounds, still not mid-fight.
    Critical,
}

/// The situation right now.
#[derive(Clone, Copy, Debug)]
pub struct Now {
    pub t: f32,
    /// A round is being played (not warmup, freeze time or round end).
    pub live: bool,
    pub alive: bool,
    /// Settings: talk during live rounds at all.
    pub in_round: bool,
    /// Settings: 0 quiet, 1 normal, 2 chatty.
    pub chattiness: u8,
}

/// Seconds of silence after you shoot, hit or get hit.
const COMBAT_HOLD: f32 = 3.0;
/// No two lines closer than this (a line is shown for 4 s).
const MIN_GAP: f32 = 5.0;

#[derive(Default)]
pub struct Speaker {
    combat_until: f32,
    last_line: Option<f32>,
    last_relay: Option<f32>,
    round_lines: u32,
}

impl Speaker {
    /// You fired, hit someone or took damage.
    pub fn combat(&mut self, t: f32) {
        self.combat_until = t + COMBAT_HOLD;
    }

    pub fn new_round(&mut self) {
        self.round_lines = 0;
    }

    /// May a line of this kind be said now? Says yes at most once per call site, and
    /// counts it: a refused line is dropped, never queued.
    pub fn allow(&mut self, kind: Kind, now: Now) -> bool {
        if now.t < self.combat_until {
            return false;
        }
        let since = |at: Option<f32>| at.map_or(f32::INFINITY, |a| now.t - a);
        let in_round = now.live && now.alive;
        if in_round {
            if !now.in_round || kind == Kind::Personal {
                return false;
            }
            let c = now.chattiness.min(2) as usize;
            // Per round: quiet 2, normal 5, chatty 8. Relay spacing: 8, 4, 3 s.
            if self.round_lines >= [2, 5, 8][c] {
                return false;
            }
            if kind == Kind::Relay && since(self.last_relay) < [8.0, 4.0, 3.0][c] {
                return false;
            }
        }
        if kind != Kind::Critical && since(self.last_line) < MIN_GAP {
            return false;
        }
        self.last_line = Some(now.t);
        if in_round {
            self.round_lines += 1;
            if kind == Kind::Relay {
                self.last_relay = Some(now.t);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(t: f32, live: bool, alive: bool) -> Now {
        Now { t, live, alive, in_round: true, chattiness: 1 }
    }

    #[test]
    fn silent_in_combat_even_for_critical() {
        let mut s = Speaker::default();
        s.combat(10.0);
        assert!(!s.allow(Kind::Critical, at(12.0, true, true)));
        assert!(s.allow(Kind::Critical, at(13.5, true, true)));
    }

    #[test]
    fn personal_waits_until_you_are_dead_or_round_over() {
        let mut s = Speaker::default();
        assert!(!s.allow(Kind::Personal, at(0.0, true, true)));
        assert!(s.allow(Kind::Personal, at(0.0, true, false)));
        assert!(s.allow(Kind::Personal, at(10.0, false, true)));
    }

    #[test]
    fn min_gap_between_lines() {
        let mut s = Speaker::default();
        assert!(s.allow(Kind::Personal, at(0.0, false, true)));
        assert!(!s.allow(Kind::Personal, at(3.0, false, true)));
        assert!(s.allow(Kind::Personal, at(5.5, false, true)));
    }

    #[test]
    fn round_cap_and_relay_spacing() {
        let mut s = Speaker::default();
        let mut said = 0;
        for i in 0..40 {
            if s.allow(Kind::Relay, at(i as f32 * 6.0, true, true)) {
                said += 1;
            }
        }
        assert_eq!(said, 5, "normal chattiness: 5 per round");
        s.new_round();
        assert!(s.allow(Kind::Relay, at(1000.0, true, true)));
    }

    #[test]
    fn in_round_off_blocks_everything_while_playing() {
        let mut s = Speaker::default();
        let now = Now { in_round: false, ..at(0.0, true, true) };
        assert!(!s.allow(Kind::Critical, now));
        assert!(s.allow(Kind::Personal, Now { alive: false, ..now }));
    }
}
