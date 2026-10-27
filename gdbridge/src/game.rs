//! What the HUD, the buy menu and the world read from a session: your health, money and
//! ammo, the round and the bomb, everyone else, grenades in the air, and what just happened.
use crate::session::Session;
use crate::units::{to_godot, yaw_to_godot};
use godot::prelude::*;
use net::protocol::{BombNet, GameEvent, Team, VoiceChannel};
use std::time::{Duration, Instant};

/// A voice stays "talking" this long after its last packet (packets come every 20 ms).
const HEARD_FOR: Duration = Duration::from_millis(250);

/// Someone we heard: on which channel, through a wall or not, how loud (1 to 255), when.
pub(crate) struct Heard {
    channel: VoiceChannel,
    muffled: bool,
    gain: u8,
    at: Instant,
}
use net::rounds::{EndReason, Phase};

/// A side as Godot names it: "t" (the Strays), "ct" (the Bureau), "" (watching).
fn side(t: Team) -> &'static str {
    match t {
        Team::T => "t",
        Team::Ct => "ct",
        Team::Spectator => "",
    }
}

#[godot_api(secondary)]
impl Session {
    /// Everything the HUD shows, read once a frame: `alive`, `health`, `armor`, `helmet`,
    /// `kit`, `money`, `can_buy`, `buy_left` (seconds, -1: no limit), `in_zone` (your buy
    /// zone), `team`, `phase`
    /// (warmup, freeze, live, over, gameover), `round`, `left` (seconds of the phase),
    /// `score_t`, `score_ct`, `alive_t`, `alive_ct`, `weapon` (CS name), `slot` (1 to 5),
    /// `clip` and `reserve` (-1: no gun), `reloading`, `grenades` (HE, flash, smoke), `c4`
    /// (you carry it), `planting` (0 to 1, -1: not), `bomb` (none, carried, dropped,
    /// planted), `bomb_at`, `bomb_left` (the fuse), `defusing` (someone is), `defuse_left`,
    /// `defuse_total` (that defuse's whole length), `you_defuse`.
    #[func]
    fn hud(&self) -> VarDictionary {
        let Some(c) = &self.client else { return VarDictionary::new() };
        let l = &c.state.loadout;
        let gun = l.active_gun();
        let phase = match c.round.phase {
            Phase::Warmup => "warmup",
            Phase::Freeze => "freeze",
            Phase::Live => "live",
            Phase::Over => "over",
            Phase::GameOver => "gameover",
        };
        let alive = |t| c.scores.iter().filter(|s| s.team == t && s.alive).count() as i64;
        let planting = l.planting.map_or(-1.0, |since| ((c.cmd_time() - since) / sim::weapons::C4_PLANT_TIME).clamp(0.0, 1.0)) as f32;
        let (bomb, at, left, defuse) = match c.bomb {
            BombNet::None => ("none", None, 0.0, None),
            BombNet::Carried(_) => ("carried", None, 0.0, None),
            BombNet::Dropped(at) => ("dropped", Some(at), 0.0, None),
            BombNet::Planted { at, left, defuse } => ("planted", Some(at), left, defuse),
        };
        // the server's lengths (CS2: 10 s, 5 with a kit); a defuse that started with more
        // left than a kit's length is a defuse without one
        let rules = net::config::MatchConfig::default();
        let defuse_total = match defuse {
            Some((_, left)) => {
                let seen = self.defuse_seen.get().max(left);
                self.defuse_seen.set(seen);
                if seen > rules.defuse_kit_time + 0.01 { rules.defuse_time } else { rules.defuse_kit_time }
            }
            None => {
                self.defuse_seen.set(0.0);
                0.0
            }
        };
        let zone = match c.team() {
            Team::T => Some(sim::zones::Side::T),
            Team::Ct => Some(sim::zones::Side::Ct),
            Team::Spectator => None,
        };
        let in_zone = zone.is_some_and(|z| c.world.zones.in_buy_zone(z, c.me().origin, c.me().hull()));
        let mut d = vdict! {
            "alive" => c.vitals.alive(),
            "health" => c.vitals.health.ceil() as i64,
            "armor" => c.vitals.armor.ceil() as i64,
            "helmet" => c.vitals.helmet,
            "kit" => c.me_info.defuse_kit,
            "money" => c.me_info.money as i64,
            "can_buy" => c.me_info.can_buy,
            "buy_left" => c.me_info.buy_left.unwrap_or(-1.0),
            "in_zone" => in_zone,
            "team" => side(c.team()),
            "phase" => phase,
            "round" => c.round.round as i64,
            "left" => c.round.left,
            "score_t" => c.round.score[0] as i64,
            "score_ct" => c.round.score[1] as i64,
            "alive_t" => alive(Team::T),
            "alive_ct" => alive(Team::Ct),
            "weapon" => sim::weapons::data::def(l.active_id()).name,
            "slot" => l.active as i64,
            "clip" => gun.map_or(-1, |g| g.clip as i64),
            "reserve" => gun.map_or(-1, |g| g.reserve as i64),
            "reloading" => gun.is_some_and(|g| g.reloading),
            "c4" => l.c4,
            "planting" => planting,
            "bomb" => bomb,
            "bomb_left" => left,
            "defusing" => defuse.is_some(),
            "defuse_left" => defuse.map_or(0.0, |d| d.1),
            "defuse_total" => defuse_total,
            "you_defuse" => defuse.is_some_and(|d| Some(d.0) == c.id()),
        };
        d.set("grenades", &PackedInt32Array::from(l.grenades.map(|n| n as i32).as_slice()));
        if let Some(at) = at {
            d.set("bomb_at", to_godot(at));
        }
        d
    }

