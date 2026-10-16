//! Parties (codes, leaders, members, invites), what players ask for, and the state they get back.
use super::*;

impl Master {
    /// A party code nobody else has.
    pub(super) fn new_code(&mut self) -> String {
        loop {
            let code = self.random_code();
            if !self.parties.values().any(|p| p.code == code) {
                return code;
            }
        }
    }

    pub(super) fn random_code(&mut self) -> String {
        // No 0/O/1/I: easy to read out to a friend.
        const ABC: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        let r = self.rand();
        let c: String = (0..6).map(|i| ABC[((r >> (i * 5)) & 31) as usize] as char).collect();
        format!("{}-{}", &c[..3], &c[3..])
    }

    pub(super) fn new_party(&mut self, player: u32) -> u32 {
        let code = self.new_code();
        let id = self.next_id;
        self.next_id += 1;
        self.parties.insert(id, Party { code, leader: player, mode: Mode::Casual, members: vec![(player, Group::A)], queued: None });
        id
    }

    /// Take a player out of their party (it closes if they were the last).
    pub(super) fn leave(&mut self, player: u32, party: u32) {
        if let Some(p) = self.parties.get_mut(&party) {
            p.members.retain(|(m, _)| *m != player);
            p.queued = None;
            if p.members.is_empty() {
                self.parties.remove(&party);
            } else if p.leader == player {
                p.leader = p.members[0].0;
            }
        }
    }

    pub(super) fn player_party(&self, session: u64) -> Option<(u32, u32)> {
        self.players.get(&session).map(|p| (p.id, p.party))
    }

    pub(super) fn command(&mut self, session: u64, cmd: Command) {
        let Some((me, party)) = self.player_party(session) else { return };
        if let Some(p) = self.players.get_mut(&session) {
            p.seen = Instant::now();
        }
        let leader = self.parties.get(&party).is_some_and(|p| p.leader == me);
        let mut notice = String::new();
        // Official lobbies: nothing but waiting until Steam has confirmed who you are.
        if self.require_steam && self.players.get(&session).is_some_and(|p| p.steam == 0) && cmd != Command::Poll {
            if let Some(p) = self.players.get_mut(&session) {
                p.notice = "Waiting for Steam to confirm your sign-in...".into();
            }
            return;
        }
        match cmd {
            Command::Poll => {}
            Command::JoinParty { code } => {
                let code = code.trim().to_ascii_uppercase();
                match self.parties.iter().find(|(_, p)| p.code == code).map(|(id, p)| (*id, p.members.len(), p.mode)) {
                    Some((target, _, _)) if target == party => {}
                    Some((target, n, mode)) => {
                        let max = if mode == Mode::Custom { MAX_CUSTOM } else { MAX_PARTY };
                        if n >= max {
                            notice = "That party is full.".into();
                        } else {
                            self.leave(me, party);
                            let t = self.parties.get_mut(&target).unwrap();
                            // New members go to the smaller custom team.
                            let a = t.members.iter().filter(|(_, g)| *g == Group::A).count();
                            t.members.push((me, if a * 2 <= n { Group::A } else { Group::B }));
                            t.queued = None;
                            self.players.get_mut(&session).unwrap().party = target;
                        }
                    }
                    None => notice = "No party with that code.".into(),
                }
            }
            Command::LeaveParty => {
                self.leave(me, party);
                let p = self.new_party(me);
                self.players.get_mut(&session).unwrap().party = p;
            }
            Command::SetMode(mode) if leader => {
                let p = self.parties.get_mut(&party).unwrap();
                if mode == Mode::Casual && p.members.len() > MAX_PARTY {
                    notice = "Casual parties are up to 5. Use a custom game for more.".into();
                } else {
                    p.mode = mode;
                    p.queued = None;
                }
            }
            Command::Move { member, group } if leader => {
                if let Some(m) = self.parties.get_mut(&party).unwrap().members.iter_mut().find(|(m, _)| *m == member) {
                    m.1 = group;
                }
            }
            Command::Accept => self.accept(me),
            Command::Decline => self.decline(me),
            Command::Play if leader => {
                let mode = self.parties[&party].mode;
                // Anyone in the party on a cooldown keeps the party out of the queue.
                let cooling = self.parties[&party].members.iter().any(|(m, _)| self.cooldown_of(*m).is_some());
                match mode {
                    Mode::Casual if cooling => notice = "Someone in your party can't queue yet (they didn't accept a match, or left one).".into(),
                    Mode::Casual => self.parties.get_mut(&party).unwrap().queued = Some(Instant::now()),
                    Mode::Custom => {
                        let members = self.parties[&party].members.clone();
                        if members.len() < 2 {
                            notice = "Invite at least one friend first.".into();
                        } else if !self.start_custom(members) {
                            notice = "No server free right now. Try again in a moment.".into();
                        }
                    }
                }
            }
            Command::Cancel if leader => {
                self.parties.get_mut(&party).unwrap().queued = None;
                self.decline(me);
            }
            Command::Pings(pings) => {
                if let Some(p) = self.players.get_mut(&session) {
                    p.pings = pings.into_iter().take(32).map(|(r, ms)| (r.chars().take(16).collect(), ms)).collect();
                }
            }
            Command::Invite { steam } => match self.players.values_mut().find(|p| p.steam == steam && steam != 0) {
                Some(friend) => {
                    friend.invites.retain(|(from, _)| *from != me);
                    friend.invites.push((me, Instant::now()));
                    // The newest few (spamming invites can't push the answer past its size).
                    let extra = friend.invites.len().saturating_sub(MAX_INVITES);
                    friend.invites.drain(..extra);
                }
                None => notice = "They're not in the game right now.".into(),
            },
            _ => notice = "Only the party leader can do that.".into(),
        }
        if !notice.is_empty() {
            if let Some(p) = self.players.get_mut(&session) {
                p.notice = notice;
            }
        }
    }

    pub(super) fn state_of(&mut self, session: u64) -> Option<LobbyState> {
        let p = self.players.get_mut(&session)?;
        let notice = std::mem::take(&mut p.notice);
        p.invites.retain(|(_, at)| at.elapsed().as_secs_f32() < INVITE_SECONDS);
        let invited = p.invites.clone();
        let (you, party_id, found) = (p.id, p.party, p.found.take());
        let party = self.parties.get(&party_id)?;
        let who = |id: u32| self.players.values().find(|q| q.id == id).map(|q| (q.name.clone(), q.steam)).unwrap_or_default();
        Some(LobbyState {
            you,
            code: party.code.clone(),
            leader: party.leader,
            mode: party.mode,
            members: party.members.iter().map(|(m, g)| (*m, who(*m), *g)).map(|(id, (name, steam), group)| Member { id, name, group, steam }).collect(),
            searching: party.queued.map(|t| t.elapsed().as_secs_f32()),
            ready: self.ready_of(you),
            rejoin: self.active.get(&self.penalty_key(you)).copied(),
            cooldown: self.cooldown_of(you),
            found,
            notice,
            regions: self.ping_targets().into_iter().take(MAX_REGIONS).collect(),
            invites: invited
                .iter()
                .filter_map(|(from, at)| {
                    let q = self.players.values().find(|q| q.id == *from)?;
                    let code = self.parties.get(&q.party)?.code.clone();
                    Some(Invite { from: q.name.clone(), steam: q.steam, code, left: INVITE_SECONDS - at.elapsed().as_secs_f32() })
                })
                .collect(),
        })
    }
}
