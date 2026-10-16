//! Reserved matches: the server runs one match for a known list of players (from the
//! matchmaker or a custom game), each with a ticket and a group (team A or B). Nobody else can
//! join; a player who drops can come back to their own match. Custom games start with a knife
//! round whose winners vote to stay or switch sides.
use super::*;
use crate::rounds::Phase;
use serde::{Deserialize, Serialize};

/// The two teams of a match before sides are known.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    A,
    B,
}

/// What the matchmaker (or a custom game's host) sets up.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MatchSetup {
    /// Each player's ticket (a secret the matchmaker gave them) and group.
    pub players: Vec<(u64, Group)>,
    /// Knife round first, winners pick sides (custom games). Otherwise A starts as T.
    pub knife_round: bool,
}

/// How long to wait for everyone before starting with whoever came, and for the side vote.
const WAIT_SECONDS: u32 = 90;
const VOTE_SECONDS: u32 = 15;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Knife {
    /// Not started yet (waiting for players).
    Pending,
    /// The knife round is being played.
    Fighting,
    /// Winners vote until this tick.
    Vote { winners: Team, until: u32 },
    /// Over (or there is none): the real match.
    Done,
}

pub(super) struct Reserved {
    pub(super) setup: MatchSetup,
    pub(super) knife: Knife,
    since: u32,
    /// Winners' votes: player id -> switch sides?
    votes: HashMap<u8, bool>,
    /// What a player had when they left, for when they come back: money, kills, deaths.
    saved: HashMap<u64, (i32, i32, u32)>,
    /// The side group A is on now (B is the other); flips at every side swap.
    pub(super) a_side: Team,
}

impl Server {
    /// Run a match for exactly these players (clears the server).
    pub fn reserve(&mut self, setup: MatchSetup) {
        self.clients.retain(|_, c| c.bot);
        let knife = if setup.knife_round { Knife::Pending } else { Knife::Done };
        self.reserved = Some(Reserved { setup, knife, since: self.tick, votes: HashMap::new(), saved: HashMap::new(), a_side: Team::T });
        self.round = crate::rounds::Match::new(self.round.cfg.clone(), TICK_RATE);
    }

    /// The team a group plays on now: A starts T, B CT, and they swap with every side swap.
    fn team_for(&self, group: Group) -> Team {
        let a = self.reserved.as_ref().map(|r| r.a_side).unwrap_or(Team::T);
        if group == Group::A {
            a
        } else {
            crate::rounds::other(a)
        }
    }

    /// Who may join a reserved server: Err(reason) for anyone without a ticket.
    pub(super) fn admit(&mut self, ticket: u64) -> Result<Option<(Group, Team, Option<(i32, i32, u32)>)>, String> {
        let Some(r) = &self.reserved else { return Ok(None) };
        let Some(&(_, group)) = r.setup.players.iter().find(|(t, _)| *t == ticket && ticket != 0) else {
            return Err("This server is playing a match.".into());
        };
        if self.clients.values().any(|c| c.ticket == ticket) {
            return Err("You're already in this match.".into());
        }
        let saved = self.reserved.as_mut().and_then(|r| r.saved.remove(&ticket));
        Ok(Some((group, self.team_for(group), saved)))
    }

    /// A ticket holder left: keep their money and score for when they reconnect.
    pub(super) fn remember(&mut self, c: &Remote) {
        if let Some(r) = &mut self.reserved {
            if c.ticket != 0 {
                r.saved.insert(c.ticket, (c.money, c.kills, c.deaths));
            }
        }
    }

