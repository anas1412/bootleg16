//! One player's complete per-command step (movement, then weapons), shared by the
//! server and the client's prediction so both compute exactly the same thing.
use crate::collision::World;
use crate::pmove::{self, Events, Input, MoveVars, Player};
use crate::weapons::{Context, Fired, Loadout};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerState {
    pub movement: Player,
    pub loadout: Loadout,
}

/// What the round allows a player right now (sent to the client for its prediction).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    /// Can't move (freeze time, defusing); looking around is fine.
    pub frozen: bool,
    /// Can't shoot, swing, throw or plant (freeze time, round over).
    pub no_attack: bool,
}

/// Run one input. `cmd_time` is this player's own command clock (input number x tick
/// length), which all weapon timers use, so a client replaying its inputs gets the
/// same timings as the server. `seed` drives spread and recoil randomness.
#[allow(clippy::too_many_arguments)]
pub fn step(s: &mut PlayerState, input: &Input, dt: f32, cmd_time: f64, seed: u64, limits: Limits, world: &World, vars: &MoveVars) -> (Events, Fired) {
    s.movement.max_speed = s.loadout.max_speed();
    let frozen_input;
    let input = if limits.frozen {
        frozen_input = Input { forward: 0.0, side: 0.0, jump: false, ..*input };
        &frozen_input
    } else {
        input
    };
    tagging(&mut s.movement, dt);
    let events = pmove::step(&mut s.movement, input, dt, world, vars);
    let ctx = Context {
        now: cmd_time,
        dt,
        on_ground: s.movement.on_ground,
        crouched: s.movement.crouched,
        speed_2d: s.movement.speed_2d(),
        yaw: input.yaw,
        pitch: input.pitch,
        attack: input.attack,
        attack2: input.attack2,
        reload: input.reload,
        slot: input.slot,
        drop: input.drop,
        can_shoot: !limits.no_attack,
        in_bomb_site: world.zones.in_bomb_site(s.movement.origin),
        seed,
    };
    let fired = s.loadout.update(&ctx);
    (events, fired)
}

/// Seed for one player's command: unique per player and input number.
pub fn seed(player: u8, seq: u32) -> u64 {
    ((player as u64) << 32) | seq as u64
}

/// The slowdown from being shot wearing off (CS 1.6 `CBasePlayer::PreThink`): on the ground,
/// each server frame the modifier gains 0.01 and scales the velocity. CS's servers ran ~100
/// frames a second; at our tick rate the same happens per 1/100 s.
pub fn tagging(p: &mut Player, dt: f32) {
    if p.on_ground && p.velocity_modifier < 1.0 {
        let frames = dt * 100.0;
        p.velocity_modifier = (p.velocity_modifier + 0.01 * frames).min(1.0);
        let k = p.velocity_modifier.powf(frames);
        p.velocity = p.velocity.map(|v| v * k);
    }
}

/// What a bullet hit does to the victim, as CS 1.6 (`TakeDamage`, `TraceAttack`, `ShouldDoLargeFlinch`):
/// slowdown 0.65 for a "large flinch" (standing, not a leg hit, from a rifle, sniper, M3 or
/// Deagle), else 0.5; and the view kicked up when the hit part had no armor.
/// `roll` is a random number in -1..1 for the head-shot tilt.
pub fn hit_by(s: &mut PlayerState, weapon: crate::weapons::Id, group: crate::hitbox::HitGroup, damage: f32, unarmored: bool, roll: f32) {
    use crate::hitbox::HitGroup::*;
    use crate::weapons::Id::*;
    let large_gun = matches!(weapon, Scout | Aug | Sg550 | Galil | Famas | Awp | M3 | M4a1 | G3sg1 | Deagle | Sg552 | Ak47);
    let large = !s.movement.crouched && !matches!(group, LeftLeg | RightLeg) && large_gun;
    s.movement.velocity_modifier = if large { 0.65 } else { 0.5 };
    if !unarmored {
        return;
    }
    let punch = &mut s.loadout.punch;
    match group {
        Head => {
            punch[0] = (damage * -0.5).max(-12.0);
            punch[2] = (damage * roll).clamp(-9.0, 9.0);
        }
        Chest | Stomach => punch[0] = (damage * -0.1).max(-4.0),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hitbox::HitGroup;
    use crate::weapons::{Id, Loadout};

    fn state() -> PlayerState {
        let mut m = Player::new([0.0; 3]);
        m.on_ground = true;
        PlayerState { movement: m, loadout: Loadout::knife_only() }
    }

    #[test]
    fn being_shot_slows_you_and_it_wears_off_in_half_a_second() {
        let mut s = state();
        hit_by(&mut s, Id::Ak47, HitGroup::Chest, 36.0, true, 0.0);
        assert_eq!(s.movement.velocity_modifier, 0.65, "AK to the chest standing: large flinch");
        assert!((s.loadout.punch[0] + 3.6).abs() < 1e-5, "chest, no armor: kick 36 x -0.1");
        s.movement.velocity = [250.0, 0.0, 0.0];
        tagging(&mut s.movement, 1.0 / 128.0);
        assert!(s.movement.velocity[0] < 250.0 * 0.75, "slowed: {:?}", s.movement.velocity);
        for _ in 0..64 {
            tagging(&mut s.movement, 1.0 / 128.0);
        }
        assert_eq!(s.movement.velocity_modifier, 1.0, "recovered after ~0.5 s");
        // Pistols and leg hits are the small flinch; a helmet stops the head kick.
        let mut s = state();
        hit_by(&mut s, Id::Glock, HitGroup::LeftLeg, 25.0, true, 0.0);
        assert_eq!((s.movement.velocity_modifier, s.loadout.punch[0]), (0.5, 0.0));
        hit_by(&mut s, Id::Usp, HitGroup::Head, 120.0, false, 0.5);
        assert_eq!(s.loadout.punch[0], 0.0);
        hit_by(&mut s, Id::Usp, HitGroup::Head, 120.0, true, 0.5);
        assert_eq!(s.loadout.punch, [-12.0, 0.0, 9.0]);
    }
}
