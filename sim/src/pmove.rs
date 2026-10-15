//! CS 1.6 player movement: friction, ground/air acceleration, stepping, jumping
//! with stamina, the bunny-hop speed cap, and crouching. Clean-room; constants
//! are read from the game's own `cs.so` (see docs/MOVEMENT.md).
use crate::collision::{Hull, World};
use crate::{add, dot, length, scale, sub, Vec3};

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

pub const VIEW_HEIGHT_STANDING: f32 = 17.0;
pub const VIEW_HEIGHT_CROUCHED: f32 = 12.0;

/// One tick of player input.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Forward and sideways move (units/s; CS clients send ±400).
    pub forward: f32,
    pub side: f32,
    /// View yaw in degrees (GoldSrc: 0 = +X, counter-clockwise).
    pub yaw: f32,
    pub jump: bool,
    pub crouch: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Player {
    pub origin: Vec3,
    pub velocity: Vec3,
    pub crouched: bool,
    pub on_ground: bool,
    /// Weapon-dependent max speed (250 knife/pistols, 221 AK...).
    pub max_speed: f32,
    stamina: f32,
    jump_held: bool,
}

impl Player {
    pub fn new(origin: Vec3) -> Player {
        Player { origin, velocity: [0.0; 3], crouched: false, on_ground: false, max_speed: 250.0, stamina: 0.0, jump_held: false }
    }

    pub fn hull(&self) -> Hull {
        if self.crouched {
            Hull::Crouched
        } else {
            Hull::Standing
        }
    }

    pub fn eye(&self) -> Vec3 {
        let h = if self.crouched { VIEW_HEIGHT_CROUCHED } else { VIEW_HEIGHT_STANDING };
        add(self.origin, [0.0, 0.0, h])
    }

    /// Horizontal speed, as shown on speed meters.
    pub fn speed_2d(&self) -> f32 {
        (self.velocity[0].powi(2) + self.velocity[1].powi(2)).sqrt()
    }
}

/// Advance one player by `dt` seconds.
pub fn step(p: &mut Player, input: &Input, dt: f32, world: &World, vars: &MoveVars) {
    p.stamina = (p.stamina - 1000.0 * dt).max(0.0);
    crouch(p, input, world);
    categorize(p, world);

    // The client clamps input to the weapon's max speed before crouching scales it.
    let (mut fmove, mut smove) = (input.forward, input.side);
    let len = (fmove * fmove + smove * smove).sqrt();
    if len > p.max_speed {
        fmove *= p.max_speed / len;
        smove *= p.max_speed / len;
    }
    if p.crouched {
        fmove *= CROUCH_SPEED;
        smove *= CROUCH_SPEED;
    }

    if input.jump {
        if !p.jump_held && p.on_ground {
            jump(p);
        }
        p.jump_held = true;
    } else {
        p.jump_held = false;
    }

    // Gravity is applied in two halves around the move (more accurate arcs).
    if !p.on_ground {
        p.velocity[2] -= vars.gravity * 0.5 * dt;
    }
    if p.on_ground {
        p.velocity[2] = 0.0;
        friction(p, dt, world, vars);
    }

    let yaw = input.yaw.to_radians();
    let (fwd, right) = ([yaw.cos(), yaw.sin(), 0.0], [yaw.sin(), -yaw.cos(), 0.0]);
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
    clamp_velocity(p, vars);
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

/// Instant crouch. Grounded players keep their feet in place; airborne players pull
/// their legs up (the classic crouch-jump).
fn crouch(p: &mut Player, input: &Input, world: &World) {
    if input.crouch && !p.crouched {
        if p.on_ground {
            p.origin[2] -= CROUCH_OFFSET;
        } else {
            p.origin[2] += CROUCH_OFFSET;
            if world.is_solid(Hull::Crouched, p.origin) {
                p.origin[2] -= CROUCH_OFFSET;
            }
        }
        p.crouched = true;
    } else if !input.crouch && p.crouched {
        // Stand up only if there is room.
        let up = add(p.origin, [0.0, 0.0, if p.on_ground { CROUCH_OFFSET } else { -CROUCH_OFFSET }]);
        if !world.is_solid(Hull::Standing, up) {
            p.origin = up;
            p.crouched = false;
        }
    }
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
    if world.trace(Hull::Standing, add(start, [0.0, 0.0, 36.0]), add(start, [0.0, 0.0, 36.0 - 34.0])).fraction == 1.0 {
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
                let input = Input { forward: 400.0, side: if i % 64 < 32 { 400.0 } else { -400.0 }, yaw: i as f32, jump: i % 50 == 0, crouch: false };
                step(&mut p, &input, DT, &w, &MoveVars::default());
            }
            (p.origin, p.velocity)
        };
        assert_eq!(sim(), sim());
    }
}
