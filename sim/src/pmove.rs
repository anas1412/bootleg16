//! CS 1.6 player movement: friction, ground/air acceleration, stepping, jumping
//! with stamina, the bunny-hop speed cap, and crouching. Clean-room; constants
//! are read from the game's own `cs.so` (see docs/MOVEMENT.md).
use crate::collision::{Contents, Hull, Ladder, World};
use crate::{add, dot, length, scale, sub, Vec3};
use serde::{Deserialize, Serialize};

/// Server movement settings. Defaults are what CS 1.6 forces every round.
#[derive(Clone, Copy, Debug)]
pub struct MoveVars {
    pub gravity: f32,
    pub stop_speed: f32,
    pub accelerate: f32,
    pub air_accelerate: f32,
    pub friction: f32,
    pub edge_friction: f32,
    pub step_size: f32,
    pub max_velocity: f32,
    pub water_accelerate: f32,
    pub water_friction: f32,
}

impl Default for MoveVars {
    fn default() -> Self {
        MoveVars {
            gravity: 800.0,
            stop_speed: 75.0,
            accelerate: 5.0,
            air_accelerate: 10.0,
            friction: 4.0,
            edge_friction: 2.0,
            step_size: 18.0,
            max_velocity: 2000.0,
            water_accelerate: 10.0,
            water_friction: 1.0,
        }
    }
}

/// Jump speed for a 45-unit jump at 800 gravity: sqrt(2 * 800 * 45).
const JUMP_SPEED: f32 = 268.328_16;
/// Stamina set by a jump; it drains by 1000 per second and slows the next jump/landing.
const JUMP_STAMINA: f32 = 1315.789;
/// Bunny-hop cap: jumping above 1.2x max speed scales you down to 0.8 of that cap.
const BHOP_CAP: f32 = 1.2;
const BHOP_PENALTY: f32 = 0.8;
const CROUCH_SPEED: f32 = 0.333;
/// Air control: wish speed is capped at 30 for air acceleration.
const AIR_WISH_CAP: f32 = 30.0;
/// Surfaces steeper than this (normal z below it) are walls, not floors.
const FLOOR_NORMAL_Z: f32 = 0.7;
/// Moving up faster than this means you are not on the ground.
const LIFT_OFF_SPEED: f32 = 180.0;
/// Origin difference between the standing and crouched hulls (box centres).
const CROUCH_OFFSET: f32 = 18.0;
/// Shift-walk multiplier: client cvar `cl_movespeedkey` (client.so).
const WALK_SPEED: f32 = 0.52;
/// `PM_UpdateStepSound`: at this speed or slower no footsteps (and the next waits
/// `FOOTSTEP_QUIET_MS`); faster, one every `FOOTSTEP_MS` (ladders, wading knee-deep slower),
/// `FOOTSTEP_SLOW_MS` more crouched or on a ladder.
const FOOTSTEP_MIN_SPEED: f32 = 150.0;
const FOOTSTEP_QUIET_MS: f32 = 400.0;
const FOOTSTEP_MS: f32 = 300.0;
const FOOTSTEP_LADDER_MS: f32 = 350.0;
const FOOTSTEP_WADE_MS: f32 = 600.0;
const FOOTSTEP_SLOW_MS: f32 = 100.0;
/// Seconds to crouch on the ground (`PM_Duck`).
const TIME_TO_DUCK: f32 = 0.4;
/// Ladders (`PM_LadderMove`).
const LADDER_CLIMB_SPEED: f32 = 200.0;
const LADDER_JUMP_SPEED: f32 = 270.0;
/// Water (`PM_WaterMove`, `PM_Jump`). Swimming is at 0.8x max speed.
const WATER_SPEED: f32 = 0.8;
const WATER_SINK_SPEED: f32 = 60.0;
const WATER_SWIM_UP_SPEED: f32 = 100.0;
/// Falling (`PM_CheckFalling`, `CBasePlayer::PostThink`, `FlPlayerFallDamage`).
const LAND_SPEED: f32 = 250.0;
const SAFE_FALL_SPEED: f32 = 500.0;
const FALL_DAMAGE_PER_SPEED: f32 = 100.0 / 600.0 * 1.25;

pub const VIEW_HEIGHT_STANDING: f32 = 17.0;
pub const VIEW_HEIGHT_CROUCHED: f32 = 12.0;

pub const SLOT_LAST: u8 = 6;
pub const SLOT_NEXT: u8 = 7;
pub const SLOT_PREV: u8 = 8;

