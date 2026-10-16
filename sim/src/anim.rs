//! How a player's model is animated, the way CS 1.6 does it, shared by the client (drawing)
//! and the server (posing hitboxes, so shots hit where players are drawn):
//! - Legs: idle / walk / run (crouched: crouch_idle / crouchrun) played by speed, facing where
//!   they move (`StudioEstimateGait`); more than 120 degrees from the view they face the other
//!   way and play backwards; standing still they catch up with the view. In the air: `jump`.
//! - Upper body: the weapon's aim pose at the look pitch, or a one-off action (shoot, reload,
//!   flinch), twisted back towards the view over the spine (`CalculateYawBlend`).
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
    /// Which way the legs face (degrees) and whether the gait plays backwards.
    pub gait_yaw: f32,
    backwards: bool,
    gait_frame: f32,
    /// Ground speed, smoothed.
    speed: f32,
    last_origin: Option<[f32; 3]>,
    /// When they left the ground.
    jumped: Option<f32>,
    action: Option<(Action, f32)>,
    reloading: bool,
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

impl Anim {
    pub fn new(yaw: f32) -> Anim {
        Anim { gait_yaw: yaw, backwards: false, gait_frame: 0.0, speed: 0.0, last_origin: None, jumped: None, action: None, reloading: false }
    }

    /// They fired (the shoot animation starts again).
    pub fn shot(&mut self, now: f32) {
        self.action = Some((Action::Shoot, now));
    }

    /// They were hit.
    pub fn hurt(&mut self, group: HitGroup, now: f32) {
        self.action = Some((Action::Flinch(group), now));
    }

    /// Advance by `dt` seconds (at time `now`).
    pub fn step(&mut self, model: &Model, d: &Drive, now: f32, dt: f32) {
        let dt = dt.max(1e-4);
        let last = self.last_origin.unwrap_or(d.origin);
        let (vx, vy) = ((d.origin[0] - last[0]) / dt, (d.origin[1] - last[1]) / dt);
        self.last_origin = Some(d.origin);
        let moved = (vx * vx + vy * vy).sqrt();
        if moved < 5.0 {
            let diff = wrap(d.yaw - self.gait_yaw);
            self.gait_yaw = wrap(self.gait_yaw + diff * if dt < 0.25 { dt * 4.0 } else { dt });
        } else {
            self.gait_yaw = vy.atan2(vx).to_degrees();
        }
        self.backwards = false;
        let twist = wrap(d.yaw - self.gait_yaw);
        if twist > 120.0 {
            self.gait_yaw = wrap(self.gait_yaw - 180.0);
            self.backwards = true;
        } else if twist < -120.0 {
            self.gait_yaw = wrap(self.gait_yaw + 180.0);
            self.backwards = true;
        }
        self.speed = self.speed * 0.8 + moved.min(400.0) * 0.2;
        let (gait, nominal) = self.gait(model, d);
        let gs = &model.sequences[gait];
        let rate = if nominal > 0.0 { (self.speed / nominal).clamp(0.5, 1.5) } else { 1.0 };
        let step = gs.fps * rate * dt * if self.backwards { -1.0 } else { 1.0 };
        self.gait_frame = (self.gait_frame + step).rem_euclid(gs.frames.max(1) as f32);
        match (d.airborne, self.jumped) {
            (true, None) => self.jumped = Some(now),
            (false, Some(_)) => self.jumped = None,
            _ => {}
        }
        if d.reloading && !self.reloading {
            self.action = Some((Action::Reload, now));
        }
        self.reloading = d.reloading;
    }

    /// The gait sequence for their speed and stance, and the speed it's drawn for.
    fn gait(&self, model: &Model, d: &Drive) -> (usize, f32) {
        let (name, nominal) = match (d.crouched, self.speed) {
            (true, s) if s > 10.0 => ("crouchrun", 85.0),
            (true, _) => ("crouch_idle", 0.0),
            (false, s) if s > 150.0 => ("run", 250.0),
            (false, s) if s > 10.0 => ("walk", 130.0),
            _ => ("idle1", 0.0),
        };
        (model.sequence(name).unwrap_or(0), nominal)
    }

    /// The pose now.
    pub fn pose(&mut self, model: &Model, d: &Drive, now: f32) -> Posed {
        let stance = if d.crouched { "crouch" } else { "ref" };
        let ext = anim_ext(d.weapon);
        let (mut gait, _) = self.gait(model, d);
        let mut gait_frame = self.gait_frame;
        if let (Some(t0), Some(j)) = (self.jumped, model.sequence("jump")) {
            gait = j;
            gait_frame = ((now - t0) * model.sequences[j].fps).min(model.sequences[j].frames as f32 - 1.0);
        }
        let aim = model.sequence(&format!("{stance}_aim_{ext}")).or_else(|| model.sequence(&format!("{stance}_aim_carbine"))).unwrap_or(0);
        let mut upper = (aim, 0.0);
        if let Some((a, t0)) = self.action {
            let name = match a {
                Action::Shoot => format!("{stance}_shoot_{ext}"),
                Action::Reload => format!("{stance}_reload_{ext}"),
                Action::Flinch(HitGroup::Head) => "head_flinch".into(),
                Action::Flinch(_) => "gut_flinch".into(),
            };
            match model.sequence(&name) {
                Some(s) if (now - t0) * model.sequences[s].fps < model.sequences[s].frames as f32 - 1.0 => upper = (s, (now - t0) * model.sequences[s].fps),
                _ => self.action = None,
            }
        }
        // Aim pitch picks the blend (9 sets from looking up to looking down).
        let blend = 0.5 + d.pitch.clamp(-90.0, 90.0) / 180.0;
        let twist = wrap(d.yaw - self.gait_yaw).clamp(-90.0, 90.0);
        Posed { bones: model.player_pose((gait, gait_frame), upper, blend, twist), yaw: self.gait_yaw }
    }
}
