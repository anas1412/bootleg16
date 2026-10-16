//! The match: round phases, score, halftime and overtime, and the money each round pays.
//! Pure rules (no players or networking), so they can be tested on their own; the server
//! applies what this decides (`server/round.rs`).
use crate::config::{LossBonus, MatchConfig};
use crate::protocol::Team;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Waiting for both teams to have someone: respawns, free buying.
    Warmup,
    /// Round start: nobody can move or shoot; buying is open.
    Freeze,
    Live,
    /// The round is decided; the next starts after a short delay.
    Over,
    GameOver,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    Eliminated,
    BombExploded,
    BombDefused,
    /// Time ran out with no bomb planted.
    TimeUp,
}

/// What happens when the gap after a round runs out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Next {
    /// Start the next round; `swap`: teams change sides first; `money`: everyone's money is
    /// set to this (halftime, overtime) and weapons are taken away.
    Round { swap: bool, money: Option<i32> },
    /// Someone won (None = a draw, when overtime is off).
    GameOver(Option<Team>),
}

/// Seconds the final scoreboard shows before the match starts over.
const GAME_OVER_SECONDS: f32 = 15.0;

pub struct Match {
    pub cfg: MatchConfig,
    pub tick_rate: u32,
    pub phase: Phase,
    /// Tick the current phase ends: freeze → live, live → time up, over → next round.
    pub phase_end: u32,
    /// Tick buying closes.
    pub buy_end: u32,
    /// Current round number (1-based).
    pub round: u32,
    /// Rounds won: terrorists, counter-terrorists (as the sides are now; swapped at halftime).
    pub score: [u32; 2],
    /// Losses in a row per side, and the shared CS 1.6 bonus.
    losses: [u32; 2],
    shared_bonus: i32,
    pub last: Option<(Team, EndReason)>,
}

pub fn side(team: Team) -> usize {
    if team == Team::Ct {
        1
    } else {
        0
    }
}

pub fn other(team: Team) -> Team {
    if team == Team::Ct {
        Team::T
    } else {
        Team::Ct
    }
}

impl Match {
    pub fn new(cfg: MatchConfig, tick_rate: u32) -> Match {
        Match { cfg, tick_rate, phase: Phase::Warmup, phase_end: 0, buy_end: 0, round: 0, score: [0; 2], losses: [0; 2], shared_bonus: 1400, last: None }
    }

    fn ticks(&self, seconds: f32) -> u32 {
        (seconds * self.tick_rate as f32).round() as u32
    }

    /// Begin the match from round 1 (scores and loss streaks cleared).
    pub fn start(&mut self, tick: u32) {
        *self = Match::new(self.cfg.clone(), self.tick_rate);
        self.begin_round(tick);
    }

    /// Freeze time of the next round.
    pub fn begin_round(&mut self, tick: u32) {
        self.round += 1;
        self.phase = Phase::Freeze;
        self.phase_end = tick + self.ticks(self.cfg.freeze_time);
        self.buy_end = self.phase_end + self.ticks(self.cfg.buy_time);
        self.last = None;
    }

    /// Freeze time over: the round clock runs.
    pub fn go_live(&mut self, tick: u32) {
        self.phase = Phase::Live;
        self.phase_end = tick + self.ticks(self.cfg.round_time);
    }

    pub fn can_buy(&self, tick: u32) -> bool {
        match self.phase {
            Phase::Warmup => true,
            Phase::Freeze | Phase::Live => tick < self.buy_end,
            _ => false,
        }
    }

    /// The round is decided. Returns each side's payout [T, CT]; kill rewards are separate.
    pub fn end_round(&mut self, tick: u32, winner: Team, reason: EndReason) -> [i32; 2] {
        let (w, l) = (side(winner), side(other(winner)));
        self.score[w] += 1;
        self.last = Some((winner, reason));
        self.phase = Phase::Over;
        self.phase_end = tick + self.ticks(self.cfg.round_end_delay);
        // docs/RULES.md 3.2 (bomb maps).
        let win = match reason {
            EndReason::BombExploded => 3500,
            _ => 3250,
        };
        let bonus = match self.cfg.loss_bonus {
            LossBonus::PerTeam => {
                self.losses[w] = 0;
                self.losses[l] += 1;
                (1400 + 500 * (self.losses[l] as i32 - 1)).min(3400)
            }
            LossBonus::Cs16 => {
                if self.losses[w] > 1 {
                    self.shared_bonus = 1500; // a streak was broken
                }
                self.losses[w] = 0;
                self.losses[l] += 1;
                if self.losses[l] > 1 && self.shared_bonus <= 2999 {
                    self.shared_bonus += 500;
                }
                self.shared_bonus
            }
        };
        // Terrorists who planted get $800 more when the bomb is defused.
        let planted = if reason == EndReason::BombDefused { 800 } else { 0 };
        let mut pay = [0; 2];
        pay[w] = win;
        pay[l] = bonus + planted;
        pay
    }