/// One tick of player input.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Input {
    /// Forward and sideways move (units/s; CS clients send ±400).
    pub forward: f32,
    pub side: f32,
    /// View yaw in degrees (GoldSrc: 0 = +X, counter-clockwise).
    pub yaw: f32,
    /// View pitch in degrees (GoldSrc: positive looks down). Used on ladders and in water.
    pub pitch: f32,
    pub jump: bool,
    pub crouch: bool,
    /// Shift: walk slowly (0.52x), quiet enough to make no footsteps.
    pub walk: bool,
    /// E (+use) held: opens use-only doors on the press.
    pub use_key: bool,
    /// Mouse 1 / mouse 2 / R held.
    pub attack: bool,
    pub attack2: bool,
    pub reload: bool,
    /// Weapon selection this tick: 1 primary .. 5 bomb, `SLOT_LAST` (Q),
    /// `SLOT_NEXT` / `SLOT_PREV` (mouse wheel); 0 = none.
    pub slot: u8,
    /// G: drop the weapon in hand.
    pub drop: bool,
}

/// Full movement state; the server sends it so clients can correct their prediction.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Player {
    pub origin: Vec3,
    pub velocity: Vec3,
    /// Fully crouched (small hull).
    pub crouched: bool,
    pub on_ground: bool,
    pub on_ladder: bool,
    /// 0 = dry, 1 = feet, 2 = waist (swimming), 3 = head under.
    pub water_level: u8,
    /// Weapon-dependent max speed (250 knife/pistols, 221 AK...).
    pub max_speed: f32,
    /// Eye height above the origin; animates while crouching.
    pub view_height: f32,
    stamina: f32,
    jump_held: bool,
    /// Crouch is held but the 0.4 s crouch hasn't finished yet.
    ducking: bool,
    /// Counts down from 1000 ms while crouching.
    duck_time: f32,
    /// Downward speed while airborne, for landing and fall damage.
    fall_velocity: f32,
    /// Milliseconds until the next footstep may play.
    step_timer: f32,
    /// Getting shot slows you (CS 1.6 `m_flVelocityModifier`): 0.5 or 0.65 after a hit,
    /// back to 1 over about half a second while on the ground (`player::tagging`).
    pub velocity_modifier: f32,
    /// Carrying a body (`bodies`): slower.
    pub carrying: bool,
}

/// Things that happened during a step, for the game rules to act on.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Events {
    /// Landed hard enough for a landing sound/view punch.
    pub landed: bool,
    /// Health to subtract (0 when the fall was safe).
    pub fall_damage: f32,
    /// A landing loud enough to be heard rolls the view this many degrees (`PM_CheckFalling`).
    pub land_roll: f32,
    /// A footstep sound plays now (other players hear it).
    pub footstep: Option<Step>,
}

/// Where a footstep is (`PM_UpdateStepSound`): on the floor (its texture picks the sound),
/// on a ladder, wading knee-deep or sloshing ankle-deep.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepKind {
    Surface,
    Ladder,
    Wade,
    Slosh,
}

/// A footstep (`PM_PlayStepSound`, always `ATTN_NORM`).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub kind: StepKind,
    /// None: the surface's own volume (concrete 0.5, dirt 0.55, vent 0.7, ...).
    pub volume: Option<f32>,
    /// Crouched: 0.35 times as loud.
    pub crouched: bool,
}

impl Step {
    /// The volume to play it at, given the surface's own (`materials.txt` letter).
    pub fn loudness(&self, material: char) -> f32 {
        let own = match material {
            'D' => 0.55,
            'V' => 0.7,
            _ => 0.5,
        };
        self.volume.unwrap_or(own) * if self.crouched { 0.35 } else { 1.0 }
    }
}

/// `PM_CheckFalling`'s landing sound (`PM_PLAYER_MIN_BOUNCE_SPEED`, `PM_PLAYER_MAX_SAFE_FALL_SPEED`).
const LAND_MIN_SOUND: f32 = 350.0;
const LAND_LOUD: f32 = 580.0;

impl Player {
    pub fn new(origin: Vec3) -> Player {
        Player {
            origin,
            velocity: [0.0; 3],
            crouched: false,
            on_ground: false,
            on_ladder: false,
            water_level: 0,
            max_speed: 250.0,
            view_height: VIEW_HEIGHT_STANDING,
            stamina: 0.0,
            jump_held: false,
            ducking: false,
            duck_time: 0.0,
            fall_velocity: 0.0,
            step_timer: 0.0,
            velocity_modifier: 1.0,
            carrying: false,
        }
    }

    pub fn hull(&self) -> Hull {
        if self.crouched {
            Hull::Crouched
        } else {
            Hull::Standing
        }
    }