    /// What happened since the last call, oldest first, each a dictionary with a `type`:
    /// `kill` (killer, victim, their teams, weapon: a CS name, "bomb" or "" for a fall,
    /// headshot, by_you, of_you), `hit` (you hit someone: head), `hurt` (you were hit:
    /// amount), `flash` (you're blinded: hold and fade seconds, alpha 0 to 1), `shot`,
    /// `swing` and `throw` (yours, for the gun in your hands), `round_start` (round),
    /// `round_end` (winner, reason: eliminated, bomb, defused, time), `swap`, `match_over`
    /// (winner, "" for a draw), `planted`, `defused`, `exploded` (at), `explosion` (an HE: at).
    #[func]
    fn take_events(&mut self) -> Array<VarDictionary> {
        let mut out = Array::new();
        let Some(c) = &mut self.client else { return out };
        let me = c.id();
        let events = c.take_events();
        let name = |id: u8| c.scores.iter().find(|s| s.id == id).map_or(String::new(), |s| s.name.clone());
        let team = |id: u8| c.scores.iter().find(|s| s.id == id).map_or("", |s| side(s.team));
        for e in events {
            let d = match e {
                GameEvent::Kill { killer, victim, weapon, headshot } => {
                    let how = match weapon {
                        Some(w) => sim::weapons::data::def(w).name,
                        None if killer == net::protocol::WORLD => "bomb",
                        None => "",
                    };
                    let by = if killer == victim || killer == net::protocol::WORLD { String::new() } else { name(killer) };
                    vdict! {
                        "type" => "kill", "killer" => by.as_str(), "killer_team" => team(killer), "victim" => name(victim).as_str(),
                        "victim_team" => team(victim), "weapon" => how, "headshot" => headshot,
                        "by_you" => Some(killer) == me && killer != victim, "of_you" => Some(victim) == me,
                    }
                }
                GameEvent::Damage { attacker, victim, amount, group, .. } if Some(attacker) == me && attacker != victim => {
                    vdict! { "type" => "hit", "head" => group == sim::hitbox::HitGroup::Head, "amount" => amount }
                }
                GameEvent::Damage { victim, amount, .. } if Some(victim) == me => vdict! { "type" => "hurt", "amount" => amount },
                GameEvent::Flash { blinded, .. } => match blinded.iter().find(|(id, _)| Some(*id) == me) {
                    Some((_, b)) => vdict! { "type" => "flash", "hold" => b.hold, "fade" => b.fade, "alpha" => b.alpha as f32 / 255.0 },
                    None => continue,
                },
                GameEvent::Shot { shooter, .. } if Some(shooter) == me => vdict! { "type" => "shot" },
                GameEvent::Knife { player, .. } if Some(player) == me => vdict! { "type" => "swing" },
                GameEvent::Thrown { player, .. } if Some(player) == me => vdict! { "type" => "throw" },
                GameEvent::RoundStart { round } => vdict! { "type" => "round_start", "round" => round as i64 },
                GameEvent::RoundEnd { winner, reason } => {
                    let why = match reason {
                        EndReason::Eliminated => "eliminated",
                        EndReason::BombExploded => "bomb",
                        EndReason::BombDefused => "defused",
                        EndReason::TimeUp => "time",
                    };
                    vdict! { "type" => "round_end", "winner" => side(winner), "reason" => why }
                }
                GameEvent::SidesSwapped => vdict! { "type" => "swap" },
                GameEvent::MatchOver { winner } => vdict! { "type" => "match_over", "winner" => winner.map_or("", side) },
                GameEvent::BombPlanted { .. } => vdict! { "type" => "planted" },
                GameEvent::BombDefused { .. } => vdict! { "type" => "defused" },
                GameEvent::BombExploded { at } => vdict! { "type" => "exploded", "at" => to_godot(at) },
                GameEvent::Explosion { at } => vdict! { "type" => "explosion", "at" => to_godot(at) },
                _ => continue,
            };
            out.push(&d);
        }
        out
    }

