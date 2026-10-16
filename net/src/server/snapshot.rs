//! What goes out each tick: the scoreboard now and then, heartbeats to the lobbies, and every
//! player's snapshot (only what they may see: see `visibility`), with reliable events.
use super::*;

impl Server {
    pub(super) fn send_updates(&mut self) {
        let tick = self.tick;
        if self.tick % (TICK_RATE / 4) == 0 {
            let mut scores: Vec<Score> = self
                .clients
                .values()
                .map(|c| Score { id: c.id, name: c.name.clone(), kills: c.kills, deaths: c.deaths, alive: c.vitals.alive(), team: c.team, bot: c.bot })
                .collect();
            scores.sort_by_key(|s| (-s.kills, s.deaths));
            let msg = encode(&ServerMsg::Scores(scores));
            let addrs: Vec<_> = self.clients.iter().filter(|(_, c)| !c.bot).map(|(a, _)| *a).collect();
            for a in addrs {
                self.socket.send(a, msg.clone());
            }
        }
        if self.tick % HEARTBEAT_EVERY == 1 {
            self.heartbeat();
        }
        if self.tick % SNAPSHOT_EVERY == 0 {
            let players: Vec<PlayerNet> = self
                .clients
                .values()
                .map(|c| PlayerNet::new(c.id, c.state.movement.origin, c.view.0, c.view.1, c.state.movement.crouched, c.vitals.alive(), c.state.loadout.active_id(), c.team))
                .collect();
            let doors = self.world.doors.net_state();
            let events = std::mem::take(&mut self.events);
            self.kills_settled = 0;
            let now = self.tick as f64 * TICK as f64;
            let grenades: Vec<_> = self.grenades.iter().map(|g| (g.kind, g.origin)).collect();
            let smokes: Vec<SmokeNet> = self.smokes.iter().map(|s| SmokeNet { at: s.0, left: (s.1 - now) as f32 }).collect();
            let ground: Vec<(Id, [f32; 3])> = self.ground.iter().map(|g| (g.gun.id, g.origin)).collect();
            let round = self.round_net();
            let broken = self.broken_models();
            let viewers: Vec<u8> = self.clients.values().filter(|c| !c.bot).map(|c| c.id).collect();
            let sights: HashMap<u8, std::collections::HashSet<u8>> = viewers.iter().map(|id| (*id, self.visible_to(*id))).collect();
            let sends: Vec<_> = self
                .clients
                .iter()
                .filter(|(_, c)| !c.bot)
                .map(|(a, c)| {
                    let me = Me { money: c.money, limits: self.limits(c.id), can_buy: self.can_buy(c), defuse_kit: c.defuse_kit };
                    let seen = &sights[&c.id];
                    let players: Vec<PlayerNet> = players.iter().filter(|p| seen.contains(&p.id) && p.id != c.id).cloned().collect();
                    let you = Some((c.state.movement, c.vitals));
                    let events = self.events_for(c.id, seen, &events);
                    (*a, c.last_run, c.state.loadout.clone(), self.bomb_net(c.team), me, players, you, events)
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
                    round,
                    bomb,
                    me,
                };
                let bytes = encode(&msg);
                self.snapshot_bytes = bytes.len();
                self.socket.send(addr, bytes);
            }
        }
    }
}