    pub fn eye(&self) -> Vec3 {
        add(self.origin, [0.0, 0.0, self.view_height])
    }

    /// Horizontal speed, as shown on speed meters.
    pub fn speed_2d(&self) -> f32 {
        (self.velocity[0].powi(2) + self.velocity[1].powi(2)).sqrt()
    }
}

/// GoldSrc view vectors (roll is always 0 for movement).
fn view_vectors(yaw: f32, pitch: f32) -> (Vec3, Vec3) {
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    ([cp * cy, cp * sy, -sp], [sy, -cy, 0.0])
}

/// Advance one player by `dt` seconds.
pub fn step(p: &mut Player, input: &Input, dt: f32, world: &World, vars: &MoveVars) -> Events {
    let mut ev = Events::default();
    let ms = dt * 1000.0;
    p.stamina = (p.stamina - ms).max(0.0);
    p.duck_time = (p.duck_time - ms).max(0.0);
    let was_on_ground = p.on_ground;
    categorize(p, world);
    check_water(p, world);
    if !p.on_ground {
        p.fall_velocity = -p.velocity[2];
    }

    duck(p, input, world);

    // The client clamps input to the weapon's max speed before crouching scales it.
    let (mut fmove, mut smove) = (input.forward, input.side);
    let len = (fmove * fmove + smove * smove).sqrt();
    if len > p.max_speed {
        fmove *= p.max_speed / len;
        smove *= p.max_speed / len;
    }
    if input.walk {
        fmove *= WALK_SPEED;
        smove *= WALK_SPEED;
    }
    if p.crouched || p.ducking {
        fmove *= CROUCH_SPEED;
        smove *= CROUCH_SPEED;
    }

    let jump_pressed = input.jump && !p.jump_held;
    p.jump_held = input.jump;

    p.on_ladder = world.ladder_at(p.hull(), p.origin).is_some();
    if let Some(ladder) = world.ladder_at(p.hull(), p.origin).copied() {
        ladder_move(p, input, &ladder, dt, world);
    } else if p.water_level >= 2 {
        if input.jump {
            p.velocity[2] = WATER_SWIM_UP_SPEED;
        }
        water_move(p, input, fmove, smove, dt, world, vars);
    } else {
        if jump_pressed && p.on_ground {
            jump(p);
            // `PM_Jump`: jumping off at 150 or faster thuds at full volume.
            if p.speed_2d() >= FOOTSTEP_MIN_SPEED {
                ev.footstep = Some(Step { kind: StepKind::Surface, volume: Some(1.0), crouched: false });
            }
        }
        // Gravity is applied in two halves around the move (more accurate arcs).
        if !p.on_ground {
            p.velocity[2] -= vars.gravity * 0.5 * dt;
        } else {
            p.velocity[2] = 0.0;
            friction(p, dt, world, vars);
        }

        let (fwd, right) = view_vectors(input.yaw, 0.0);
        let wish = add(scale(fwd, fmove), scale(right, smove));
        let wish_speed = length(wish).min(p.max_speed);
        let wish_dir = if wish_speed > 0.0 { scale(wish, 1.0 / length(wish)) } else { [0.0; 3] };
        if p.on_ground {
            accelerate(p, wish_dir, wish_speed, vars.accelerate, dt);
            clamp_velocity(p, vars);
            walk_move(p, dt, world, vars);
        } else {
            air_accelerate(p, wish_dir, wish_speed, vars.air_accelerate, dt);
            clamp_velocity(p, vars);
            fly_move(p, dt, world);
        }
        categorize(p, world);
        if !p.on_ground {
            p.velocity[2] -= vars.gravity * 0.5 * dt;
        } else {
            p.velocity[2] = 0.0;
        }
    }
    clamp_velocity(p, vars);
    categorize(p, world);
    check_water(p, world);

    if p.on_ground && !was_on_ground && p.fall_velocity >= LAND_SPEED {
        ev.landed = true;
        // In this order, as CS has it (so 0.5 is only heard in water).
        let volume = match p.fall_velocity {
            _ if p.water_level > 0 => 0.5,
            v if v > LAND_LOUD => 1.0,
            v if v > LAND_LOUD / 2.0 => 0.85,
            v if v < LAND_MIN_SOUND => 0.0,
            _ => 0.5,
        };
        if volume > 0.0 {
            ev.footstep = Some(Step { kind: StepKind::Surface, volume: Some(volume), crouched: false });
            p.step_timer = FOOTSTEP_MS;
            ev.land_roll = p.fall_velocity * 0.013;
        }
        if p.water_level == 0 && p.fall_velocity > SAFE_FALL_SPEED {
            ev.fall_damage = (p.fall_velocity - SAFE_FALL_SPEED) * FALL_DAMAGE_PER_SPEED;
        }
    }
    if p.on_ground || p.on_ladder || p.water_level >= 2 {
        p.fall_velocity = 0.0;
    }
    p.step_timer = (p.step_timer - ms).max(0.0);
    if p.step_timer == 0.0 {
        if crate::length(p.velocity) <= FOOTSTEP_MIN_SPEED {
            p.step_timer = FOOTSTEP_QUIET_MS;
        } else if p.on_ground || p.on_ladder {
            let h = p.hull();
            let height = h.maxs()[2] - h.mins()[2];
            let wet = |z: f32| world.point_contents([p.origin[0], p.origin[1], p.origin[2] - z * height]) == Contents::Water;
            let (kind, volume, ms) = if p.on_ladder {
                (StepKind::Ladder, Some(0.35), FOOTSTEP_LADDER_MS)
            } else if wet(0.3) {
                (StepKind::Wade, Some(0.65), FOOTSTEP_WADE_MS)
            } else if wet(0.5) {
                (StepKind::Slosh, Some(0.5), FOOTSTEP_MS)
            } else {
                (StepKind::Surface, None, FOOTSTEP_MS)
            };
            p.step_timer = ms + if p.crouched || p.on_ladder { FOOTSTEP_SLOW_MS } else { 0.0 };
            ev.footstep = Some(Step { kind, volume, crouched: p.crouched && !p.on_ladder });
        }
    }
    ev
}

