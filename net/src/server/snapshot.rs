//! What goes out each tick: the scoreboard now and then, heartbeats to the lobbies, and every
//! player's snapshot (only what they may see: see `visibility`), with reliable events.
use super::*;

/// Snapshots kept per client as baselines (half a second: confirmations older than that, from
/// a bad connection, get full snapshots).
const SENT_KEPT: usize = (crate::tick::SNAPSHOT_RATE / 2) as usize;

impl Server {
    pub(super) fn send_updates(&mut self) {
        let tick = self.tick;
        if self.tick % (self.rate.hz() / 4) == 0 {
            let mut scores: Vec<Score> = self.clients.values().map(|c| c.score()).collect();
            scores.sort_by_key(|s| (-s.kills, s.deaths));
            // Money only for your own team (spectators: nobody's).
            let for_team = |team: Team| {
                let list: Vec<Score> = scores.iter().map(|s| Score { money: s.money.filter(|_| team != Team::Spectator && s.team == team), ..s.clone() }).collect();
                encode(&ServerMsg::Scores(list))
            };
            let msgs = [for_team(Team::T), for_team(Team::Ct), for_team(Team::Spectator)];
            let history = encode(&ServerMsg::History(self.round.history.clone()));
            let addrs: Vec<_> = self.clients.iter().filter(|(_, c)| !c.bot).map(|(a, c)| (*a, c.team)).collect();
            for (a, team) in addrs {
                let msg = match team {
                    Team::T => &msgs[0],
                    Team::Ct => &msgs[1],
                    _ => &msgs[2],
                };
                self.socket.send(a, msg.clone());
                self.socket.send(a, history.clone());
            }
        }
        if self.tick % self.rate.secs(HEARTBEAT_SECS) == 1 {
            self.heartbeat();
        }
        if self.tick % self.rate.snapshot_every() == 0 {
            let players: Vec<PlayerNet> = self
                .clients
                .values()
                .map(|c| {
                    let (m, v) = (&c.state.movement, &c.vitals);
                    let mate = Mate { health: v.health.clamp(0.0, 255.0).ceil() as u8, armor: v.armor.clamp(0.0, 255.0).ceil() as u8, helmet: v.helmet, defuse_kit: c.defuse_kit, ammo: c.state.loadout.active_gun().map(|g| (g.clip.min(u16::MAX as u32) as u16, g.reserve.min(u16::MAX as u32) as u16)) };
                    PlayerNet::new(c.id, m.origin, c.view.0, c.view.1, m.crouched, v.alive(), !m.on_ground && !m.on_ladder, c.state.loadout.active_gun().is_some_and(|g| g.reloading), c.state.loadout.active_id(), c.team, Some(mate)).protected(c.protected_until > self.tick)
                })
                .collect();
            let doors = self.world.doors.net_state();
            let events = std::mem::take(&mut self.events);
            self.kills_settled = 0;
            let now = self.tick as f64 * self.rate.tick() as f64;
            let grenades: Vec<_> = self.grenades.iter().map(|g| (g.kind, g.origin)).collect();
            let smokes: Vec<SmokeNet> = self.smokes.iter().map(|s| SmokeNet { at: s.0, left: (s.1 - now) as f32 }).collect();
            let ground: Vec<(Id, [f32; 3])> = self.ground.iter().map(|g| (g.gun.id, g.origin)).collect();
            let round = self.round_net();
            let broken = self.broken_models();
            // The recording gets everything, before anything is culled per player.
            self.record(&players, &events, &doors, &broken, &grenades, &smokes, &ground, &round);
            let viewers: Vec<u8> = self.clients.values().filter(|c| !c.bot).map(|c| c.id).collect();
            let sights: HashMap<u8, std::collections::HashSet<u8>> = viewers.iter().map(|id| (*id, self.visible_to(*id))).collect();
            self.note_sightings(&sights);
            let sends: Vec<_> = self
                .clients
                .iter()
                .filter(|(_, c)| !c.bot)
                .map(|(a, c)| {
                    let buy_left = if self.rules.free_play() { None } else { self.round.buy_left(self.tick).map(|t| t as f32 * self.rate.tick()) };
                    let me = Me { money: c.money, limits: self.limits(c.id), can_buy: self.can_buy(c), buy_left, defuse_kit: c.defuse_kit, protected: c.protected_until > self.tick, hurt_from: c.hurt_from.iter().map(|f| { let o = c.state.movement.origin; (f[1] - o[1]).atan2(f[0] - o[0]).to_degrees() }).collect() };
                    let seen = &sights[&c.id];
                    // Health and kit only for teammates (spectators see everything anyway).
                    let players: Vec<PlayerNet> = players
                        .iter()
                        .filter(|p| seen.contains(&p.id) && p.id != c.id)
                        .map(|p| {
                            let mut p = p.clone();
                            p.mate = p.mate.filter(|_| c.team == Team::Spectator || p.team == c.team);
                            p
                        })
                        .collect();
                    let you = Some((c.state.movement, c.vitals));
                    let events = self.events_for(c.id, seen, &events);
                    (*a, c.last_run, c.state.loadout.clone(), self.bomb_net(c.team, |id| seen.contains(&id), c.state.movement.eye()), me, players, you, events)
                })
                .collect();
            for (addr, ack, loadout, bomb, me, players, you, events) in sends {
                // Reliable events go in the client's numbered outbox until it confirms them.
                let (reliable_new, events): (Vec<GameEvent>, Vec<GameEvent>) = events.into_iter().partition(|e| e.reliable());
                let c = self.clients.get_mut(&addr).unwrap();
                for e in reliable_new {
                    c.next_reliable += 1;
                    c.outbox.push_back((c.next_reliable, e));
                }
                while c.outbox.len() > MAX_OUTBOX {
                    c.outbox.pop_front(); // a client that stopped acknowledging: don't grow forever
                }
                let reliable: Vec<(u32, GameEvent)> = c.outbox.iter().cloned().collect();
                let msg = ServerMsg::Snapshot {
                    tick,
                    ack,
                    players,
                    you,
                    doors: doors.clone(),
                    loadout,
                    events,
                    reliable,
                    broken: broken.clone(),
                    grenades: grenades.clone(),
                    smokes: smokes.clone(),
                    ground: ground.clone(),
                    kits: self.kits.clone(),
                    round,
                    bomb,
                    me,
                };
                let bytes = encode(&msg);
                let c = self.clients.get_mut(&addr).unwrap();
                c.hurt_from.clear();
                let wire = match c.sent.iter().find(|(t, _)| *t == c.snap_ack) {
                    Some((base, old)) => Some(encode(&ServerMsg::Delta { base: *base, patch: crate::delta::diff(old, &bytes) })).filter(|d| d.len() < bytes.len()),
                    None => None,
                };
                c.sent.push_back((tick, bytes.clone()));
                if c.sent.len() > SENT_KEPT {
                    c.sent.pop_front();
                }
                let wire = wire.unwrap_or(bytes);
                self.snapshot_bytes = wire.len();
                self.socket.send(addr, wire);
            }
        }
    }
}