    /// The gap after a round is over: what comes next.
    pub fn next(&mut self, tick: u32) -> Next {
        let total = self.score[0] + self.score[1];
        let reg = self.cfg.max_rounds;
        let half = reg / 2;
        let ot = self.cfg.overtime_rounds;
        let best = self.score[0].max(self.score[1]);
        let leader = if self.score[0] > self.score[1] { Team::T } else { Team::Ct };
        // Rounds needed to win: 13 of 24, then 4 of each 6-round overtime on top of the tie.
        let target = if total <= reg {
            half + 1
        } else {
            let n = (total - reg).div_ceil(ot.max(1));
            half + n * ot / 2 + 1
        };
        if best >= target {
            return self.game_over(tick, Some(leader));
        }
        if total == reg && ot == 0 {
            return self.game_over(tick, None);
        }
        let mut swap = false;
        let mut money = None;
        if total == half {
            swap = true;
            money = Some(self.cfg.start_money);
        } else if total >= reg && ot > 0 {
            let into = (total - reg) % ot;
            if into == 0 {
                money = Some(self.cfg.overtime_money); // an overtime begins
            } else if into == ot / 2 {
                swap = true;
                money = Some(self.cfg.overtime_money);
            }
        }
        if swap {
            self.score.swap(0, 1);
        }
        if money.is_some() {
            self.losses = [0; 2];
            self.shared_bonus = 1400;
        }
        self.begin_round(tick);
        Next::Round { swap, money }
    }

    /// The match ends now (a surrender or a forfeit).
    pub fn forfeit(&mut self, tick: u32, winner: Team) {
        self.game_over(tick, Some(winner));
    }

    fn game_over(&mut self, tick: u32, winner: Option<Team>) -> Next {
        self.phase = Phase::GameOver;
        self.phase_end = tick + self.ticks(GAME_OVER_SECONDS);
        Next::GameOver(winner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m() -> Match {
        let mut m = Match::new(MatchConfig::default(), 1);
        m.start(0);
        m
    }

    #[test]
    fn halftime_swaps_and_resets_money_then_13_wins() {
        let mut m = m();
        for _ in 0..11 {
            m.end_round(0, Team::T, EndReason::Eliminated);
            assert_eq!(m.next(0), Next::Round { swap: false, money: None });
        }
        m.end_round(0, Team::T, EndReason::Eliminated);
        assert_eq!(m.next(0), Next::Round { swap: true, money: Some(800) });
        assert_eq!(m.score, [0, 12], "the old T team is CT now");
        m.end_round(0, Team::Ct, EndReason::TimeUp);
        assert_eq!(m.next(0), Next::GameOver(Some(Team::Ct)));
    }

    #[test]
    fn overtime_at_12_all_with_10k_and_side_swaps() {
        let mut m = m();
        for i in 0..24 {
            let w = if (i < 12) == (i % 2 == 0) { Team::T } else { Team::Ct };
            m.end_round(0, w, EndReason::Eliminated);
            let n = m.next(0);
            if i == 23 {
                assert_eq!(m.score[0] + m.score[1], 24);
                assert_eq!(n, Next::Round { swap: false, money: Some(10_000) }, "{:?}", m.score);
            }
        }
        assert_eq!(m.score, [12, 12]);
        // OT: 3 rounds, swap, then first to 16.
        for _ in 0..2 {
            m.end_round(0, Team::T, EndReason::Eliminated);
            m.next(0);
        }
        m.end_round(0, Team::Ct, EndReason::Eliminated);
        assert_eq!(m.next(0), Next::Round { swap: true, money: Some(10_000) });
        assert_eq!(m.score, [13, 14]);
        m.end_round(0, Team::Ct, EndReason::Eliminated);
        m.next(0);
        m.end_round(0, Team::Ct, EndReason::Eliminated);
        assert_eq!(m.next(0), Next::GameOver(Some(Team::Ct)), "16-13");
    }

    #[test]
    fn loss_bonus_ladders() {
        let mut m = m();
        let pays: Vec<i32> = (0..6).map(|_| m.end_round(0, Team::Ct, EndReason::Eliminated)[0]).collect();
        assert_eq!(pays, [1400, 1900, 2400, 2900, 3400, 3400]);
        assert_eq!(m.end_round(0, Team::T, EndReason::BombExploded), [3500, 1400]);
        assert_eq!(m.end_round(0, Team::Ct, EndReason::BombDefused), [1400 + 800, 3250], "the ladder restarted after the win");

        let mut m = Match::new(MatchConfig { loss_bonus: LossBonus::Cs16, ..Default::default() }, 1);
        m.start(0);
        let pays: Vec<i32> = (0..5).map(|_| m.end_round(0, Team::Ct, EndReason::Eliminated)[0]).collect();
        assert_eq!(pays, [1400, 1900, 2400, 2900, 3400]);
        // Ts break their streak: the bonus drops to the 1.6 reset value, then climbs again.
        assert_eq!(m.end_round(0, Team::T, EndReason::Eliminated)[1], 1500);
        assert_eq!(m.end_round(0, Team::T, EndReason::Eliminated)[1], 2000);
    }
}
