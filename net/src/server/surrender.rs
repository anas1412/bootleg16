//! Team votes and leavers in a reserved match. One leaver changes nothing: their seat waits
//! and they can come back. Once a teammate has been gone for 5 minutes (abandoned), the team
//! may call a surrender vote; it passes with a majority of the team still connected and the
//! other team wins. A team may also vote a teammate out (a kick vote: a majority of the
//! others): they can't come back to this match, and the team may then surrender. A side with
//! nobody left pauses the match; if it stays empty that long, it forfeits.
use super::*;
use crate::protocol::VoteNet;
use crate::rounds::Phase;

/// Gone this long counts as abandoned.
pub(super) const ABANDON_SECONDS: u32 = 300;
const VOTE_SECONDS: u32 = 30;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum VoteKind {
    Surrender,
    /// Vote this player (id, seat ticket) out.
    Kick(u8, u64),
}

pub(super) struct Vote {
    kind: VoteKind,
    team: Team,
    yes: Vec<u8>,
    no: Vec<u8>,
    until: u32,
}

impl Server {
    /// Players of this team (by their match seat) gone for good: away 5 minutes, or voted out.
    pub(super) fn abandoned(&self, team: Team) -> bool {
        let Some(r) = &self.reserved else { return false };
        r.left_at.iter().any(|(ticket, since)| {
            let group = r.setup.players.iter().find(|(t, _)| t == ticket).map(|(_, g)| *g);
            let gone = r.kicked.contains(ticket) || self.tick - since > ABANDON_SECONDS * self.rate.hz();
            group.is_some_and(|g| self.team_for(g) == team) && gone
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
        r.vote = Some(Vote { kind: VoteKind::Surrender, team, yes: vec![id], no: Vec::new(), until: tick + VOTE_SECONDS * self.rate.hz() });
    }

    /// A player asks their team to vote a teammate out (one vote at a time, one call per
    /// player per round).
    pub(super) fn call_kick(&mut self, from: SocketAddr, target: u8) {
        let Some(c) = self.clients.get(&from) else { return };
        let (id, team, round, tick) = (c.id, c.team, self.round.round, self.tick);
        let Some(t) = self.clients.values().find(|t| t.id == target && t.team == team && t.ticket != 0 && t.id != id) else { return };
        let ticket = t.ticket;
        let Some(r) = &mut self.reserved else { return };
        if team == Team::Spectator || r.vote.is_some() || r.kick_called.get(&id) == Some(&round) {
            return;
        }
        r.kick_called.insert(id, round);
        r.vote = Some(Vote { kind: VoteKind::Kick(target, ticket), team, yes: vec![id], no: Vec::new(), until: tick + VOTE_SECONDS * self.rate.hz() });
    }

    pub(super) fn surrender_vote(&mut self, from: SocketAddr, yes: bool) {
        let Some(c) = self.clients.get(&from) else { return };
        let (id, team) = (c.id, c.team);
        if let Some(v) = self.reserved.as_mut().and_then(|r| r.vote.as_mut()) {
            let target = matches!(v.kind, VoteKind::Kick(t, _) if t == id);
            if v.team == team && !target && !v.yes.contains(&id) && !v.no.contains(&id) {
                if yes { v.yes.push(id) } else { v.no.push(id) }
            }
        }
    }

    /// Who may vote: the team still connected (not the one a kick vote is about).
    fn voters(&self, team: Team, kind: VoteKind) -> usize {
        self.connected(team).iter().filter(|id| !matches!(kind, VoteKind::Kick(t, _) if t == **id)).count()
    }

    fn needed(&self, team: Team, kind: VoteKind) -> usize {
        self.voters(team, kind) / 2 + 1
    }

    /// The vote for the HUD (everyone sees it; the client shows it to that team).
    pub(super) fn vote_net(&self) -> Option<VoteNet> {
        let v = self.reserved.as_ref()?.vote.as_ref()?;
        let kick = if let VoteKind::Kick(t, _) = v.kind { Some(t) } else { None };
        Some(VoteNet { team: v.team, kick, yes: v.yes.len() as u8, no: v.no.len() as u8, needed: self.needed(v.team, v.kind) as u8, left: v.until.saturating_sub(self.tick) as f32 * self.rate.tick() })
    }

    /// Once a tick in a reserved match: count the vote, and pause while a side is empty.
    /// True while the match is paused or over (the round flow must not run).
    pub(super) fn surrender_step(&mut self) -> bool {
        let tick = self.tick;
        if let Some(v) = self.reserved.as_ref().and_then(|r| r.vote.as_ref()) {
            let (kind, team, yes, no, until) = (v.kind, v.team, v.yes.len(), v.no.len(), v.until);
            let (needed, voters) = (self.needed(team, kind), self.voters(team, kind));
            if yes >= needed {
                self.reserved.as_mut().unwrap().vote = None;
                match kind {
                    VoteKind::Surrender => {
                        self.events.push(GameEvent::Surrendered { team });
                        self.end_match(crate::rounds::other(team));
                        return true;
                    }
                    VoteKind::Kick(id, ticket) => self.vote_out(id, ticket),
                }
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
                if tick - since > ABANDON_SECONDS * self.rate.hz() {
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

    /// Voted out: gone for this match (they can't come back), and their team may surrender.
    fn vote_out(&mut self, id: u8, ticket: u64) {
        let tick = self.tick;
        if let Some(r) = &mut self.reserved {
            r.kicked.push(ticket);
            r.left_at.insert(ticket, tick);
        }
        self.events.push(GameEvent::VotedOut { player: id });
        if let Some(addr) = self.clients.iter().find(|(_, c)| c.id == id).map(|(a, _)| *a) {
            self.socket.send(addr, encode(&ServerMsg::Reject { reason: "Your team voted you out of this match.".into() }));
            self.drop_bomb(id);
            self.clients.remove(&addr);
        }
    }

    /// The match is decided before the last round (surrender, forfeit).
    fn end_match(&mut self, winner: Team) {
        self.round.forfeit(self.tick, winner);
        if let Some(r) = &mut self.reserved {
            r.winner = Some(winner);
        }
        self.events.push(GameEvent::MatchOver { winner: Some(winner) });
    }
}
