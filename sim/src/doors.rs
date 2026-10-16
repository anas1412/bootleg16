//! Doors (`func_door`, `func_door_rotating`), buttons (`func_button`, `func_rot_button`) and
//! the touch zones that open them (`trigger_multiple`). Behaviour follows GoldSrc's door
//! rules: named doors only open when triggered, unnamed ones open on touch, use-only doors
//! open with +use, rotating doors swing away from whoever opened them, and a closing door
//! that hits a player goes back up. A button moves in when used (or touched, if it's
//! touch-only), fires its target once it's in, and comes back out after `wait`.
//! +use picks as CS 1.6's `PlayerUse` does: the usable thing most in front of you, within
//! 64 units of your centre and 45 degrees of your view.
use crate::{add, dot, length, scale, sub, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Spawnflags shared by both door types.
const SF_START_OPEN: u32 = 1;
const SF_TOGGLE: u32 = 32; // stays open until activated again
const SF_USE_ONLY: u32 = 256;
// Rotating-door spawnflags.
const SF_REVERSE: u32 = 2;
const SF_ONE_WAY: u32 = 16;
const SF_ROTATE_X: u32 = 64;
const SF_ROTATE_Y: u32 = 128;

// Button spawnflags (toggle is the same bit as the doors' `SF_TOGGLE`).
const SF_BUTTON_DONTMOVE: u32 = 1;
const SF_BUTTON_TOUCH_ONLY: u32 = 256;

/// `PlayerUse`: `MAX_PLAYER_USE_RADIUS` (your centre to the thing's) and `VIEW_FIELD_NARROW`.
const USE_RANGE: f32 = 64.0;
const USE_CONE: f32 = 0.7;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub enum DoorState {
    Closed,
    Opening,
    /// Open; closes when the timer (seconds) runs out, unless it never auto-closes.
    Open(f32),
    Closing,
}

#[derive(Clone, Debug)]
pub struct Door {
    /// Index into the map's brush models (for rendering and collision).
    pub model: usize,
    pub name: Option<String>,
    /// Rotation pivot (rotating doors) or zero.
    pub pivot: Vec3,
    /// Model bounds in model space.
    pub mins: Vec3,
    pub maxs: Vec3,
    rotating: bool,
    /// Linear: offset when fully open. Rotating: angles (pitch, yaw, roll) when fully open.
    open_delta: Vec3,
    speed: f32,
    /// Seconds to stay open; negative = never close on its own.
    wait: f32,
    flags: u32,
    /// 0 = closed .. 1 = open.
    progress: f32,
    /// A button: what it fires when pressed in.
    button: Option<Button>,
    /// Rotating doors: +1 or -1, picked so the door swings away from the opener.
    sign: f32,
    pub state: DoorState,
}

impl Door {
    /// Current offset from the closed position, and current rotation angles (degrees).
    pub fn transform(&self) -> (Vec3, Vec3) {
        let d = scale(self.open_delta, self.progress * self.sign);
        if self.rotating {
            ([0.0; 3], d)
        } else {
            (d, [0.0; 3])
        }
    }

    /// World-space bounding box at the current position (conservative while rotating).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let (off, ang) = self.transform();
        let origin = add(self.pivot, off);
        let (f, r, u) = angle_vectors(ang);
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for i in 0..8 {
            let c = [
                if i & 1 == 0 { self.mins[0] } else { self.maxs[0] },
                if i & 2 == 0 { self.mins[1] } else { self.maxs[1] },
                if i & 4 == 0 { self.mins[2] } else { self.maxs[2] },
            ];
            let w = add(origin, local_to_world(c, f, r, u));
            for k in 0..3 {
                lo[k] = lo[k].min(w[k]);
                hi[k] = hi[k].max(w[k]);
            }
        }
        (lo, hi)
    }

    fn travel(&self) -> f32 {
        length(self.open_delta).max(1.0)
    }

    fn activate(&mut self, opener: Option<Vec3>) {
        if self.button.is_some() {
            // `ButtonUse`: not while moving; pressed in, only a toggle button comes back out.
            match self.state {
                DoorState::Closed => self.state = DoorState::Opening,
                DoorState::Open(t) if self.flags & SF_TOGGLE != 0 && t.is_infinite() && self.wait >= 0.0 => self.state = DoorState::Closing,
                _ => {}
            }
            return;
        }
        match self.state {
            DoorState::Open(_) if self.flags & SF_TOGGLE != 0 || self.wait < 0.0 => self.state = DoorState::Closing,
            DoorState::Open(_) | DoorState::Opening => {}
            DoorState::Closed | DoorState::Closing => {
                if self.rotating && self.state == DoorState::Closed && self.flags & SF_ONE_WAY == 0 {
                    if let Some(p) = opener {
                        self.sign = self.swing_away_from(p);
                    }
                }
                self.state = DoorState::Opening;
            }
        }
    }

    /// Opened with +use: use-only doors, and buttons that aren't touch-only.
    fn usable(&self) -> bool {
        match self.button {
            Some(_) => self.flags & SF_BUTTON_TOUCH_ONLY == 0,
            None => self.flags & SF_USE_ONLY != 0,
        }
    }

    /// Opened by walking into it: unnamed doors that aren't use-only, touch-only buttons.
    fn touch_opens(&self) -> bool {
        match self.button {
            Some(_) => self.flags & SF_BUTTON_TOUCH_ONLY != 0,
            None => self.name.is_none() && self.flags & SF_USE_ONLY == 0,
        }
    }

    /// Pick the rotation direction that moves the door's far edge away from `p`.
    fn swing_away_from(&self, p: Vec3) -> f32 {
        let centre = scale(add(self.mins, self.maxs), 0.5); // model space = relative to pivot
        let to_player = sub(p, self.pivot);
        let base = if self.flags & SF_REVERSE != 0 { -1.0 } else { 1.0 };
        // Positive yaw turns the panel counter-clockwise, towards the panel's left side.
        let cross = centre[0] * to_player[1] - centre[1] * to_player[0];
        if cross > 0.0 {
            -base
        } else {
            base
        }
    }
}

