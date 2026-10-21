//! How a player's model is animated, the way CS 1.6 does it, shared by the client (drawing)
//! and the server (posing hitboxes, so shots hit where players are drawn):
//! - Legs (`CBasePlayer::SetAnimation`'s gait): idle1 when still, walk up to 135 u/s, run
//!   above; crouched crouch_idle / crouchrun; `jump` after leaving the ground. They face
//!   where they move (`StudioEstimateGait`) and step as far as the player moved
//!   (`StudioProcessGait`: the sequence's own linear movement). Standing still the legs
//!   stay put until the view is more than 90 degrees off, then catch up.
//! - Upper body: the weapon's aim pose, or a one-off action (shoot, reload, flinch), from the
//!   3x3 grid of yaw (view against legs, `CalculateYawBlend`; moving more than 120 degrees
//!   off, the legs turn round and step backwards) and pitch (view pitch over +-45 degrees,
//!   `CalculatePitchBlend`); a new upper sequence fades in over 0.2 s.
use crate::hitbox::HitGroup;
use crate::weapons::{data::anim_ext, Id};
use goldsrc::mdl::{BoneTransform, Model};

/// What drives a player's animation this instant.
#[derive(Clone, Copy, Debug)]
pub struct Drive {
    pub origin: [f32; 3],
    /// Where they look (GoldSrc degrees; pitch down-positive).
    pub yaw: f32,
    pub pitch: f32,
    pub crouched: bool,
    pub airborne: bool,
    pub reloading: bool,
    pub weapon: Id,
}

/// A one-off upper-body animation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Shoot,
    Reload,
    Flinch(HitGroup),
}

#[derive(Clone, Debug)]
pub struct Anim {
    /// Which way the legs face (degrees).
    pub gait_yaw: f32,
    gait_frame: f32,
    /// How far they moved over the last step (negative: the legs step backwards), and the
    /// ground speed then.
    movement: f32,
    speed: f32,
    last_origin: Option<[f32; 3]>,
    /// How fast the still legs catch up with the view (CS's `fuser1`: 0.05 once within 5
    /// degrees, 3.5 once past 90), and the time not yet stepped at 100 Hz.
    turn_rate: f32,
    turn_time: f32,
    /// When they left the ground.
    jumped: Option<f32>,
    action: Option<(Action, f32)>,
    reloading: bool,
    /// The upper-body sequence last drawn, at what frame, and the one it is fading in from:
    /// ((sequence, frame), when it changed).
    upper: Option<(usize, f32)>,
    fade: Option<((usize, f32), f32)>,
}

/// What `pose` computed: bone transforms (model space) and the yaw the model faces.
#[derive(Debug)]
pub struct Posed {
    pub bones: Vec<BoneTransform>,
    pub yaw: f32,
}

fn wrap(a: f32) -> f32 {
    (a + 540.0).rem_euclid(360.0) - 180.0
}

/// CS's aim pitch blend (0..1 for its 0..255 byte): view pitch -45 (up) to 45 (down).
pub fn pitch_blend(pitch: f32) -> f32 {
    let p = pitch.trunc();
    if p <= -45.0 {
        0.0
    } else if p >= 45.0 {
        1.0
    } else {
        ((45.0 + p) * 255.0 / 90.0).trunc() / 255.0
    }
}

/// CS's aim yaw blend (0..1): the view `twist` degrees to the left of the legs; walking,
/// shifted 26 steps further right (`StudioSetupBones`).
fn yaw_blend(twist: f32, walking: bool) -> f32 {
    let v = twist / 90.0 * 128.0 + 127.0;
    let mut b = if v >= 255.0 { 0 } else if v <= 0.0 { 255 } else { (255.0 - v) as i32 };
    if walking {
        b = (b - 26).max(0);
    }
    b as f32 / 255.0
}

/// How long a new upper-body sequence fades in from the last one.
const FADE: f32 = 0.2;
/// Walking (shift) or running: CS's gait switch.
const RUN_SPEED: f32 = 135.0;

impl Anim {
    pub fn new(yaw: f32) -> Anim {
        Anim { gait_yaw: yaw, gait_frame: 0.0, movement: 0.0, speed: 0.0, last_origin: None, turn_rate: 0.0, turn_time: 0.0, jumped: None, action: None, reloading: false, upper: None, fade: None }
    }

    /// They fired (the shoot animation starts again; not while flinching, as in CS).
    pub fn shot(&mut self, now: f32) {
        if !matches!(self.action, Some((Action::Flinch(_), _))) {
            self.action = Some((Action::Shoot, now));
        }
    }

    /// They were hit.
    pub fn hurt(&mut self, group: HitGroup, now: f32) {
        self.action = Some((Action::Flinch(group), now));
    }

