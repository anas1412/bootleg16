//! The weapon in your hands (CS `v_` models): drawn by its own camera so it never
//! sinks into walls, animated from what you do (draw, shoot, reload, ...).
use crate::models::{set_pose, weapon_model, Loaded, ModelCache, SpawnedSkin};
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use bevy::camera::visibility::RenderLayers;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;
use sim::weapons::{Fired, Id};
use std::sync::Arc;

/// Render layer for the view model and its camera.
const LAYER: usize = 1;

/// Things that happened to our weapon this frame (from our own predicted inputs).
#[derive(Default)]
pub struct Cues {
    pub shots: Vec<f32>, // side offset per shot (Elite left/right)
    /// The last shot was silenced (no muzzle flash).
    pub silenced: bool,
    pub swing: Option<bool>, // stab?
    pub throw: bool,
}

impl Cues {
    pub fn add(&mut self, f: &Fired) {
        self.shots.extend(f.shots.iter().map(|s| s.side_offset));
        if let Some(s) = f.shots.last() {
            self.silenced = s.silenced;
        }
        if let Some(s) = f.swing {
            self.swing = Some(s.stab);
        }
        self.throw |= f.throw.is_some();
    }
}

#[derive(Resource, Default)]
pub struct ViewModel {
    /// Which model is shown, and its silencer/burst mode.
    shown: Option<(Id, u8)>,
    model: Option<Arc<Loaded>>,
    /// The model's bones and parts (GPU-skinned; only the bones move).
    skin: Option<SpawnedSkin>,
    seq: usize,
    started: f32,
    looping: bool,
    was_reloading: bool,
    was_pinned: bool,
    was_planting: bool,
    slash: u8,
    pub cues: Cues,
    /// Muzzle flash quad (child of the view model camera), until when it shows, and which muzzle.
    flash: Option<Entity>,
    flash_until: f32,
    flash_muzzle: usize,
    /// Animation frame reached last update (for firing animation sound events once).
    last_frame: f32,
    last_fov: u8,
}

#[derive(Component)]
pub struct ViewModelCamera;

/// Second camera, child of the main one, that only sees the view model and draws it
/// on top of the world. Fixed 90 degree field of view, like CS.
pub fn attach(commands: &mut Commands, cam: Entity) {
    let vm = commands
        .spawn((
            ViewModelCamera,
            Camera3d::default(),
            Msaa::Off,
            bevy::light::cluster::ClusterConfig::Single,
            Camera { order: 1, clear_color: ClearColorConfig::None, ..default() },
            Projection::Perspective(PerspectiveProjection { fov: 2.0 * (45f32.to_radians().tan() * 0.75).atan(), near: 1.0, far: 200.0, ..default() }),
            bevy::core_pipeline::tonemapping::Tonemapping::None,
            bevy::core_pipeline::tonemapping::DebandDither::Disabled,
            RenderLayers::layer(LAYER),
            Transform::default(),
            // The weapon in your hands is lit by where you stand.
            crate::studio::LitByMe,
        ))
        .id();
    commands.entity(cam).add_child(vm);
}

/// From the model's Y-up space (`map::to_y_up`) to camera space, mirrored left-right:
/// GoldSrc (x forward, y left, z up) ends up as (y, z, -x). CS's v_ models are stored
/// left-handed and CS 1.6 shows them right-handed by default (`cl_righthand 1`).
fn to_camera() -> Transform {
    // (a, b, c) -> (-c, b, -a): a reflection, written as a mirror in x then a rotation.
    let rotation = Quat::from_mat3(&Mat3::from_cols(Vec3::new(0.0, 0.0, 1.0), Vec3::Y, Vec3::new(-1.0, 0.0, 0.0)));
    Transform { rotation, scale: Vec3::new(-1.0, 1.0, 1.0), ..default() }
}