/// The part of a door that changes during play (sent from server to clients).
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct DoorNet {
    pub progress: f32,
    pub sign: f32,
    pub state: DoorState,
}

/// What a button fires (by name: doors open), after `delay`.
#[derive(Clone, Debug)]
struct Button {
    target: String,
    delay: f32,
}

/// A `trigger_multiple`: touching it fires its target (after `delay`), then rests `wait` seconds.
#[derive(Clone, Debug)]
struct Trigger {
    mins: Vec3,
    maxs: Vec3,
    target: String,
    wait: f32,
    delay: f32,
    ready_at: f32,
}

/// What a player is doing this tick, for doors and triggers.
#[derive(Clone, Copy, Debug)]
pub struct Actor {
    pub mins: Vec3,
    pub maxs: Vec3,
    pub eye: Vec3,
    /// View direction (unit vector).
    pub forward: Vec3,
    /// +use pressed this tick.
    pub using: bool,
}

#[derive(Default)]
pub struct Doors {
    pub doors: Vec<Door>,
    triggers: Vec<Trigger>,
    /// (time, target name) waiting to fire.
    pending: Vec<(f32, String)>,
    time: f32,
}

fn vec3(s: Option<&String>) -> Vec3 {
    let v: Vec<f32> = s.map(|s| s.split_whitespace().filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    [v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)]
}

