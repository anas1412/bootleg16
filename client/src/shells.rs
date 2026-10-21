//! Spent shells, as CS 1.6 throws them (`EV_GetDefaultShellInfo` and `EV_EjectBrass` in
//! client.so, read with REA): the weapon's shell model leaves the gun to the right with half
//! the shooter's velocity plus 50-90 sideways, 75-175 up and 25-150 forward (your own from
//! closer to the gun, some a little lower), spinning, falls under full gravity, bounces off the world with a
//! tinkle (`pl_shell`, shotgun shells `sshell`) and is gone after 2 s.
use crate::game::{Frame, Walker};
use crate::map;
use crate::menu::AppState;
use crate::models::{self, ModelCache};
use crate::sound::Cue;
use crate::studio::StudioMaterial;
use bevy::prelude::*;
use sim::weapons::Id;
use std::collections::HashMap;

/// Seconds a shell lies around (`R_TempModel`'s life) and gravity (`sv_gravity`).
const LIFE: f32 = 2.0;
const GRAVITY: f32 = 800.0;

pub struct ShellsPlugin;

impl Plugin for ShellsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Shells>().add_systems(Update, (spawn, fly).chain().in_set(Frame::World));
    }
}

/// How a weapon throws its shells (each gun's `EV_Fire*`): the model, where it leaves
/// (forward, up, right of the eye; everyone else's from the same spot), how fast it spins,
/// whether it goes out to the left (`bReverseDirection`), and how much less your own rises.
/// None: no shell when it fires (the knife, grenades). CS throws none for the Scout and AWP;
/// we do.
struct Throw {
    model: &'static str,
    own: [f32; 3],
    spin: f32,
    left: bool,
    drop: f32,
    shotgun: bool,
}

const OTHERS: [f32; 3] = [20.0, -12.0, 4.0];

fn throw(id: Id) -> Option<Throw> {
    use Id::*;
    let t = |model, own, spin, left, drop| Some(Throw { model, own, spin, left, drop, shotgun: false });
    let (p, r) = ("pshell.mdl", "rshell.mdl");
    match id {
        Glock => t(p, [36.0, -14.0, 14.0], 4.0, false, 0.0),
        Usp => t(p, [36.0, -14.0, 14.0], 5.0, false, 0.0),
        P228 => t(p, [36.0, -14.0, 14.0], 4.0, false, 0.0),
        Deagle => t(p, [35.0, -11.0, 16.0], 5.0, false, 0.0),
        Mac10 => t(p, [32.0, -9.0, 11.0], 7.0, false, 0.0),
        Tmp => t(p, [32.0, -6.0, 11.0], 8.0, false, 0.0),
        Mp5 => t(p, [34.0, -10.0, 11.0], 13.0, false, 50.0),
        Ump45 => t(p, [34.0, -10.0, 11.0], 13.0, false, 50.0),
        P90 => t(r, [35.0, -16.0, 22.0], 10.0, true, 0.0),
        Galil => t(r, [20.0, -8.0, 10.0], 9.0, false, 75.0),
        Famas => t(r, [17.0, -8.0, 14.0], 8.0, true, 0.0),
        Ak47 => t(r, [20.0, -8.0, 10.0], 9.0, false, 75.0),
        M4a1 => t(r, [20.0, -8.0, 10.0], 10.0, false, 45.0),
        // Ours, not CS's (it throws none for these): the rifle shell, as the AK.
        Scout | Awp => t(r, [20.0, -8.0, 10.0], 9.0, false, 75.0),
        _ => None,
    }
}

/// Shots to throw shells for this frame: (weapon, eye, forward, shooter velocity, yours).
#[derive(Resource, Default)]
pub struct Shells {
    queue: Vec<(Id, [f32; 3], [f32; 3], [f32; 3], bool)>,
    rng: u64,
}

impl Shells {
    /// A shot by someone at `eye` looking along `forward` (GoldSrc coordinates).
    pub fn eject(&mut self, weapon: Id, eye: [f32; 3], forward: [f32; 3], velocity: [f32; 3], own: bool) {
        self.queue.push((weapon, eye, forward, velocity, own));
    }