    /// Advance by `dt` seconds (at time `now`).
    pub fn step(&mut self, model: &Model, d: &Drive, now: f32, dt: f32) {
        let dt = dt.clamp(1e-4, 1.0);
        let last = self.last_origin.unwrap_or(d.origin);
        let delta = [d.origin[0] - last[0], d.origin[1] - last[1], d.origin[2] - last[2]];
        self.last_origin = Some(d.origin);
        let moved = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt();
        self.speed = (delta[0] * delta[0] + delta[1] * delta[1]).sqrt() / dt;
        if moved / dt >= 5.0 && (delta[0] != 0.0 || delta[1] != 0.0) {
            self.movement = moved;
            self.gait_yaw = delta[1].atan2(delta[0]).to_degrees();
            self.turn_time = 0.0;
            // Moving more than 120 degrees from the view: face the other way, step backwards.
            let twist = wrap(d.yaw - self.gait_yaw);
            if twist.abs() > 120.0 {
                self.gait_yaw = wrap(self.gait_yaw + 180.0);
                self.movement = -moved;
            }
        } else {
            self.movement = 0.0;
            // ponytail: stepped at a fixed 100 Hz (CS at fps_max 100) so the server's
            // hitboxes and every client's frame rate agree; CS itself steps once per frame.
            self.turn_time += dt;
            while self.turn_time >= 0.01 {
                self.turn_time -= 0.01;
                let diff = wrap(d.yaw - self.gait_yaw);
                if diff.abs() < 5.0 {
                    self.turn_rate = 0.05;
                }
                if diff.abs() > 90.0 {
                    self.turn_rate = 3.5;
                }
                let turn = 0.01 * self.turn_rate * diff;
                if turn.trunc().abs() >= 0.1 {
                    self.gait_yaw = wrap(self.gait_yaw + turn);
                }
            }
        }
        match (d.airborne, self.jumped) {
            (true, None) => {
                self.jumped = Some(now);
                self.gait_frame = 0.0;
            }
            (false, Some(_)) => self.jumped = None,
            _ => {}
        }
        let gs = &model.sequences[self.gait(model, d)];
        let frames = gs.frames.max(1) as f32;
        self.gait_frame += if gs.linear_x > 0.0 { frames * self.movement / gs.linear_x } else { dt * gs.fps };
        self.gait_frame = self.gait_frame.rem_euclid(frames);
        if d.reloading && !self.reloading {
            self.action = Some((Action::Reload, now));
        }
        self.reloading = d.reloading;
        let upper = self.upper_seq(model, d, now);
        if let Some(last) = self.upper.filter(|l| l.0 != upper.0) {
            self.fade = Some((last, now));
        }
        self.upper = Some(upper);
    }

    /// The gait sequence for their speed and stance.
    fn gait(&self, model: &Model, d: &Drive) -> usize {
        let name = match (self.jumped.is_some(), d.crouched, self.movement != 0.0) {
            (true, _, _) => "jump",
            (_, true, true) => "crouchrun",
            (_, true, false) => "crouch_idle",
            (_, false, true) if self.speed > RUN_SPEED => "run",
            (_, false, true) => "walk",
            _ => "idle1",
        };
        model.sequence(name).unwrap_or(0)
    }

    /// The upper-body (sequence, frame) now: the weapon's aim pose, or the action playing.
    fn upper_seq(&mut self, model: &Model, d: &Drive, now: f32) -> (usize, f32) {
        let stance = if d.crouched { "crouch" } else { "ref" };
        let ext = anim_ext(d.weapon);
        let aim = model.sequence(&format!("{stance}_aim_{ext}")).or_else(|| model.sequence(&format!("{stance}_aim_carbine"))).unwrap_or(0);
        if let Some((a, t0)) = self.action {
            let name = match a {
                Action::Shoot => format!("{stance}_shoot_{ext}"),
                Action::Reload => format!("{stance}_reload_{ext}"),
                // CS: head and chest hits jerk the head back, the rest double over.
                Action::Flinch(HitGroup::Head | HitGroup::Chest) => "head_flinch".into(),
                Action::Flinch(_) => "gut_flinch".into(),
            };
            match model.sequence(&name) {
                Some(s) if (now - t0) * model.sequences[s].fps < model.sequences[s].frames as f32 - 1.0 => return (s, (now - t0) * model.sequences[s].fps),
                _ => self.action = None,
            }
        }
        (aim, 0.0)
    }

    /// The pose now.
    pub fn pose(&mut self, model: &Model, d: &Drive, now: f32) -> Posed {
        let gait = self.gait(model, d);
        let upper = self.upper_seq(model, d, now);
        let prev = self.fade.filter(|f| now - f.1 < FADE && f.0 .0 != upper.0).map(|(seq, t)| (seq, 1.0 - (now - t) / FADE));
        let walking = model.sequence("walk") == Some(gait);
        let blend = [yaw_blend(wrap(d.yaw - self.gait_yaw), walking), pitch_blend(d.pitch)];
        Posed { bones: model.player_pose((gait, self.gait_frame), upper, blend, prev), yaw: self.gait_yaw }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blends_are_cs_bytes() {
        assert_eq!(pitch_blend(0.0), 127.0 / 255.0);
        assert_eq!((pitch_blend(-45.0), pitch_blend(-89.0), pitch_blend(45.0), pitch_blend(89.0)), (0.0, 0.0, 1.0, 1.0));
        assert_eq!(yaw_blend(0.0, false), 128.0 / 255.0);
        assert_eq!((yaw_blend(90.0, false), yaw_blend(-90.0, false)), (0.0, 1.0));
        assert_eq!(yaw_blend(0.0, true), 102.0 / 255.0);
    }
}