    /// Everyone else as drawn this frame (`frac` as in `eye`): `feet` (Godot metres), `yaw`
    /// (Godot radians), `team`, `alive`, `crouched`, `slot` (what's in their hands, 1 to 5).
    #[func]
    fn others(&self, frac: f32) -> Array<VarDictionary> {
        let Some(c) = &self.client else { return Array::new() };
        c.others_at(frac)
            .iter()
            .filter(|o| o.team != Team::Spectator)
            .map(|o| {
                vdict! {
                    "feet" => to_godot([o.origin[0], o.origin[1], o.origin[2] - if o.crouched { 18.0 } else { 36.0 }]),
                    "yaw" => yaw_to_godot(o.yaw), "team" => side(o.team), "alive" => o.alive,
                    "crouched" => o.crouched, "slot" => sim::weapons::data::def(o.weapon).slot as i64,
                }
            })
            .collect()
    }

    /// Everyone in the match for the top bar, in score order: `id`, `name`, `team`, `alive`,
    /// `bot`, `you`, `health` (yours and your teammates'; -1 for enemies, as CS shows them).
    #[func]
    fn players(&self) -> Array<VarDictionary> {
        let Some(c) = &self.client else { return Array::new() };
        let me = c.id();
        let mates: std::collections::HashMap<u8, i64> = c.others_at(1.0).iter().filter_map(|o| o.mate.map(|m| (o.id, m.health as i64))).collect();
        c.scores
            .iter()
            .filter(|s| s.team != Team::Spectator)
            .map(|s| {
                let you = Some(s.id) == me;
                let health = if you { c.vitals.health.ceil() as i64 } else { mates.get(&s.id).copied().unwrap_or(-1) };
                vdict! {
                    "id" => s.id as i64, "name" => s.name.as_str(), "team" => side(s.team), "alive" => s.alive,
                    "bot" => s.bot, "you" => you, "health" => health,
                }
            })
            .collect()
    }

