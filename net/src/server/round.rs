//! Running the match on the server: what `rounds::Match` decides, applied to the players
//! (spawning, money, side swaps, who wins a round).
use super::*;
use crate::rounds::{side, EndReason, Next, Phase};

/// Kill rewards (docs/RULES.md 3.4).
const KILL_REWARD: i32 = 300;
const TEAMKILL_PENALTY: i32 = 3300;

impl Server {
    /// No rounds in the practice room.
    pub(super) fn rounds_on(&self) -> bool {
        !self.rules.free_play()
    }

    /// What the round phase allows everyone (plus the defuser can't move).
    pub(super) fn limits(&self, id: u8) -> sim::player::Limits {
        let freeze = self.rounds_on() && self.round.phase == Phase::Freeze;
        let over = self.rounds_on() && self.round.phase == Phase::GameOver;
        sim::player::Limits { frozen: freeze || over || self.defuser() == Some(id), no_attack: freeze || over }
    }

    /// Whether dead players come back by themselves (practice and warmup) or wait for the next round.
    pub(super) fn respawns(&self) -> bool {
        !self.rounds_on() || self.round.phase == Phase::Warmup
    }

    fn team_size(&self, team: Team) -> usize {
        self.clients.values().filter(|c| c.team == team).count()
    }

    pub(super) fn alive(&self, team: Team) -> usize {
        self.clients.values().filter(|c| c.team == team && c.vitals.alive()).count()
    }

    /// Once a tick: phase changes and round wins.
    pub(super) fn round_step(&mut self) {
        if !self.rounds_on() || self.surrender_step() || self.reserved_step() {
            return;
        }
        let tick = self.tick;
        let both = self.team_size(Team::T) > 0 && self.team_size(Team::Ct) > 0;
        match self.round.phase {
            Phase::Warmup => {
                if both {
                    self.round.start(tick);
                    self.new_round(false, Some(self.round.cfg.start_money));
                }
            }
            _ if !both => {
                // A side emptied: wait for players again.
                self.round = crate::rounds::Match::new(self.round.cfg.clone(), self.rate.hz());
                self.bomb = Bomb::None;
                for c in self.clients.values_mut().filter(|c| c.team != Team::Spectator && !c.vitals.alive()) {
                    c.respawn_at = Some(tick);
                }
            }
            Phase::Freeze => {
                if tick >= self.round.phase_end {
                    self.round.go_live(tick);
                }
            }
            Phase::Live => {
                let planted = matches!(self.bomb, Bomb::Planted { .. });
                if self.alive(Team::Ct) == 0 {
                    self.end_round(Team::T, EndReason::Eliminated);
                } else if self.alive(Team::T) == 0 && !planted {
                    self.end_round(Team::Ct, EndReason::Eliminated);
                } else if tick >= self.round.phase_end && !planted {
                    self.end_round(Team::Ct, EndReason::TimeUp);
                }
            }
            Phase::Over => {
                if tick >= self.round.phase_end {
                    match self.round.next(tick) {
                        Next::Round { swap, money } => self.new_round(swap, money),
                        Next::GameOver(winner) => {
                            if let Some(r) = &mut self.reserved {
                                r.winner = winner;
                            }
                            self.events.push(GameEvent::MatchOver { winner })
                        }
                    }
                }
            }
            Phase::GameOver => {
                if tick >= self.round.phase_end && self.reserved.is_some() {
                    self.finish_reserved();
                } else if tick >= self.round.phase_end {
                    self.round.start(tick);
                    for c in self.clients.values_mut() {
                        c.kills = 0;
                        c.deaths = 0;
                    }
                    self.new_round(false, Some(self.round.cfg.start_money));
                }
            }
        }
    }

    /// Decide the round and pay both teams.
    pub(super) fn end_round(&mut self, winner: Team, reason: EndReason) {
        let pay = self.round.end_round(self.tick, winner, reason);
        let max = self.round.cfg.max_money;
        for c in self.clients.values_mut().filter(|c| c.team != Team::Spectator) {
            // Time ran out: Ts still alive (who didn't plant) get nothing (CS 1.6
            // `MarkLivingPlayersOnTeamAsNotReceivingMoneyNextRound` after `#Target_Saved`).
            if reason == EndReason::TimeUp && c.team == Team::T && c.vitals.alive() {
                continue;
            }
            c.money = (c.money + pay[side(c.team)]).min(max);
        }
        self.events.push(GameEvent::RoundEnd { winner, reason });
        // The round's MVP (CS2): the defuser, the planter whose bomb went off, else the
        // winner with the most kills (then damage) this round.
        let defuser = self.events.iter().rev().find_map(|e| match e {
            GameEvent::BombDefused { by } => Some(*by),
            _ => None,
        });
        let mvp = match reason {
            EndReason::BombDefused => defuser,
            EndReason::BombExploded => self.planter,
            _ => None,
        }
        .or_else(|| {
            self.clients.values().filter(|c| c.team == winner && c.stats.round_kills > 0).max_by_key(|c| (c.stats.round_kills, c.stats.round_damage, std::cmp::Reverse(c.id))).map(|c| c.id)
        });
        for c in self.clients.values_mut().filter(|c| matches!(c.team, Team::T | Team::Ct)) {
            c.stats.rounds += 1;
            c.stats.mvps += (Some(c.id) == mvp) as u32;
        }
    }

