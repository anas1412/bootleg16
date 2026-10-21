//! What each message from a player does.
use super::*;

impl Server {
    pub(super) fn handle(&mut self, from: SocketAddr, msg: ClientMsg) {
        match msg {
            ClientMsg::Hello { version, name, cookie, ticket } => {
                if version != PROTOCOL_VERSION {
                    let reason = format!("server runs protocol {PROTOCOL_VERSION}, you have {version}");
                    self.socket.send(from, encode(&ServerMsg::Reject { reason }));
                    return;
                }
                if !self.clients.contains_key(&from) {
                    // A new address must first prove it's real: echo our cookie (stateless).
                    if !self.guard.cookie_ok(from, self.tick, cookie) {
                        let cookie = self.guard.cookie(from, self.tick);
                        self.socket.send(from, encode(&ServerMsg::Challenge { cookie }));
                        return;
                    }
                    if !from.ip().is_loopback() && self.clients.keys().filter(|a| a.ip() == from.ip()).count() >= guard::MAX_PER_IP {
                        self.socket.send(from, encode(&ServerMsg::Reject { reason: "too many players from your address".into() }));
                        return;
                    }
                    // A reserved match only lets in its own players (and puts them on their team).
                    let seat = match self.admit(ticket) {
                        Ok(seat) => seat,
                        Err(reason) => {
                            self.socket.send(from, encode(&ServerMsg::Reject { reason }));
                            return;
                        }
                    };
                    let humans = self.clients.values().filter(|c| !c.bot).count();
                    if self.clients.len() >= MAX_PLAYERS || humans >= self.round.cfg.max_players as usize {
                        self.socket.send(from, encode(&ServerMsg::Reject { reason: "server full".into() }));
                        return;
                    }
                    let id = (0..=u8::MAX).find(|i| self.clients.values().all(|c| c.id != *i)).unwrap_or(0);
                    // Everyone starts as a spectator and picks a side (JoinTeam), like CS.
                    let name: String = name.chars().filter(|c| !c.is_control()).take(24).collect();
                    let mut r = Remote::new(id, name, self.tick);
                    r.money = self.round.cfg.start_money;
                    // A deathmatch seat: keep its number (reports and aim flags name it).
                    if self.seats.contains_key(&ticket) {
                        r.ticket = ticket;
                    }
                    if let Some((group, team, saved)) = seat {
                        r.ticket = ticket;
                        r.group = Some(group);
                        r.team = team; // dead until the next round starts
                        if let Some((money, kills, deaths)) = saved {
                            (r.money, r.kills, r.deaths) = (money, kills, deaths);
                        }
                    }
                    self.clients.insert(from, r);
                    // An encrypted join: this address talks over the player's channel now.
                    if let Some((addr, ticket, ch)) = self.joining.take().filter(|j| j.0 == from) {
                        self.socket.secure(addr, ticket, ch);
                    }
                    // Deathmatch: straight in, on the smaller team (no team pick, like CSDM).
                    if self.rules.deathmatch {
                        self.handle(from, ClientMsg::JoinTeam { team: None });
                    }
                }
                let id = self.clients[&from].id;
                self.socket.send(from, encode(&ServerMsg::Welcome { id, map: self.map.clone(), map_hash: self.map_hash, tick_rate: self.rate.hz(), practice: self.rules.free_play() }));
            }
            ClientMsg::Inputs { first_seq, inputs, events, snap } => {
                let Some(c) = self.clients.get_mut(&from) else { return };
                c.last_heard = self.tick;
                // Ping, as the server sees it: from sending a snapshot to hearing it arrived.
                if snap > c.snap_ack && snap <= self.tick {
                    let ms = (self.tick - snap) as f32 * 1000.0 / self.rate.hz() as f32;
                    c.stats.ping = if c.stats.ping == 0.0 { ms } else { c.stats.ping * 0.9 + ms * 0.1 };
                }
                c.snap_ack = c.snap_ack.max(snap);
                while c.outbox.front().is_some_and(|(n, _)| *n <= events) {
                    c.outbox.pop_front();
                }
                for (i, input) in inputs.into_iter().enumerate() {
                    let seq = first_seq + i as u32;
                    if seq > c.last_queued {
                        c.queue.push_back((seq, input));
                        c.last_queued = seq;
                    }
                }
                // Never let a client queue up more than a second of movement.
                while c.queue.len() > self.rate.hz() as usize {
                    c.queue.pop_front();
                }
            }
            ClientMsg::Buy { item } => self.buy(from, item),
            ClientMsg::UndoBuys => self.undo_buys(from),
            ClientMsg::SideVote { switch } => self.side_vote(from, switch),
            ClientMsg::CallSurrender => self.call_surrender(from),
            ClientMsg::CallKick { player } => self.call_kick(from, player),
            ClientMsg::Report { player, reason } => self.report(from, player, reason),
            ClientMsg::Vote { yes } => self.surrender_vote(from, yes),
            ClientMsg::JoinTeam { .. } if self.reserved.is_some() => {} // teams are fixed in a reserved match
            ClientMsg::JoinTeam { team } => {
                let (t, ct) = self.clients.values().filter(|c| !c.bot).fold((0, 0), |(t, ct), c| match c.team {
                    Team::T => (t + 1, ct),
                    Team::Ct => (t, ct + 1),
                    Team::Spectator => (t, ct),
                });
                let tick = self.tick;
                // Mid-round you wait for the next one (freeze time still lets you in).
                let can_spawn = self.respawns() || self.round.phase == crate::rounds::Phase::Freeze;
                let Some(c) = self.clients.get_mut(&from) else { return };
                let team = team.unwrap_or(if t <= ct { Team::T } else { Team::Ct });
                if c.team == team {
                    return;
                }
                c.team = team;
                c.vitals.health = 0.0; // switching sides respawns you (or makes you watch)
                c.respawn_at = if team == Team::Spectator || !can_spawn { None } else { Some(tick) };
                let id = c.id;
                self.drop_bomb(id);
                self.place_dummies();
            }
            ClientMsg::Voice { channel, data } => self.relay_voice(from, channel, data),
            ClientMsg::Info { pad } => {
                // Padded requests only: the answer must not be bigger than the question.
                if pad.len() >= INFO_PAD {
                    let players = self.clients.values().filter(|c| !c.bot).count() as u8;
                    let cfg = &self.round.cfg;
                    // Short names: the answer must stay smaller than the padded request.
                    let short = |s: &str| s.chars().take(24).collect::<String>();
                    let info = ServerMsg::Info { version: PROTOCOL_VERSION, name: short(&cfg.name), map: short(&self.map), players, max: cfg.max_players };
                    self.socket.send(from, encode(&info));
                }
            }
            ClientMsg::Character { id } => {
                if let Some(c) = self.clients.get_mut(&from) {
                    c.character = character_id(id);
                }
            }
            ClientMsg::Bye => {
                if let Some(c) = self.clients.remove(&from) {
                    self.remember(&c);
                }
            }
        }
    }
}

/// A character id as the client sent it, or empty if it isn't one: lowercase [a-z0-9_-], at most 32 chars.
fn character_id(id: String) -> String {
    let ok = !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok { id } else { String::new() }
}
