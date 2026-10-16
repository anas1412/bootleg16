//! When players leave a reserved match. One leaver changes nothing: their seat waits and they
//! can come back. Once a teammate has been gone for 5 minutes (abandoned), the team may call a
//! surrender vote; it passes with a majority of the team still connected and the other team
//! wins. A side with nobody left pauses the match; if it stays empty that long, it forfeits.
use super::*;
use crate::protocol::VoteNet;
use crate::rounds::Phase;

/// Gone this long counts as abandoned.
pub(super) const ABANDON_SECONDS: u32 = 300;
const VOTE_SECONDS: u32 = 30;

pub(super) struct Vote {
    team: Team,
    yes: Vec<u8>,
    no: Vec<u8>,
    until: u32,
}

impl Server {
    /// Players of this team (by their match seat) who left and stayed away 5 minutes.
    pub(super) fn abandoned(&self, team: Team) -> bool {
        let Some(r) = &self.reserved else { return false };
        r.left_at.iter().any(|(ticket, since)| {
            let group = r.setup.players.iter().find(|(t, _)| t == ticket).map(|(_, g)| *g);
            group.is_some_and(|g| self.team_for(g) == team) && self.tick - since > ABANDON_SECONDS * TICK_RATE
        })
    }

    fn connected(&self, team: Team) -> Vec<u8> {
        self.clients.values().filter(|c| c.team == team && !c.bot).map(|c| c.id).collect()
    }

    /// A player asks their team to surrender (only once someone of theirs abandoned, and one
    /// vote per team per round).
    pub(super) fn call_surrender(&mut self, from: SocketAddr) {
        let Some(c) = self.clients.get(&from) else { return };
        let (id, team) = (c.id, c.team);
        let round = self.round.round;
        if team == Team::Spectator || !self.abandoned(team) {
            return;
        }
        let tick = self.tick;
        let Some(r) = &mut self.reserved else { return };
        if r.vote.is_some() || r.voted_in_round[(team == Team::Ct) as usize] == Some(round) {
            return;
        }
        r.voted_in_round[(team == Team::Ct) as usize] = Some(round);
        r.vote = Some(Vote { team, yes: vec![id], no: Vec::new(), until: tick + VOTE_SECONDS * TICK_RATE });
    }

    pub(super) fn surrender_vote(&mut self, from: SocketAddr, yes: bool) {
        let Some(c) = self.clients.get(&from) else { return };
        let (id, team) = (c.id, c.team);
        if let Some(v) = self.reserved.as_mut().and_then(|r| r.vote.as_mut()) {
            if v.team == team && !v.yes.contains(&id) && !v.no.contains(&id) {
                if yes { v.yes.push(id) } else { v.no.push(id) }
            }
        }
    }

    fn needed(&self, team: Team) -> usize {
        self.connected(team).len() / 2 + 1
    }

    /// The vote for the HUD (everyone sees it; the client shows it to that team).
    pub(super) fn vote_net(&self) -> Option<VoteNet> {
        let v = self.reserved.as_ref()?.vote.as_ref()?;
        Some(VoteNet { team: v.team, yes: v.yes.len() as u8, no: v.no.len() as u8, needed: self.needed(v.team) as u8, left: v.until.saturating_sub(self.tick) as f32 * TICK })
    }

    /// Once a tick in a reserved match: count the vote, and pause while a side is empty.
    /// True while the match is paused or over (the round flow must not run).
    pub(super) fn surrender_step(&mut self) -> bool {
        let tick = self.tick;
        if let Some(v) = self.reserved.as_ref().and_then(|r| r.vote.as_ref()) {
            let (team, yes, no, until) = (v.team, v.yes.len(), v.no.len(), v.until);
            let (needed, voters) = (self.needed(team), self.connected(team).len());
            if yes >= needed {
                self.reserved.as_mut().unwrap().vote = None;
                self.events.push(GameEvent::Surrendered { team });
                self.end_match(crate::rounds::other(team));
                return true;
            }
            if tick >= until || voters - no.min(voters) < needed {
                self.reserved.as_mut().unwrap().vote = None;
            }
        }
        let Some(r) = &self.reserved else { return false };
        let started = matches!(r.knife, reserve::Knife::Done) && !matches!(self.round.phase, Phase::Warmup | Phase::GameOver);
        // One side empty pauses; everyone gone is the server's to free (see `reserved_step`).
        let anyone = self.clients.values().any(|c| c.ticket != 0);
        let empty = [Team::T, Team::Ct].into_iter().find(|t| self.connected(*t).is_empty()).filter(|_| anyone);
        match (started, empty) {
            (true, Some(team)) => {
                let since = *self.reserved.as_mut().unwrap().empty_since.get_or_insert(tick);
                if tick - since > ABANDON_SECONDS * TICK_RATE {
                    self.reserved.as_mut().unwrap().empty_since = None;
                    self.end_match(crate::rounds::other(team));
                } else {
                    // Paused: the clock doesn't run.
                    self.round.phase_end += 1;
                    self.round.buy_end += 1;
                }
                true
            }
            _ => {
                self.reserved.as_mut().unwrap().empty_since = None;
                false
            }
        }
    }

    /// The match is decided before the last round (surrender, forfeit).
    fn end_match(&mut self, winner: Team) {
        self.round.forfeit(self.tick, winner);
        self.events.push(GameEvent::MatchOver { winner: Some(winner) });
    }
}