    /// Before the first round: wait for everyone (or a while), then knife round or round 1.
    /// True while this handles the round flow (the normal flow must not run).
    pub(super) fn reserved_step(&mut self) -> bool {
        let Some(r) = &self.reserved else { return false };
        let tick = self.tick;
        let expected = r.setup.players.len();
        let here = self.clients.values().filter(|c| c.ticket != 0).count();
        let waited = tick - r.since > WAIT_SECONDS * TICK_RATE;
        match r.knife {
            Knife::Pending if self.round.phase == Phase::Warmup => {
                if here >= expected || (waited && here > 0) {
                    self.start_knife_round();
                }
                true
            }
            Knife::Done if self.round.phase == Phase::Warmup => {
                // Matchmade: start when everyone is in (the normal flow does the rest).
                !(here >= expected || (waited && here > 0))
            }
            Knife::Fighting => {
                let (t, ct) = (self.alive(Team::T), self.alive(Team::Ct));
                let over = tick >= self.round.phase_end && self.round.phase == Phase::Live;
                if self.round.phase == Phase::Freeze && tick >= self.round.phase_end {
                    self.round.go_live(tick);
                } else if self.round.phase == Phase::Live && (t == 0 || ct == 0 || over) {
                    // Time up: whoever has more standing (a tie goes to the terrorists).
                    let winners = if t == 0 { Team::Ct } else if ct == 0 || t >= ct { Team::T } else { Team::Ct };
                    self.events.push(GameEvent::KnifeWon { winners });
                    let until = tick + VOTE_SECONDS * TICK_RATE;
                    self.round.phase = Phase::Over;
                    self.round.phase_end = until;
                    let r = self.reserved.as_mut().unwrap();
                    r.knife = Knife::Vote { winners, until };
                    r.votes.clear();
                }
                true
            }
            Knife::Vote { winners, until } => {
                let voters = self.clients.values().filter(|c| c.team == winners && !c.bot).count();
                let r = self.reserved.as_ref().unwrap();
                let switch = r.votes.values().filter(|s| **s).count();
                if tick >= until || r.votes.len() >= voters {
                    // Majority to switch, else stay.
                    let swap = switch * 2 > r.votes.len().max(1);
                    self.reserved.as_mut().unwrap().knife = Knife::Done;
                    self.events.push(GameEvent::SidesChosen { switched: swap });
                    self.round.start(tick);
                    self.new_round(swap, Some(self.round.cfg.start_money));
                }
                true
            }
            _ => false,
        }
    }

    /// Everyone out with only a knife, no buying, no bomb.
    fn start_knife_round(&mut self) {
        let tick = self.tick;
        self.round.phase = Phase::Freeze;
        self.round.round = 0;
        self.round.phase_end = tick + 3 * TICK_RATE;
        self.round.buy_end = 0;
        self.reserved.as_mut().unwrap().knife = Knife::Fighting;
        self.new_round(false, Some(0));
        for c in self.clients.values_mut().filter(|c| c.team != Team::Spectator) {
            c.state.loadout = sim::weapons::Loadout::knife_only();
            c.defuse_kit = false;
            c.vitals.armor = 0.0;
        }
        self.bomb = Bomb::None;
    }

    /// A vote from the knife round's winners.
    pub(super) fn side_vote(&mut self, from: SocketAddr, switch: bool) {
        let Some(c) = self.clients.get(&from) else { return };
        let (id, team) = (c.id, c.team);
        if let Some(r) = &mut self.reserved {
            if let Knife::Vote { winners, .. } = r.knife {
                if team == winners {
                    r.votes.insert(id, switch);
                }
            }
        }
    }

    /// The match is over: tell everyone, and free the server for the next one.
    pub(super) fn finish_reserved(&mut self) {
        if self.reserved.take().is_none() {
            return;
        }
        let addrs: Vec<SocketAddr> = self.clients.iter().filter(|(_, c)| !c.bot).map(|(a, _)| *a).collect();
        for a in addrs {
            self.socket.send(a, encode(&ServerMsg::MatchEnded));
        }
        self.clients.retain(|_, c| c.bot);
        self.round = crate::rounds::Match::new(self.round.cfg.clone(), TICK_RATE);
    }

    /// The knife round / side vote, for the HUD.
    pub(super) fn stage(&self) -> Stage {
        match self.reserved.as_ref().map(|r| r.knife) {
            Some(Knife::Pending) => Stage::Waiting,
            Some(Knife::Fighting) => Stage::Knife,
            Some(Knife::Vote { winners, until }) => Stage::SideVote { winners, left: until.saturating_sub(self.tick) as f32 * TICK },
            _ => Stage::Match,
        }
    }
}