/// Water depth from three probe points: feet, box centre, eyes.
fn check_water(p: &mut Player, world: &World) {
    let wet = |z: f32| world.point_contents([p.origin[0], p.origin[1], z]) == Contents::Water;
    let feet = p.origin[2] + p.hull().mins()[2] + 1.0;
    let mid = p.origin[2] + (p.hull().mins()[2] + p.hull().maxs()[2]) * 0.5;
    p.water_level = if !wet(feet) {
        0
    } else if !wet(mid) {
        1
    } else if !wet(p.origin[2] + p.view_height) {
        2
    } else {
        3
    };
}

fn clamp_velocity(p: &mut Player, vars: &MoveVars) {
    for v in &mut p.velocity {
        *v = v.clamp(-vars.max_velocity, vars.max_velocity);
    }
}

/// On the ground = a walkable floor within 2 units below, and not flying upward.
fn categorize(p: &mut Player, world: &World) {
    if p.velocity[2] > LIFT_OFF_SPEED {
        p.on_ground = false;
        return;
    }
    let t = world.trace(p.hull(), p.origin, sub(p.origin, [0.0, 0.0, 2.0]));
    p.on_ground = t.fraction < 1.0 && t.normal[2] >= FLOOR_NORMAL_Z && !t.all_solid;
    if p.on_ground && !t.start_solid {
        p.origin = t.end; // snap down onto the floor
    }
}

fn jump(p: &mut Player) {
    // Bunny-hop cap: arriving too fast costs speed.
    let max = BHOP_CAP * p.max_speed;
    let spd = length(p.velocity);
    if spd > max {
        p.velocity = scale(p.velocity, max / spd * BHOP_PENALTY);
    }
    p.velocity[2] = JUMP_SPEED;
    if p.stamina > 0.0 {
        p.velocity[2] *= stamina_factor(p.stamina);
    }
    p.stamina = JUMP_STAMINA;
    p.on_ground = false;
}

/// 1.0 when rested, lower right after a jump (CS 1.6 formula).
fn stamina_factor(stamina: f32) -> f32 {
    (100.0 - stamina * 0.001 * 19.0) * 0.01
}

/// Smooth 0..1 ease used for the crouch view animation.
fn spline_fraction(t: f32, scale: f32) -> f32 {
    let v = (t * scale).clamp(0.0, 1.0);
    3.0 * v * v - 2.0 * v * v * v
}

