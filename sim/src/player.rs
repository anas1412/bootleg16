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