    /// Freeze time: everyone back to their spawn. Survivors keep their weapons and armor;
    /// the dead start over with a pistol. `money`: halftime/overtime reset (weapons go too).
    pub(super) fn new_round(&mut self, swap: bool, money: Option<i32>) {
        if swap {
            if let Some(r) = &mut self.reserved {
                r.a_side = crate::rounds::other(r.a_side);
            }
            for c in self.clients.values_mut() {
                c.team = match c.team {
                    Team::T => Team::Ct,
                    Team::Ct => Team::T,
                    s => s,
                };
            }
            self.events.push(GameEvent::SidesSwapped);
        }
        self.planter = None;
        for c in self.clients.values_mut() {
            c.stats.round_kills = 0;
            c.stats.round_damage = 0;
            c.purchases.clear(); // a new round: last round's buys are yours for good
        }
        self.ground.clear();
        self.kits.clear();
        self.grenades.clear();
        self.smokes.clear();
        self.restore_pieces();
        let mut ids: Vec<(SocketAddr, u8)> = self.clients.iter().filter(|(_, c)| c.team != Team::Spectator).map(|(a, c)| (*a, c.id)).collect();
        ids.sort_by_key(|(_, id)| *id); // spawn points handed out in a fixed order
        for (addr, _) in ids {
            let (team, bot, keep) = {
                let c = &self.clients[&addr];
                (c.team, c.bot, c.vitals.alive() && money.is_none() && !swap)
            };
            // Everyone is "dead" while spawn points are picked, so nobody blocks their own.
            self.clients.get_mut(&addr).unwrap().vitals.health = 0.0;
            let fresh = self.spawn_state(team, bot);
            let c = self.clients.get_mut(&addr).unwrap();
            let armor = (c.vitals.armor, c.vitals.helmet);
            if keep {
                c.state.movement = fresh.movement;
                c.state.loadout.c4 = false;
                c.state.loadout.planting = None;
            } else {
                c.state = fresh;
                c.defuse_kit = false;
            }
            c.vitals = Vitals::default();
            if keep {
                (c.vitals.armor, c.vitals.helmet) = armor;
            }
            if let Some(m) = money {
                c.money = m;
            }
            // Ammo isn't bought: every round starts with full reserves.
            c.state.loadout.refill_ammo(true);
            c.state.loadout.refill_ammo(false);
            c.respawn_at = None;
        }
        self.give_bomb();
        self.events.push(GameEvent::RoundStart { round: self.round.round });
    }

    /// Money for kills (not in practice).
    pub(super) fn kill_money(&mut self, killer: u8, victim: u8) {
        if !self.rounds_on() || killer == victim || self.round.phase == Phase::Warmup {
            return;
        }
        let team = |id: u8| self.clients.values().find(|c| c.id == id).map(|c| c.team);
        let (kt, vt) = (team(killer), team(victim));
        let max = self.round.cfg.max_money;
        if let Some(k) = self.clients.values_mut().find(|c| c.id == killer) {
            k.money = if kt == vt { (k.money - TEAMKILL_PENALTY).max(0) } else { (k.money + KILL_REWARD).min(max) };
        }
    }

    /// Practice: after the bomb decided, everyone goes back to their spawn (a mini-round).
    pub(super) fn practice_step(&mut self) {
        if self.practice_reset == 0 || self.tick < self.practice_reset {
            return;
        }
        self.practice_reset = 0;
        let tick = self.tick;
        for c in self.clients.values_mut().filter(|c| !c.bot && c.team != Team::Spectator) {
            c.respawn_at = Some(tick);
        }
    }

    /// Deathmatch (CS2): 10-minute games; when one ends the top scorer is announced and
    /// everyone's score starts over.
    pub(super) fn deathmatch_step(&mut self) {
        if !self.rules.deathmatch {
            return;
        }
        if self.dm_end == 0 {
            self.dm_end = self.tick + self.rate.secs(self.rules.time_limit);
        }
        if self.tick < self.dm_end {
            return;
        }
        let best = self.clients.values().filter(|c| c.team != Team::Spectator).max_by_key(|c| (c.points, std::cmp::Reverse(c.id))).map(|c| c.id);
        if let Some(player) = best {
            self.events.push(GameEvent::KillLimit { player });
        }
        for c in self.clients.values_mut() {
            (c.kills, c.deaths, c.points) = (0, 0, 0);
            c.stats = Stats { ping: c.stats.ping, ..Stats::default() };
        }
        self.dm_end = self.tick + self.rate.secs(self.rules.time_limit);
    }

    /// The round state for the HUD.
    pub(super) fn round_net(&self) -> RoundNet {
        let left = match self.round.phase {
            // deathmatch: what's left of this game
            _ if self.rules.deathmatch => self.dm_end.saturating_sub(self.tick) as f32 * self.rate.tick(),
            Phase::Warmup => 0.0,
            _ => self.round.phase_end.saturating_sub(self.tick) as f32 * self.rate.tick(),
        };
        RoundNet {
            phase: if self.rounds_on() { self.round.phase } else { Phase::Warmup },
            round: self.round.round,
            score: self.round.score,
            left,
            stage: self.stage(),
            abandoned: [self.abandoned(Team::T), self.abandoned(Team::Ct)],
            vote: self.vote_net(),
            paused: self.reserved.as_ref().is_some_and(|r| r.empty_since.is_some()),
        }
    }
}