/// Crouching takes 0.4 s on the ground (instant in the air): the eye lowers
/// smoothly, then the hull shrinks. Grounded players keep their feet in place;
/// airborne players pull their legs up (the classic crouch-jump).
fn duck(p: &mut Player, input: &Input, world: &World) {
    if input.crouch {
        if !p.crouched && !p.ducking {
            p.ducking = true;
            p.duck_time = 1000.0;
        }
        if p.ducking {
            let elapsed = (1000.0 - p.duck_time) / 1000.0;
            if elapsed >= TIME_TO_DUCK || !p.on_ground {
                if p.on_ground {
                    p.origin[2] -= CROUCH_OFFSET;
                }
                p.crouched = true;
                p.ducking = false;
                p.view_height = VIEW_HEIGHT_CROUCHED;
            } else {
                // Eye height relative to the still-standing origin: 17 down to 12 - 18.
                let f = spline_fraction(elapsed, 1.0 / TIME_TO_DUCK);
                p.view_height = (VIEW_HEIGHT_CROUCHED - CROUCH_OFFSET) * f + VIEW_HEIGHT_STANDING * (1.0 - f);
            }
        }
    } else if p.ducking {
        p.ducking = false;
        p.view_height = VIEW_HEIGHT_STANDING;
    } else if p.crouched {
        // Stand up only if there is room.
        let up = add(p.origin, [0.0, 0.0, if p.on_ground { CROUCH_OFFSET } else { 0.0 }]);
        if !world.is_solid(Hull::Standing, up) {
            p.origin = up;
            p.crouched = false;
            p.view_height = VIEW_HEIGHT_STANDING;
        }
    }
}

/// Climbing: no gravity; look direction and keys move you along the ladder face.
fn ladder_move(p: &mut Player, input: &Input, ladder: &Ladder, dt: f32, world: &World) {
    // The ladder's face normal: its thin horizontal axis, pointing at the player.
    let size = sub(ladder.maxs, ladder.mins);
    let centre = scale(add(ladder.mins, ladder.maxs), 0.5);
    let axis = if size[0] < size[1] { 0 } else { 1 };
    let mut normal = [0.0; 3];
    normal[axis] = if p.origin[axis] >= centre[axis] { 1.0 } else { -1.0 };

    if input.jump {
        p.velocity = scale(normal, LADDER_JUMP_SPEED);
        p.on_ladder = false;
        fly_move(p, dt, world);
        return;
    }
    let mut climb = LADDER_CLIMB_SPEED;
    if p.crouched || p.ducking {
        climb *= CROUCH_SPEED;
    }
    let forward = climb * input.forward.signum() * (input.forward != 0.0) as i32 as f32;
    let right = climb * input.side.signum() * (input.side != 0.0) as i32 as f32;
    if forward == 0.0 && right == 0.0 {
        p.velocity = [0.0; 3];
        return;
    }
    let (vpn, vright) = view_vectors(input.yaw, input.pitch);
    let vel = add(scale(vpn, forward), scale(vright, right));
    // Split into "into the ladder" and "along it"; pushing into it climbs up.
    let up = [0.0, 0.0, 1.0];
    let perp = {
        let c = [up[1] * normal[2] - up[2] * normal[1], up[2] * normal[0] - up[0] * normal[2], up[0] * normal[1] - up[1] * normal[0]];
        scale(c, 1.0 / length(c).max(1e-6))
    };
    let normal_comp = dot(vel, normal);
    let lateral = sub(vel, scale(normal, normal_comp));
    let tmp = [normal[1] * perp[2] - normal[2] * perp[1], normal[2] * perp[0] - normal[0] * perp[2], normal[0] * perp[1] - normal[1] * perp[0]];
    p.velocity = add(lateral, scale(tmp, -normal_comp));
    // Walking backwards off a ladder that reaches the floor.
    let feet = add(p.origin, [0.0, 0.0, p.hull().mins()[2] - 1.0]);
    if world.point_contents(feet) == Contents::Solid && normal_comp > 0.0 {
        p.velocity = add(p.velocity, scale(normal, LADDER_CLIMB_SPEED));
    }
    fly_move(p, dt, world);
}

/// Swimming: 3D movement along the view direction, slow sinking, heavy drag.
fn water_move(p: &mut Player, input: &Input, fmove: f32, smove: f32, dt: f32, world: &World, vars: &MoveVars) {
    let (vpn, vright) = view_vectors(input.yaw, input.pitch);
    let mut wish = add(scale(vpn, fmove), scale(vright, smove));
    if fmove == 0.0 && smove == 0.0 && !input.jump {
        wish[2] -= WATER_SINK_SPEED;
    }
    let mut wish_speed = length(wish);
    let wish_dir = if wish_speed > 0.0 { scale(wish, 1.0 / wish_speed) } else { [0.0; 3] };
    wish_speed = wish_speed.min(p.max_speed) * WATER_SPEED;

    let speed = length(p.velocity);
    let new_speed = if speed > 0.0 { (speed - dt * speed * vars.friction * vars.water_friction).max(0.0) } else { 0.0 };
    if speed > 0.0 {
        p.velocity = scale(p.velocity, new_speed / speed);
    }
    if wish_speed >= 0.1 {
        let add_speed = wish_speed - new_speed;
        if add_speed > 0.0 {
            let a = (vars.water_accelerate * wish_speed * dt).min(add_speed);
            p.velocity = add(p.velocity, scale(wish_dir, a));
        }
    }
    fly_move(p, dt, world);
}