    /// How far the next shot can stray, as the crosshair draws it: the tangent of the cone
    /// (0 with no gun out). The gap at a screen height `h` is `spread * (h / 2) / 0.75`.
    #[func]
    fn spread(&self) -> f32 {
        let Some(c) = &self.client else { return 0.0 };
        let p = c.me();
        c.state.loadout.spread_now(p.on_ground, p.crouched, p.speed_2d()).unwrap_or(0.0)
    }

    /// Sell back what you bought this round and haven't used (the server says yes or no:
    /// buy time, buy zone), as CS2's undo.
    #[func]
    fn undo_buys(&mut self) {
        if let Some(c) = &mut self.client {
            c.undo_buys();
        }
    }

    /// Who you hear now: `id`, `name`, `team`, `channel` ("team" or "near"), `muffled` (a
    /// wall between you), `level` (0 to 1, near voice fades with distance). Until this
    /// client plays voice, the sound itself is dropped here.
    #[func]
    fn voices(&mut self) -> Array<VarDictionary> {
        let Some(c) = &mut self.client else { return Array::new() };
        let now = Instant::now();
        for h in c.take_voice() {
            self.heard.insert(h.from, Heard { channel: h.channel, muffled: h.muffled, gain: h.gain, at: now });
        }
        self.heard.retain(|_, h| now - h.at < HEARD_FOR);
        let mut out = Array::new();
        for (id, h) in &self.heard {
            let Some(s) = c.scores.iter().find(|s| s.id == *id) else { continue };
            let channel = if h.channel == VoiceChannel::Team { "team" } else { "near" };
            out.push(&vdict! {
                "id" => *id as i64, "name" => s.name.as_str(), "team" => side(s.team), "channel" => channel,
                "muffled" => h.muffled, "level" => h.gain as f32 / 255.0,
            });
        }
        out
    }

    /// Grenades in the air (`kind`: he, flash, smoke; `at`) and smoke clouds (`at`, the
    /// cloud's centre; `left`, its seconds), Godot metres.
    #[func]
    fn grenades(&self) -> Array<VarDictionary> {
        let Some(c) = &self.client else { return Array::new() };
        let flying = c.grenades.iter().map(|(kind, at)| {
            let kind = match kind {
                sim::grenades::Kind::He => "he",
                sim::grenades::Kind::Flash => "flash",
                sim::grenades::Kind::Smoke => "smoke",
            };
            vdict! { "kind" => kind, "at" => to_godot(*at) }
        });
        // the cloud sits a little above where the grenade lies (as the server's sight check)
        let clouds = c.smokes.iter().map(|s| vdict! { "kind" => "cloud", "at" => to_godot([s.at[0], s.at[1], s.at[2] + 40.0]), "left" => s.left });
        flying.chain(clouds).collect()
    }

    /// The buy menu for your side: `category`, `name`, `key` (for `buy`), `price`, and `cost`
    /// now (-1: nothing to buy, you have it or carry the most allowed).
    #[func]
    fn shop(&self) -> Array<VarDictionary> {
        let Some(c) = &self.client else { return Array::new() };
        crate::shop::layout(c.team())
            .into_iter()
            .map(|(cat, name, item)| {
                let cost = net::server::buy::cost_now(item, &c.state.loadout, c.vitals.armor, c.vitals.helmet, c.me_info.defuse_kit);
                vdict! {
                    "category" => cat, "name" => name, "key" => crate::shop::key(item),
                    "price" => net::server::buy::price(item).unwrap_or(0) as i64, "cost" => cost.map_or(-1, |p| p as i64),
                }
            })
            .collect()
    }

    /// Buy an item by its key (from `shop`); the server says yes or no (money, time, place).
    #[func]
    fn buy(&mut self, key: GString) {
        if let (Some(c), Some(item)) = (&mut self.client, crate::shop::parse(&key.to_string())) {
            c.buy(item);
        }
    }
}