fn find(model: &goldsrc::mdl::Model, names: &[&str]) -> Option<usize> {
    names.iter().find_map(|n| model.sequence(n))
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut commands: Commands,
    time: Res<Time>,
    mut w: ResMut<Walker>,
    mut vm: ResMut<ViewModel>,
    mut cache: ResMut<ModelCache>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<crate::studio::StudioMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut vis: Query<&mut Visibility>,
    mut bones: Query<&mut Transform, With<ChildOf>>,
    camera: Single<Entity, With<ViewModelCamera>>,
    flash_look: Res<crate::muzzle::FlashLook>,
    watch: Res<crate::spectate::Watch>,
) {
    let now = time.elapsed_secs();
    let l = &w.client.state.loadout;
    // Watching a teammate through their eyes: their gun (idle) in their hands.
    let watched = crate::spectate::in_eye(&w, &watch);
    let id = watched.map(|o| o.weapon).unwrap_or(l.active_id());
    let gun = l.active_gun().filter(|_| watched.is_none());
    let mode = gun.map(|g| g.mode).unwrap_or(0);
    let alive = (w.client.vitals.alive() && w.client.id().is_some() && !crate::spectate::spectating(&w)) || watched.is_some();
    // Silenced-capable guns have *_unsil animations for the plain mode.
    let unsil = matches!(id, Id::Usp | Id::M4a1) && mode == 0;
    let names = |base: &str| -> Vec<String> { if unsil { vec![format!("{base}_unsil"), base.to_string()] } else { vec![base.to_string()] } };

    // A different weapon (or silencer switch): load it and play draw / attach.
    if vm.shown.map(|s| s.0) != Some(id) {
        if let Some(old) = vm.skin.take() {
            old.despawn(&mut commands);
        }
        vm.model = cache.get(&weapon_model("v", id), &mut images, &mut materials);
        if let Some(m) = vm.model.clone() {
            // The parts hang off the view model camera, which only sees their layer.
            let skin = m.spawn_skinned(&mut commands, *camera, Some(to_camera()), &mut meshes, &mut bindposes, &mut materials);
            for e in &skin.parts {
                commands.entity(*e).insert(RenderLayers::layer(LAYER));
            }
            vm.skin = Some(skin);
            // The Glock picks one of two draws at random (cs.so CGLOCK18::Deploy).
            let draw_name = if id == Id::Glock && (now * 1000.0) as u32 % 2 == 1 { "draw2" } else { "draw" };
            let draw: Vec<String> = names(draw_name).into_iter().chain(["deploy".to_string(), "draw".to_string()]).collect();
            let draw: Vec<&str> = draw.iter().map(String::as_str).collect();
            play(&mut vm, find(&m.model, &draw), now, false);
        }
        vm.shown = Some((id, mode));
        vm.cues = Cues::default();
    } else if vm.shown.map(|s| s.1) != Some(mode) {
        if let Some(m) = vm.model.clone() {
            let name = if matches!(id, Id::Usp | Id::M4a1) { if mode == 1 { "add_silencer" } else { "detach_silencer" } } else { "" };
            play(&mut vm, m.model.sequence(name), now, false);
        }
        vm.shown = Some((id, mode));
    }
    let Some(m) = vm.model.clone() else { return };

    // What happened this frame decides the animation.
    let cues = std::mem::take(&mut vm.cues);
    let reloading = gun.is_some_and(|g| g.reloading || g.loading_shells());
    if let Some(side) = cues.shots.last() {
        if crate::muzzle::flashes(id, cues.silenced) {
            vm.flash_until = now + crate::muzzle::FLASH_SECONDS;
            // Dual Elites: the left or right gun's muzzle.
            vm.flash_muzzle = if id == Id::Elite && *side > 0.0 { 1 } else { 0 };
        }
        let empty = gun.is_some_and(|g| g.clip == 0);
        let seq = if id == Id::Elite {
            find(&m.model, if *side < 0.0 { &["shoot_left1"] } else { &["shoot_right1"] })
        } else if id == Id::Glock {
            // CS 1.6 (client.so EV_FireGlock18): every bullet, burst or not, plays `shoot3`;
            // the last round `shoot_empty`. (`shoot`/`shoot2` are the shield versions.)
            find(&m.model, if empty { &["shoot_empty"] } else { &["shoot3"] })
        } else {
            let k = (now * 1000.0) as usize % 3;
            let shoot = names(["shoot1", "shoot2", "shoot3"][k]);
            let last = names("shootlast");
            let mut want: Vec<&str> = Vec::new();
            if empty {
                want.extend(last.iter().map(String::as_str));
                want.push("shoot_empty");
            }
            want.extend(shoot.iter().map(String::as_str));
            want.extend(["shoot", "shoot1"]);
            find(&m.model, &want)
        };
        play(&mut vm, seq, now, false);
    } else if let Some(stab) = cues.swing {
        vm.slash ^= 1;
        let seq = if stab { find(&m.model, &["stab"]) } else { find(&m.model, if vm.slash == 0 { &["midslash1", "slash1"] } else { &["midslash2", "slash2"] }) };
        play(&mut vm, seq, now, false);
    } else if cues.throw {
        play(&mut vm, m.model.sequence("throw"), now, false);
    } else if l.planting.is_some() && !vm.was_planting {
        play(&mut vm, m.model.sequence("pressbutton"), now, false);
    } else if l.planting.is_none() && vm.was_planting && l.c4 {
        // Let go before it was planted: back to holding it.
        play(&mut vm, m.model.sequence("idle1"), now, true);
    } else if l.pin_pulled() && !vm.was_pinned {
        play(&mut vm, m.model.sequence("pullpin"), now, false);
    } else if reloading && !vm.was_reloading {
        // The Glock picks one of two reloads at random (cs.so CGLOCK18::Reload).
        let r = names(if id == Id::Glock && (now * 1000.0) as u32 % 2 == 1 { "reload2" } else { "reload" });
        let mut want: Vec<&str> = r.iter().map(String::as_str).collect();
        want.insert(0, "start_reload");
        play(&mut vm, find(&m.model, &want), now, false);
    } else if gun.is_some_and(|g| g.loading_shells()) && vm.looping {
        play(&mut vm, m.model.sequence("insert"), now, false);
    }
    vm.was_reloading = reloading;
    vm.was_pinned = l.pin_pulled();
    vm.was_planting = l.planting.is_some();

    // A one-shot animation that has finished goes back to idle (looping).
    let s = &m.model.sequences[vm.seq.min(m.model.sequences.len() - 1)];
    let mut frame = (now - vm.started) * s.fps;
    // Holding a pulled grenade (or a plant in progress): stay on the last frame, cocked,
    // instead of dropping back to idle before the throw.
    let holding = l.pin_pulled() || l.planting.is_some();
    if !vm.looping && holding && frame >= (s.frames - 1) as f32 {
        frame = (s.frames - 1) as f32;
    } else if !vm.looping && frame >= (s.frames - 1) as f32 {
        let idle = names("idle");
        let mut want: Vec<&str> = idle.iter().map(String::as_str).collect();
        want.extend(["idle1", "idle"]);
        if gun.is_some_and(|g| g.loading_shells()) {
            want.insert(0, "insert");
        }
        let seq = find(&m.model, &want).unwrap_or(0);
        play(&mut vm, Some(seq), now, true);
        frame = 0.0;
    }
    let s = &m.model.sequences[vm.seq.min(m.model.sequences.len() - 1)];
    if vm.looping {
        frame %= (s.frames.max(2) - 1) as f32;
    }

    // Sounds the model's animation asks for (reload clicks, bolt pulls, pin pulls).
    let mut sounds = Vec::new();
    let s = &m.model.sequences[vm.seq.min(m.model.sequences.len() - 1)];
    let wrapped = frame < vm.last_frame;
    for (f, event, file) in &s.events {
        let f = *f as f32;
        let crossed = if wrapped { f > vm.last_frame || f <= frame } else { f > vm.last_frame && f <= frame };
        if *event == 5004 && crossed && alive {
            sounds.push(crate::sound::Cue::File(file.clone()));
        }
    }
    vm.last_frame = frame;
    // Scoping in and out clicks.
    let fov = gun.map(|g| g.fov()).unwrap_or(90);
    if fov != vm.last_fov && vm.shown.is_some_and(|s| s.0 == id) && vm.last_fov != 0 {
        sounds.push(crate::sound::Cue::Zoom);
    }
    vm.last_fov = fov;

    // Pose the bones (the GPU moves the vertices).
    let pose = m.model.pose_at(vm.seq, frame, 0.0);
    if let Some(skin) = &vm.skin {
        set_pose(&skin.joints, &pose, &mut bones);
    }
    // Muzzle flash at the muzzle, facing us, a random turn each shot.
    let flash = *vm.flash.get_or_insert_with(|| {
        let e = crate::muzzle::spawn(&mut commands, &flash_look);
        commands.entity(e).insert(RenderLayers::layer(LAYER));
        commands.entity(*camera).add_child(e);
        e
    });
    let lit = now < vm.flash_until && alive;
    if let (true, Some(at)) = (lit, m.model.attachment(vm.flash_muzzle, &pose)) {
        if let Ok(mut t) = bones.get_mut(flash) {
            t.translation = to_camera().transform_point(Vec3::from(crate::map::to_y_up(at)));
            t.rotation = Quat::from_rotation_z((now * 9973.0).fract() * std::f32::consts::TAU);
            t.scale = Vec3::splat(9.0);
        }
    }
    if let Ok(mut v) = vis.get_mut(flash) {
        v.set_if_neq(if lit { Visibility::Inherited } else { Visibility::Hidden });
    }
    // Hidden while dead (unless watching a teammate through their eyes) or through a sniper scope.
    let scoped = gun.is_some_and(|g| g.fov() < 90) && matches!(id, Id::Awp | Id::Scout | Id::G3sg1 | Id::Sg550);
    let show = alive && !scoped && !w.noclip;
    for e in vm.skin.iter().flat_map(|s| &s.parts) {
        if let Ok(mut v) = vis.get_mut(*e) {
            v.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
        }
    }
    w.cues.extend(sounds);
}

fn play(vm: &mut ViewModel, seq: Option<usize>, now: f32, looping: bool) {
    if let Some(seq) = seq {
        vm.seq = seq;
        vm.started = now;
        vm.looping = looping;
        vm.last_frame = -1.0;
    }
}

pub struct ViewModelPlugin;

impl Plugin for ViewModelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ViewModel>()
            // A fresh match starts with no weapon shown (the old entities went with the camera).
            .add_systems(OnEnter(AppState::InGame), |mut commands: Commands| commands.insert_resource(ViewModel::default()))
            .add_systems(Update, update.in_set(Frame::World));
    }
}