    fn rand(&mut self, lo: f32, hi: f32) -> f32 {
        if self.rng == 0 {
            self.rng = 0x9E37_79B9_7F4A_7C15;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        lo + (hi - lo) * ((self.rng % 100_000) as f32 / 100_000.0)
    }
}

#[derive(Component)]
struct Shell {
    vel: Vec3,
    spin: Vec3,
    left: f32,
    shotgun: bool,
    sounds: u8,
}

/// The shell's start and velocity (GoldSrc coordinates).
fn launch(t: &Throw, eye: [f32; 3], forward: [f32; 3], velocity: [f32; 3], own: bool, r: &mut impl FnMut(f32, f32) -> f32) -> ([f32; 3], [f32; 3]) {
    let f = forward;
    let flat = (f[0] * f[0] + f[1] * f[1]).sqrt().max(1e-3);
    let right = [f[1] / flat, -f[0] / flat, 0.0];
    // up = right x forward
    let up = [right[1] * f[2] - right[2] * f[1], right[2] * f[0] - right[0] * f[2], right[0] * f[1] - right[1] * f[0]];
    let [fs, us, rs] = if own { t.own } else { OTHERS };
    let (side, lift, fwd) = (r(50.0, 90.0) * if t.left { -1.0 } else { 1.0 }, r(75.0, 175.0), r(25.0, 150.0));
    let half: [f32; 3] = velocity.map(|v| v * 0.5);
    let mut vel: [f32; 3] = std::array::from_fn(|i| half[i] + right[i] * side + up[i] * lift + f[i] * fwd);
    if own {
        vel[2] -= t.drop;
    }
    let at = std::array::from_fn(|i| eye[i] + f[i] * fs + up[i] * us + right[i] * rs + half[i] / 5.0);
    (at, vel)
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    mut shells: ResMut<Shells>,
    mut cache: ResMut<ModelCache>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StudioMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: Local<HashMap<&'static str, models::Parts>>,
) {
    for (weapon, eye, forward, velocity, own) in std::mem::take(&mut shells.queue) {
        let Some(t) = throw(weapon) else { continue };
        let Some(m) = cache.get(t.model, &mut images, &mut materials) else { continue };
        let p = parts.entry(t.model).or_insert_with(|| {
            let pose = m.model.pose(0, 0, 0.0);
            m.model.posed_body(&pose, 0).iter().filter(|p| !p.indices.is_empty()).map(|p| (meshes.add(models::part_mesh(p, map::to_y_up, false)), m.materials.get(p.texture).cloned().unwrap_or_default())).collect()
        });
        let (at, vel) = launch(&t, eye, forward, velocity, own, &mut |lo, hi| shells.rand(lo, hi));
        // `R_TempModel`'s spin, times the weapon's (degrees a second)
        let spin = Vec3::new(shells.rand(-256.0, 255.0), shells.rand(-256.0, 255.0), shells.rand(-256.0, 255.0)) * t.spin.to_radians();
        let yaw = forward[1].atan2(forward[0]);
        let parent = commands
            .spawn((Shell { vel: Vec3::from(map::to_y_up(vel)), spin, left: LIFE, shotgun: t.shotgun, sounds: 0 }, Transform::from_translation(Vec3::from(map::to_y_up(at))).with_rotation(Quat::from_rotation_y(yaw)), Visibility::default(), DespawnOnExit(AppState::InGame)))
            .id();
        models::spawn_lit(&mut commands, parent, p, &mut materials);
    }
}

/// Full gravity, bouncing off the world at half speed with a tinkle (the first few bounces).
fn fly(mut commands: Commands, time: Res<Time>, mut w: ResMut<Walker>, mut shells: Query<(Entity, &mut Shell, &mut Transform)>) {
    let dt = time.delta_secs().min(0.05);
    let mut cues = Vec::new();
    for (e, mut s, mut tf) in &mut shells {
        s.left -= dt;
        if s.left <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        s.vel.y -= GRAVITY * dt;
        let from = tf.translation;
        let to = from + s.vel * dt;
        let (a, b) = ([from.x, -from.z, from.y], [to.x, -to.z, to.y]);
        let hit = w.client.world.trace_line(a, b);
        if hit.fraction < 1.0 && !hit.start_solid {
            let n = Vec3::from(map::to_y_up(hit.normal));
            let speed = s.vel.length();
            s.vel = (s.vel - 2.0 * s.vel.dot(n) * n) * 0.5;
            s.spin *= 0.5;
            tf.translation = Vec3::from(map::to_y_up(hit.end)) + n * 0.5;
            if s.sounds < 3 && speed > 60.0 {
                s.sounds += 1;
                let file = if s.shotgun { ["weapons/sshell1.wav", "weapons/sshell2.wav", "weapons/sshell3.wav"][s.sounds as usize % 3] } else { ["player/pl_shell1.wav", "player/pl_shell2.wav", "player/pl_shell3.wav"][s.sounds as usize % 3] };
                cues.push(Cue::Quiet { file, at: hit.end, volume: 0.8 });
            }
            if n.y > 0.7 && s.vel.length() < 20.0 {
                s.vel = Vec3::ZERO;
                s.spin = Vec3::ZERO;
            }
        } else {
            tf.translation = to;
        }
        let spin = s.spin * dt;
        tf.rotate(Quat::from_euler(EulerRot::XYZ, spin.x, spin.y, spin.z));
    }
    w.cues.extend(cues);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shell_leaves_to_the_right_and_up() {
        let t = throw(Id::Ak47).unwrap();
        let mid = |lo: f32, hi: f32| (lo + hi) / 2.0;
        // looking along +x: right is -y in GoldSrc
        let (at, vel) = launch(&t, [0.0; 3], [1.0, 0.0, 0.0], [0.0; 3], false, &mut |lo, hi| mid(lo, hi));
        assert!(at[1] < 0.0 && at[2] < 0.0 && at[0] > 0.0, "{at:?}");
        assert!(vel[1] < -50.0 && vel[2] > 75.0, "{vel:?}");
        assert!(throw(Id::Knife).is_none() && throw(Id::HeGrenade).is_none() && throw(Id::Awp).is_some());
        // the P90 throws its shells out to the left
        let (_, vel) = launch(&throw(Id::P90).unwrap(), [0.0; 3], [1.0, 0.0, 0.0], [0.0; 3], false, &mut |lo, hi| mid(lo, hi));
        assert!(vel[1] > 50.0, "{vel:?}");
    }
}