fn num(e: &HashMap<String, String>, k: &str, default: f32) -> f32 {
    e.get(k).and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// GoldSrc "angle" key: -1 = up, -2 = down, else a yaw in degrees.
fn move_dir(e: &HashMap<String, String>) -> Vec3 {
    let yaw = e.get("angle").and_then(|v| v.parse::<f32>().ok()).unwrap_or_else(|| vec3(e.get("angles"))[1]);
    if yaw == -1.0 {
        [0.0, 0.0, 1.0]
    } else if yaw == -2.0 {
        [0.0, 0.0, -1.0]
    } else {
        let (s, c) = yaw.to_radians().sin_cos();
        [c, s, 0.0]
    }
}

/// GoldSrc AngleVectors: forward, right, up for (pitch, yaw, roll) in degrees.
pub fn angle_vectors(a: Vec3) -> (Vec3, Vec3, Vec3) {
    let (sp, cp) = a[0].to_radians().sin_cos();
    let (sy, cy) = a[1].to_radians().sin_cos();
    let (sr, cr) = a[2].to_radians().sin_cos();
    (
        [cp * cy, cp * sy, -sp],
        [-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp],
        [cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp],
    )
}

pub fn world_to_local(v: Vec3, f: Vec3, r: Vec3, u: Vec3) -> Vec3 {
    [dot(v, f), -dot(v, r), dot(v, u)]
}

pub fn local_to_world(l: Vec3, f: Vec3, r: Vec3, u: Vec3) -> Vec3 {
    add(add(scale(f, l[0]), scale(r, -l[1])), scale(u, l[2]))
}

fn overlaps(amin: Vec3, amax: Vec3, bmin: Vec3, bmax: Vec3) -> bool {
    (0..3).all(|i| amin[i] <= bmax[i] && amax[i] >= bmin[i])
}

impl Doors {
    pub fn from_bsp(bsp: &goldsrc::bsp::Bsp) -> Doors {
        let mut out = Doors::default();
        for e in bsp.entity_list() {
            let class = e.get("classname").map(String::as_str).unwrap_or("");
            let Some(model) = e.get("model").and_then(|m| m.strip_prefix('*')?.parse::<usize>().ok()) else { continue };
            let Some(m) = bsp.models.get(model) else { continue };
            let flags = num(&e, "spawnflags", 0.0) as u32;
            match class {
                "trigger_multiple" => out.triggers.push(Trigger {
                    mins: m.mins,
                    maxs: m.maxs,
                    target: e.get("target").cloned().unwrap_or_default(),
                    wait: num(&e, "wait", 0.2),
                    delay: num(&e, "delay", 0.0),
                    ready_at: 0.0,
                }),
                "func_door" | "func_door_rotating" | "func_button" | "func_rot_button" => {
                    let rotating = matches!(class, "func_door_rotating" | "func_rot_button");
                    let button = matches!(class, "func_button" | "func_rot_button");
                    // Buttons: `lip` 4, speed 40 and wait 1 when not set (0); `wait` -1 stays in.
                    let or = |v: f32, d: f32| if v == 0.0 { d } else { v };
                    let lip = if button { or(num(&e, "lip", 0.0), 4.0) } else { num(&e, "lip", 0.0) };
                    let open_delta = if rotating {
                        let dist = num(&e, "distance", 90.0);
                        if flags & SF_ROTATE_X != 0 {
                            [0.0, 0.0, dist]
                        } else if flags & SF_ROTATE_Y != 0 {
                            [dist, 0.0, 0.0]
                        } else {
                            [0.0, dist, 0.0]
                        }
                    } else {
                        // Move by the model's size along the direction, minus `lip`.
                        let dir = move_dir(&e);
                        let size = sub(m.maxs, m.mins);
                        let span = (dir[0] * (size[0] - 2.0)).abs() + (dir[1] * (size[1] - 2.0)).abs() + (dir[2] * (size[2] - 2.0)).abs();
                        if button && (span - lip < 1.0 || flags & SF_BUTTON_DONTMOVE != 0) {
                            [0.0; 3]
                        } else {
                            scale(dir, span - lip)
                        }
                    };
                    let mut door = Door {
                        model,
                        name: e.get("targetname").cloned(),
                        pivot: vec3(e.get("origin")),
                        mins: m.mins,
                        maxs: m.maxs,
                        rotating,
                        open_delta,
                        speed: if button { or(num(&e, "speed", 0.0), 40.0) } else { num(&e, "speed", 100.0) }.max(1.0),
                        wait: if button { or(num(&e, "wait", 0.0), 1.0) } else { num(&e, "wait", 3.0) },
                        // A button's flags only share the toggle and rotation bits with doors'.
                        flags: if button { flags & (SF_TOGGLE | SF_REVERSE | SF_ROTATE_X | SF_ROTATE_Y | SF_BUTTON_TOUCH_ONLY) } else { flags },
                        progress: 0.0,
                        button: button.then(|| Button { target: e.get("target").cloned().unwrap_or_default(), delay: num(&e, "delay", 0.0) }),
                        sign: if rotating && flags & SF_REVERSE != 0 { -1.0 } else { 1.0 },
                        state: DoorState::Closed,
                    };
                    if !button && flags & SF_START_OPEN != 0 {
                        door.progress = 1.0;
                        door.state = DoorState::Open(f32::INFINITY);
                    }
                    out.doors.push(door);
                }
                _ => {}
            }
        }
        out
    }

    pub fn net_state(&self) -> Vec<DoorNet> {
        self.doors.iter().map(|d| DoorNet { progress: d.progress, sign: d.sign, state: d.state }).collect()
    }

    /// Adopt the server's door positions (same map, so same door order).
    pub fn apply_net_state(&mut self, s: &[DoorNet]) {
        for (d, n) in self.doors.iter_mut().zip(s) {
            d.progress = n.progress;
            d.sign = n.sign;
            d.state = n.state;
        }
    }

    /// Index of the door using this brush model, if it is one.
    pub fn door_for_model(&self, model: usize) -> Option<usize> {
        self.doors.iter().position(|d| d.model == model)
    }

    /// Advance doors and triggers by `dt`. `blocked(door, actor)` reports whether a
    /// door at its current position overlaps that player (supplied by the world).
    pub fn update(&mut self, dt: f32, actors: &[Actor], blocked: impl Fn(&Door, &Actor) -> bool) {
        self.time += dt;
        let now = self.time;

        // Touch zones.
        for t in &mut self.triggers {
            if now >= t.ready_at && actors.iter().any(|a| overlaps(a.mins, a.maxs, t.mins, t.maxs)) {
                self.pending.push((now + t.delay, t.target.clone()));
                t.ready_at = now + t.wait;
            }
        }
        // Fire due targets.
        let (due, later): (Vec<_>, Vec<_>) = self.pending.drain(..).partition(|(t, _)| *t <= now);
        self.pending = later;
        for (_, name) in due {
            for d in self.doors.iter_mut().filter(|d| d.name.as_deref() == Some(name.as_str())) {
                d.activate(None);
            }
        }

        // Players touching or using doors and buttons.
        for a in actors {
            let centre = scale(add(a.mins, a.maxs), 0.5);
            if a.using {
                // `PlayerUse`: the usable thing whose centre is most in front of you, in reach.
                let hit = self
                    .doors
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| d.usable())
                    .filter_map(|(i, d)| {
                        let (lo, hi) = d.bounds();
                        let mid = scale(add(lo, hi), 0.5);
                        let to = sub(mid, a.eye);
                        let facing = dot(to, a.forward) / length(to).max(1e-3);
                        (length(sub(mid, centre)) <= USE_RANGE && facing > USE_CONE).then_some((i, facing))
                    })
                    .max_by(|x, y| x.1.total_cmp(&y.1));
                if let Some((i, _)) = hit {
                    self.doors[i].activate(Some(centre));
                }
            }
            for d in self.doors.iter_mut().filter(|d| d.touch_opens()) {
                let (lo, hi) = d.bounds();
                let grow = |v: Vec3, s: f32| add(v, [s; 3]);
                if matches!(d.state, DoorState::Closed | DoorState::Closing) && overlaps(a.mins, a.maxs, grow(lo, -1.0), grow(hi, 1.0)) {
                    d.activate(Some(centre));
                }
            }
        }

        // Motion.
        for d in &mut self.doors {
            let step = d.speed * dt / d.travel();
            match d.state {
                DoorState::Opening => {
                    d.progress = (d.progress + step).min(1.0);
                    if d.progress >= 1.0 {
                        d.state = DoorState::Open(if d.wait < 0.0 || d.flags & SF_TOGGLE != 0 { f32::INFINITY } else { d.wait });
                        // `TriggerAndWait`: a button pressed all the way in fires its target.
                        if let Some(b) = &d.button {
                            self.pending.push((now + b.delay, b.target.clone()));
                        }
                    }
                }
                DoorState::Open(t) => {
                    if t.is_finite() {
                        let left = t - dt;
                        d.state = if left <= 0.0 { DoorState::Closing } else { DoorState::Open(left) };
                    }
                }
                DoorState::Closing => {
                    let before = d.progress;
                    d.progress = (d.progress - step).max(0.0);
                    if d.button.is_none() && actors.iter().any(|a| blocked(d, a)) {
                        // Hit a player: go back up instead of crushing them.
                        d.progress = before;
                        d.state = DoorState::Opening;
                    } else if d.progress <= 0.0 {
                        d.state = DoorState::Closed;
                        // `ButtonBackHome`: a toggle button fires again when it comes back out.
                        if let Some(b) = d.button.as_ref().filter(|_| d.flags & SF_TOGGLE != 0) {
                            self.pending.push((now + b.delay, b.target.clone()));
                        }
                    }
                }
                DoorState::Closed => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel(model: usize, name: Option<&str>, button: Option<&str>, at: Vec3, open_delta: Vec3) -> Door {
        Door {
            model,
            name: name.map(String::from),
            pivot: at,
            mins: [-2.0, -8.0, -8.0],
            maxs: [2.0, 8.0, 8.0],
            rotating: false,
            open_delta,
            speed: 40.0,
            wait: 1.0,
            flags: 0,
            progress: 0.0,
            sign: 1.0,
            state: DoorState::Closed,
            button: button.map(|t| Button { target: t.into(), delay: 0.0 }),
        }
    }

    /// Someone standing `back` units in front of the button (at x = 0), looking along `forward`.
    fn actor(back: f32, forward: Vec3, using: bool) -> Actor {
        let o = [-back, 0.0, 36.0];
        Actor { mins: add(o, [-16.0, -16.0, -36.0]), maxs: add(o, [16.0, 16.0, 36.0]), eye: add(o, [0.0, 0.0, 17.0]), forward, using }
    }

    #[test]
    fn a_button_pressed_with_use_opens_its_door_then_pops_back_out() {
        let mut d = Doors { doors: vec![panel(1, None, Some("gate"), [0.0, 0.0, 50.0], [4.0, 0.0, 0.0]), panel(2, Some("gate"), None, [300.0, 0.0, 50.0], [0.0, 0.0, 100.0])], ..Default::default() };
        let tick = |d: &mut Doors, a: Actor| d.update(0.01, &[a], |_, _| false);
        // Looking away, or too far: nothing.
        tick(&mut d, actor(40.0, [-1.0, 0.0, 0.0], true));
        tick(&mut d, actor(90.0, [1.0, 0.0, 0.0], true));
        assert_eq!(d.doors[0].state, DoorState::Closed);
        // Facing it, close enough: it goes in (4 units at 40/s), fires, and the gate opens.
        tick(&mut d, actor(40.0, [1.0, 0.0, 0.0], true));
        assert_eq!(d.doors[0].state, DoorState::Opening);
        for _ in 0..12 {
            tick(&mut d, actor(40.0, [1.0, 0.0, 0.0], false));
        }
        assert!(matches!(d.doors[0].state, DoorState::Open(_)), "{:?}", d.doors[0].state);
        assert_eq!(d.doors[1].state, DoorState::Opening, "the button fired its target");
        // Pressing again while it's in does nothing; after `wait` (1 s) it comes back out.
        tick(&mut d, actor(40.0, [1.0, 0.0, 0.0], true));
        for _ in 0..120 {
            tick(&mut d, actor(40.0, [1.0, 0.0, 0.0], false));
        }
        assert_eq!(d.doors[0].state, DoorState::Closed);
        // Touching a use-button does nothing.
        let mut touch = actor(0.0, [1.0, 0.0, 0.0], false);
        touch.mins[0] = -2.0;
        tick(&mut d, touch);
        assert_eq!(d.doors[0].state, DoorState::Closed);
    }
}