fn friction(p: &mut Player, dt: f32, world: &World, vars: &MoveVars) {
    let speed = length(p.velocity);
    if speed < 0.1 {
        return;
    }
    // Edge friction: stronger when the ground ends just ahead (stops you sliding off ledges).
    let ahead = add(p.origin, scale(p.velocity, 16.0 / speed));
    let start = [ahead[0], ahead[1], p.origin[2] + p.hull().mins()[2]];
    let mut fric = vars.friction;
    if world.trace(p.hull(), start, sub(start, [0.0, 0.0, 34.0])).fraction == 1.0 {
        fric *= vars.edge_friction;
    }
    let control = speed.max(vars.stop_speed);
    let new_speed = (speed - control * fric * dt).max(0.0);
    p.velocity = scale(p.velocity, new_speed / speed);
}

fn accelerate(p: &mut Player, dir: Vec3, wish_speed: f32, accel: f32, dt: f32) {
    let add_speed = wish_speed - dot(p.velocity, dir);
    if add_speed <= 0.0 {
        return;
    }
    let a = (accel * dt * wish_speed).min(add_speed);
    p.velocity = add(p.velocity, scale(dir, a));
}

/// Air strafing: the projected speed cap is tiny (30), but acceleration uses the full
/// wish speed, so turning while strafing gains speed.
fn air_accelerate(p: &mut Player, dir: Vec3, wish_speed: f32, accel: f32, dt: f32) {
    let add_speed = wish_speed.min(AIR_WISH_CAP) - dot(p.velocity, dir);
    if add_speed <= 0.0 {
        return;
    }
    let a = (accel * wish_speed * dt).min(add_speed);
    p.velocity = add(p.velocity, scale(dir, a));
}

/// Ground move with stair stepping: try the move flat, and also stepped up by
/// `step_size` then back down; keep whichever went further.
fn walk_move(p: &mut Player, dt: f32, world: &World, vars: &MoveVars) {
    if p.stamina > 0.0 {
        let f = stamina_factor(p.stamina);
        p.velocity[0] *= f;
        p.velocity[1] *= f;
    }
    p.velocity[2] = 0.0;
    if length(p.velocity) < 1.0 {
        p.velocity = [0.0; 3];
        return;
    }
    let dest = add(p.origin, scale(p.velocity, dt));
    let t = world.trace(p.hull(), p.origin, dest);
    if t.fraction == 1.0 {
        p.origin = t.end;
        return;
    }

    let (orig, ovel) = (p.origin, p.velocity);
    fly_move(p, dt, world);
    let (down_origin, down_vel) = (p.origin, p.velocity);

    p.origin = orig;
    p.velocity = ovel;
    let up = world.trace(p.hull(), p.origin, add(p.origin, [0.0, 0.0, vars.step_size]));
    if !up.start_solid && !up.all_solid {
        p.origin = up.end;
    }
    fly_move(p, dt, world);
    let down = world.trace(p.hull(), p.origin, sub(p.origin, [0.0, 0.0, vars.step_size]));
    if !down.start_solid && !down.all_solid {
        p.origin = down.end;
    }
    let flat_dist = (down_origin[0] - orig[0]).powi(2) + (down_origin[1] - orig[1]).powi(2);
    let step_dist = (p.origin[0] - orig[0]).powi(2) + (p.origin[1] - orig[1]).powi(2);
    if down.normal[2] < FLOOR_NORMAL_Z || flat_dist > step_dist {
        p.origin = down_origin;
        p.velocity = down_vel;
    } else {
        p.velocity[2] = down_vel[2];
    }
}

fn clip_velocity(v: Vec3, normal: Vec3) -> Vec3 {
    let back = dot(v, normal);
    let mut out = sub(v, scale(normal, back));
    for c in &mut out {
        if c.abs() < 0.1 {
            *c = 0.0;
        }
    }
    out
}

/// Slide along whatever is hit, up to 4 times per move.
fn fly_move(p: &mut Player, dt: f32, world: &World) {
    let primal = p.velocity;
    let mut original = p.velocity;
    let mut planes: Vec<Vec3> = Vec::with_capacity(5);
    let mut time_left = dt;

    for _ in 0..4 {
        if p.velocity == [0.0; 3] {
            break;
        }
        let end = add(p.origin, scale(p.velocity, time_left));
        let t = world.trace(p.hull(), p.origin, end);
        if t.all_solid {
            p.velocity = [0.0; 3];
            return;
        }
        if t.fraction > 0.0 {
            p.origin = t.end;
            original = p.velocity;
            planes.clear();
        }
        if t.fraction == 1.0 {
            break;
        }
        time_left -= time_left * t.fraction;
        if planes.len() >= 5 {
            p.velocity = [0.0; 3];
            break;
        }
        planes.push(t.normal);

        if !p.on_ground {
            // In the air: clip against each plane in turn (this is what makes surfing work).
            for &n in &planes {
                p.velocity = clip_velocity(original, n);
                original = p.velocity;
            }
        } else {
            // On the ground: find a direction that slides along every plane hit.
            let mut found = false;
            for i in 0..planes.len() {
                let v = clip_velocity(original, planes[i]);
                if planes.iter().enumerate().all(|(j, &n)| j == i || dot(v, n) >= 0.0) {
                    p.velocity = v;
                    found = true;
                    break;
                }
            }
            if !found {
                if planes.len() != 2 {
                    p.velocity = [0.0; 3];
                    break;
                }
                // Slide along the crease of two planes.
                let (a, b) = (planes[0], planes[1]);
                let crease = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
                p.velocity = scale(crease, dot(crease, p.velocity));
            }
        }
        if dot(p.velocity, primal) <= 0.0 {
            p.velocity = [0.0; 3];
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 128.0;

    fn run(p: &mut Player, input: Input, secs: f32, w: &World) {
        for _ in 0..(secs / DT) as usize {
            step(p, &input, DT, w, &MoveVars::default());
        }
    }

    fn standing_on_floor() -> (Player, World) {
        let w = World::flat_floor(0.0);
        let mut p = Player::new([0.0, 0.0, 100.0]);
        run(&mut p, Input::default(), 1.0, &w);
        (p, w)
    }

    #[test]
    fn falls_and_lands_on_floor() {
        let (p, _) = standing_on_floor();
        assert!(p.on_ground);
        assert!((p.origin[2] - 36.0).abs() < 0.1, "{:?}", p.origin);
    }

    #[test]
    fn running_reaches_and_keeps_max_speed() {
        let (mut p, w) = standing_on_floor();
        run(&mut p, Input { forward: 400.0, ..Default::default() }, 2.0, &w);
        let s = p.speed_2d();
        assert!((s - 250.0).abs() < 2.0, "speed {s}");
    }

    #[test]
    fn you_step_over_a_body_but_a_tall_box_stops_you() {
        let (mut p, mut w) = standing_on_floor();
        let floor = p.origin[2] - 36.0;
        // a body lying across the way, 12 high: you walk over it
        w.boxes.push(([60.0, -40.0, floor], [100.0, 40.0, floor + 12.0]));
        run(&mut p, Input { forward: 400.0, ..Default::default() }, 1.0, &w);
        assert!(p.origin[0] > 150.0, "stuck at {:?}", p.origin);
        // a wall-high box: you don't
        let (mut p, mut w) = standing_on_floor();
        w.boxes.push(([60.0, -40.0, floor], [100.0, 40.0, floor + 80.0]));
        run(&mut p, Input { forward: 400.0, ..Default::default() }, 1.0, &w);
        assert!(p.origin[0] < 60.0 - 15.0, "went through to {:?}", p.origin);
    }

    #[test]
    fn friction_stops_a_runner() {
        let (mut p, w) = standing_on_floor();
        run(&mut p, Input { forward: 400.0, ..Default::default() }, 2.0, &w);
        run(&mut p, Input::default(), 1.0, &w);
        assert_eq!(p.speed_2d(), 0.0);
    }

    #[test]
    fn jump_reaches_45_units() {
        let (mut p, w) = standing_on_floor();
        let base = p.origin[2];
        let mut top = base;
        step(&mut p, &Input { jump: true, ..Default::default() }, DT, &w, &MoveVars::default());
        for _ in 0..128 {
            step(&mut p, &Input::default(), DT, &w, &MoveVars::default());
            top = top.max(p.origin[2]);
        }
        assert!((top - base - 45.0).abs() < 1.0, "jump height {}", top - base);
        assert!(p.on_ground);
    }

    #[test]
    fn crouching_slows_and_lowers_view() {
        let (mut p, w) = standing_on_floor();
        run(&mut p, Input { forward: 400.0, crouch: true, ..Default::default() }, 2.0, &w);
        assert!(p.crouched);
        assert!((p.speed_2d() - 250.0 * CROUCH_SPEED).abs() < 2.0, "{}", p.speed_2d());
        assert!((p.eye()[2] - (18.0 + VIEW_HEIGHT_CROUCHED)).abs() < 0.1);
        run(&mut p, Input::default(), 0.1, &w);
        assert!(!p.crouched && (p.origin[2] - 36.0).abs() < 0.1);
    }

    #[test]
    fn bhop_speed_is_capped() {
        let (mut p, w) = standing_on_floor();
        p.velocity = [400.0, 0.0, 0.0];
        step(&mut p, &Input { jump: true, ..Default::default() }, DT, &w, &MoveVars::default());
        assert!(p.speed_2d() <= 250.0 * BHOP_CAP * BHOP_PENALTY + 1.0, "{}", p.speed_2d());
    }

    #[test]
    fn same_inputs_same_result() {
        let w = World::flat_floor(0.0);
        let sim = || {
            let mut p = Player::new([0.0, 0.0, 100.0]);
            for i in 0..512 {
                let input = Input { forward: 400.0, side: if i % 64 < 32 { 400.0 } else { -400.0 }, yaw: i as f32, jump: i % 50 == 0, ..Default::default() };
                step(&mut p, &input, DT, &w, &MoveVars::default());
            }
            (p.origin, p.velocity)
        };
        assert_eq!(sim(), sim());
    }

    #[test]
    fn crouch_takes_point_four_seconds_on_ground() {
        let (mut p, w) = standing_on_floor();
        let crouch = Input { crouch: true, ..Default::default() };
        run(&mut p, crouch, 0.2, &w);
        assert!(!p.crouched && p.view_height < VIEW_HEIGHT_STANDING, "mid-crouch eye {}", p.view_height);
        run(&mut p, crouch, 0.25, &w);
        assert!(p.crouched && p.view_height == VIEW_HEIGHT_CROUCHED);
    }

    #[test]
    fn fall_damage_matches_cs() {
        let w = World::flat_floor(0.0);
        let mut events = Vec::new();
        for height in [100.0, 300.0, 1000.0] {
            let mut p = Player::new([0.0, 0.0, 36.0 + height]);
            let mut dmg = 0.0;
            for _ in 0..1024 {
                dmg += step(&mut p, &Input::default(), DT, &w, &MoveVars::default()).fall_damage;
            }
            events.push(dmg);
        }
        // 100 units: safe. 300 units: ~693 u/s at landing -> ~40 HP. 1000 units: lethal.
        assert_eq!(events[0], 0.0);
        assert!((events[1] - 40.0).abs() < 3.0, "{events:?}");
        assert!(events[2] > 100.0, "{events:?}");
    }

    #[test]
    fn a_heard_landing_rolls_the_view() {
        let w = World::flat_floor(0.0);
        let roll = |height: f32| {
            let mut p = Player::new([0.0, 0.0, 36.0 + height]);
            (0..1024).map(|_| step(&mut p, &Input::default(), DT, &w, &MoveVars::default()).land_roll).fold(0.0, f32::max)
        };
        // 20 units: too soft to hear, no roll. 300 units: ~693 u/s, 0.013 a unit/s.
        assert_eq!(roll(20.0), 0.0);
        assert!((roll(300.0) - 9.0).abs() < 0.5, "{}", roll(300.0));
    }

    #[test]
    fn shift_walk_is_slow_and_silent() {
        let (mut p, w) = standing_on_floor();
        let walk = Input { forward: 400.0, walk: true, ..Default::default() };
        let mut steps = 0;
        for _ in 0..256 {
            steps += step(&mut p, &walk, DT, &w, &MoveVars::default()).footstep.is_some() as i32;
        }
        assert!((p.speed_2d() - 130.0).abs() < 1.0, "{}", p.speed_2d());
        assert_eq!(steps, 0);
        let run_input = Input { forward: 400.0, ..Default::default() };
        for _ in 0..256 {
            steps += step(&mut p, &run_input, DT, &w, &MoveVars::default()).footstep.is_some() as i32;
        }
        // One every 300 ms once over 150 (after up to 400 ms from walking): 6 in 2 s.
        assert!((5..=7).contains(&steps), "running made {steps} footsteps in 2 s");
        // Crouch-running is under 150: silent.
        let mut quiet = 0;
        for _ in 0..256 {
            quiet += step(&mut p, &Input { crouch: true, ..run_input }, DT, &w, &MoveVars::default()).footstep.is_some() as i32;
        }
        assert!(p.crouched);
        assert!(quiet <= 1, "{quiet} footsteps crouched");
    }
}
