//! Playing: the connection (`Walker`), keyboard and mouse into sim inputs, and the camera.
//! Also defines the order the other in-game plugins run in each frame (`Frame`).
use crate::binds::{Action, Bind, Binds};
use crate::buy::BuyMenu;
use crate::level::Spawns;
use crate::menu::{overlay_open, AppState, GameConfig, MenuError, Overlay, PendingClient, Settings};
use crate::{game_dir, map, sound, viewmodel};
use bevy::{
    core_pipeline::tonemapping::Tonemapping,
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};

/// In-game frame order: camera first, then server events, then everything drawn from
/// them, then sounds, then the HUD.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Frame {
    Camera,
    Events,
    World,
    Sound,
    Ui,
}

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (Frame::Camera, Frame::Events, Frame::World, Frame::Sound, Frame::Ui).chain().run_if(in_state(AppState::InGame)).run_if(resource_exists::<Walker>),
        )
            .add_systems(OnEnter(AppState::InGame), enter)
            .add_systems(FixedUpdate, walk.run_if(in_state(AppState::InGame)).run_if(resource_exists::<Walker>))
            .add_systems(Update, (face_spawn, mouse_look, move_camera, grab_cursor).chain().in_set(Frame::Camera));
    }
}

/// The connection to the server; it also predicts our own player.
#[derive(Resource)]
pub struct Walker {
    pub client: net::Client,
    /// Eye position at the previous tick, for smoothing between ticks.
    prev_eye: [f32; 3],
    /// V detaches the camera for free flight (the player stays put).
    pub noclip: bool,
    /// Weapon slot key held (to send each press once).
    slot_held: u8,
    drop_held: bool,
    /// Mouse wheel movement not yet turned into a weapon switch.
    wheel: f32,
    /// Bullet tracers: from, to, seconds left.
    pub tracers: Vec<([f32; 3], [f32; 3], f32)>,
    /// Events for `effects::grenade_effects` (explosions, flashes).
    pub pending_fx: Vec<net::protocol::GameEvent>,
    /// Sounds to play this frame.
    pub cues: Vec<sound::Cue>,
    /// Camera offset still to glide away after a prediction correction (Y-up units).
    smooth: Vec3,
}

/// Prediction corrections fade with this time constant; beyond this distance they snap.
const SMOOTH_SECONDS: f32 = 0.05;
const SNAP_DISTANCE: f32 = 64.0;

/// The player's camera (other modules read where it looks).
#[derive(Component)]
pub struct FlyCam {
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
}

/// Take over the connection, swap in the real map collision, add the camera.
fn enter(mut commands: Commands, mut pending: ResMut<PendingClient>, config: Res<GameConfig>, mut next: ResMut<NextState<AppState>>) {
    let Some(mut client) = pending.0.take() else { return };
    let path = game_dir().join("cstrike/maps").join(format!("{}.bsp", config.map));
    match goldsrc::bsp::Bsp::load(&path) {
        // Same map name isn't enough: a changed file would mean different walls on our side.
        Ok(bsp) if bsp.hash != client.map_hash => {
            client.disconnect();
            commands.insert_resource(MenuError(format!("Your {} is different from the server's (different version or modified). Get the server's map file.", config.map)));
            next.set(AppState::Menu);
            return;
        }
        Ok(bsp) => client.world = sim::collision::World::from_bsp(&bsp),
        Err(e) => {
            commands.insert_resource(MenuError(format!("You don't have {} ({e})", config.map)));
            next.set(AppState::Menu);
            return;
        }
    }
    commands.insert_resource(Walker {
        client,
        prev_eye: [0.0; 3],
        noclip: false,
        slot_held: 0,
        drop_held: false,
        wheel: 0.0,
        tracers: Vec::new(),
        pending_fx: Vec::new(),
        cues: Vec::new(),
        smooth: Vec3::ZERO,
    });
    commands.insert_resource(Overlay::None);
    let cam = commands
        .spawn((
            Camera3d::default(),
            Msaa::Off, // CS 1.6 look (no anti-aliasing), and cheaper
            // No dynamic lights (all baked in lightmaps): one cluster (None breaks Bevy 0.20 GPU clustering).
            bevy::light::cluster::ClusterConfig::Single,
            Projection::Perspective(PerspectiveProjection {
                fov: 73.74_f32.to_radians(), // vertical FOV matching 90° horizontal at 4:3
                near: 4.0,
                far: 16384.0,
                ..default()
            }),
            // Exposure of 1.0, so lightmap values map straight to screen brightness.
            bevy::camera::Exposure { ev100: -(1.2_f32.log2()) },
            Tonemapping::None,
            bevy::core_pipeline::tonemapping::DebandDither::Disabled,
            Transform::default(),
            FlyCam { yaw: 0.0, pitch: 0.0 },
            SpatialListener::new(16.0),
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    viewmodel::attach(&mut commands, cam);
}

/// On (re)spawning, look the way the spawn point faces, like CS.
fn face_spawn(w: Res<Walker>, spawns: Option<Res<Spawns>>, mut cam: Single<&mut FlyCam>, mut was_alive: Local<bool>) {
    let alive = w.client.vitals.alive() && w.client.id().is_some();
    if alive && !*was_alive {
        let me = Vec3::from(map::to_y_up(w.client.me().origin));
        if let Some((_, yaw)) = spawns.iter().flat_map(|s| s.0.iter()).min_by(|a, b| me.distance(Vec3::from(a.0)).total_cmp(&me.distance(Vec3::from(b.0)))) {
            cam.yaw = (yaw - 90.0).to_radians();
            cam.pitch = 0.0;
        }
    }
    *was_alive = alive;
}

fn mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions>,
    w: Res<Walker>,
    settings: Res<Settings>,
    mut cam: Single<(&mut Transform, &mut FlyCam, &mut Projection)>,
) {
    let (ref mut tf, ref mut fc, ref mut proj) = *cam;
    // Zoomed weapons slow the mouse down in proportion, like CS.
    let fov = w.client.state.loadout.active_gun().map(|g| g.fov()).unwrap_or(90) as f32;
    // CS: sensitivity × m_yaw (0.022°) per mouse count.
    let sens = (settings.sensitivity * 0.022).to_radians() * fov / 90.0;
    // Screenshot runs can aim the camera: BOOTLEG16_LOOK=yaw,pitch (GoldSrc degrees).
    if let Some((y, p)) = std::env::var("BOOTLEG16_LOOK").ok().and_then(|v| v.split_once(',').map(|(a, b)| (a.parse::<f32>().ok(), b.parse::<f32>().ok()))) {
        if let (Some(y), Some(p)) = (y, p) {
            fc.yaw = (y - 90.0).to_radians();
            fc.pitch = -p.to_radians();
        }
    }
    if cursor.grab_mode != CursorGrabMode::None {
        fc.yaw -= motion.delta.x * sens;
        fc.pitch = (fc.pitch - motion.delta.y * sens).clamp(-1.55, 1.55);
    }
    // Recoil punch moves the view (GoldSrc pitch is positive downwards).
    let punch = w.client.state.loadout.punch;
    tf.rotation = Quat::from_euler(EulerRot::YXZ, fc.yaw + punch[1].to_radians(), fc.pitch - punch[0].to_radians(), punch[2].to_radians());
    // Horizontal FOV (90 = normal) at 4:3, as a vertical FOV.
    if let Projection::Perspective(p) = &mut **proj {
        p.fov = 2.0 * ((fov.to_radians() / 2.0).tan() * 0.75).atan();
    }
}

/// One simulation tick from the keyboard and mouse, through the player's bindings (`binds`;
/// CS's defaults: WASD, Space, Ctrl, Shift, E, mouse, R, G, Q, 1-5).
#[allow(clippy::too_many_arguments)]
fn walk(
    mut vm: ResMut<viewmodel::ViewModel>,
    binds: Res<Binds>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    wheel: Res<AccumulatedMouseScroll>,
    cursor: Single<&CursorOptions>,
    cam: Single<&FlyCam>,
    buy: Res<BuyMenu>,
    overlay: Option<Res<Overlay>>,
    mut w: ResMut<Walker>,
) {
    // Mouse buttons only count while the mouse is captured (the first click just captures it).
    let captured = cursor.grab_mode != CursorGrabMode::None;
    // Automated screenshot runs ignore the keyboard too (the window may pop up while you type);
    // so do menus over the game.
    let auto = std::env::var_os("BOOTLEG16_SCREENSHOT").is_some();
    let keys = if auto || overlay_open(overlay) { &ButtonInput::<KeyCode>::default() } else { &*keys };
    let slot_keys = [Action::Primary, Action::Secondary, Action::Knife, Action::Grenade, Action::Bomb];
    let on = |a: Action| binds.held(a, keys, &mouse);
    // While the buy menu is open, number keys belong to the menu.
    let mut slot = if buy.is_open() { 0 } else { slot_keys.iter().position(|a| on(*a)).map(|i| i as u8 + 1).unwrap_or(0) };
    if on(Action::LastWeapon) {
        slot = sim::pmove::SLOT_LAST;
    }
    // Mouse wheel: up = previous weapon, down = next (CS invprev / invnext).
    let scroll = if captured && !auto { wheel.delta.y } else { 0.0 };
    w.wheel += scroll;
    // Several ticks can run per frame: send a slot press once.
    let held = slot;
    let mut slot = if slot == w.slot_held { 0 } else { slot };
    w.slot_held = held;
    if slot == 0 && w.wheel.abs() >= 1.0 {
        slot = if w.wheel > 0.0 { sim::pmove::SLOT_PREV } else { sim::pmove::SLOT_NEXT };
        w.wheel = 0.0;
    }
    let drop = on(Action::Drop) && !w.drop_held;
    w.drop_held = on(Action::Drop);
    let axis = |pos: Action, neg: Action| 400.0 * (on(pos) as i32 - on(neg) as i32) as f32;
    let input = sim::pmove::Input {
        forward: axis(Action::Forward, Action::Back),
        side: axis(Action::Right, Action::Left),
        // Bevy yaw 0 looks down -Z, which is GoldSrc yaw 90.
        yaw: cam.yaw.to_degrees() + 90.0,
        // Bevy pitch is positive looking up; GoldSrc is positive looking down.
        pitch: -cam.pitch.to_degrees(),
        jump: on(Action::Jump),
        crouch: on(Action::Crouch),
        walk: on(Action::Walk),
        use_key: on(Action::Use),
        // mouse buttons only while the mouse is captured; keys count either way
        attack: on(Action::Fire) && (captured || matches!(binds.get(Action::Fire), Bind::Key(_))),
        attack2: on(Action::AltFire) && (captured || matches!(binds.get(Action::AltFire), Bind::Key(_))),
        reload: on(Action::Reload),
        slot,
        drop,
    };
    // Free flight: keep the connection alive but stand still.
    let input = if w.noclip { sim::pmove::Input { yaw: input.yaw, pitch: input.pitch, ..default() } } else { input };
    w.prev_eye = w.client.me().eye();
    let (events, fired) = w.client.tick(input);
    vm.cues.add(&fired);
    // Our own sounds, predicted: no waiting for the server.
    let pistol = w.client.state.loadout.active == sim::weapons::Slot::Secondary;
    for shot in &fired.shots {
        w.cues.push(sound::Cue::Shot { weapon: shot.weapon, silenced: shot.silenced, at: None });
    }
    if fired.dry_fire {
        w.cues.push(sound::Cue::DryFire { pistol });
    }
    if fired.swing.is_some() {
        w.cues.push(sound::Cue::Swing);
    }
    if let Some(step) = events.footstep {
        let me = *w.client.me();
        let feet = [me.origin[0], me.origin[1], me.origin[2] + me.hull().mins()[2]];
        w.cues.push(sound::Cue::Footstep { feet, who: None, step });
    }
    // Our own shots show at once (predicted); other players' come from server events.
    for shot in fired.shots {
        let eye = w.client.me().eye();
        let (f, _, _) = sim::weapons::bullets::view_vectors(shot.angles[0], shot.angles[1]);
        let end = w.client.world.trace_line(eye, std::array::from_fn(|i| eye[i] + f[i] * 8192.0)).end;
        w.tracers.push((eye, end, 0.08));
    }
}

fn move_camera(
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut w: ResMut<Walker>,
    watch: Res<crate::spectate::Watch>,
    mut tf: Single<&mut Transform, With<FlyCam>>,
) {
    // free flight, for looking around maps in practice (V is team voice)
    if keys.just_pressed(KeyCode::KeyN) && w.client.practice {
        w.noclip = !w.noclip;
    }
    // Dead, spectating (or forced with BOOTLEG16_SPECTATE): watch a living teammate only. With
    // none left the camera stays put behind the black "team is out" screen (spectate.rs).
    if crate::spectate::spectating(&w) && !w.noclip {
        if watch.out {
            return;
        }
        if let Some(o) = crate::spectate::target(&w, &watch) {
            let eye = [o.origin[0], o.origin[1], o.origin[2] + 20.0];
            let (s, c) = o.yaw.to_radians().sin_cos();
            // Chase view from behind (like CS); in front if there's no room (or
            // BOOTLEG16_SPECTATE=front, for checking models). Never in a wall.
            let front_first = std::env::var("BOOTLEG16_SPECTATE").is_ok_and(|v| v == "front");
            let spot = if front_first { [1.0f32, -1.0] } else { [-1.0, 1.0] }
                .iter()
                .map(|side| {
                    let want = [eye[0] + c * 100.0 * side, eye[1] + s * 100.0 * side, eye[2] + 10.0];
                    let t = w.client.world.trace_line(eye, want);
                    (t.fraction, [eye[0] + (want[0] - eye[0]) * t.fraction * 0.9, eye[1] + (want[1] - eye[1]) * t.fraction * 0.9, eye[2] + (want[2] - eye[2]) * t.fraction * 0.9])
                })
                .find(|(f, _)| *f > 0.6)
                .map(|(_, p)| p)
                .unwrap_or(eye);
            tf.translation = Vec3::from(map::to_y_up(spot));
            tf.look_at(Vec3::from(map::to_y_up(eye)), Vec3::Y);
            return;
        }
    }
    if !w.noclip {
        if w.client.id().is_none() {
            return; // still connecting
        }
        let a = fixed.overstep_fraction();
        let (p, c) = (Vec3::from(map::to_y_up(w.prev_eye)), Vec3::from(map::to_y_up(w.client.me().eye())));
        // Server corrections glide out over ~0.1 s instead of snapping; big jumps (respawn, a
        // teleport) snap.
        let d = Vec3::from(map::to_y_up(w.client.take_correction()));
        w.smooth = if (w.smooth - d).length() > SNAP_DISTANCE { Vec3::ZERO } else { w.smooth - d };
        w.smooth *= (-time.delta_secs() / SMOOTH_SECONDS).exp();
        tf.translation = p.lerp(c, a) + w.smooth;
        return;
    }
    let mut dir = Vec3::ZERO;
    for (k, d) in [
        (KeyCode::KeyW, tf.forward().as_vec3()),
        (KeyCode::KeyS, tf.back().as_vec3()),
        (KeyCode::KeyA, tf.left().as_vec3()),
        (KeyCode::KeyD, tf.right().as_vec3()),
        (KeyCode::Space, Vec3::Y),
        (KeyCode::ControlLeft, Vec3::NEG_Y),
    ] {
        if keys.pressed(k) {
            dir += d;
        }
    }
    let speed = if keys.pressed(KeyCode::ShiftLeft) { 1200.0 } else { 400.0 };
    tf.translation += dir.normalize_or_zero() * speed * time.delta_secs();
}

/// Click to capture the mouse (not while a menu or the buy menu is open). Esc is the pause menu.
fn grab_cursor(mut cursor: Single<&mut CursorOptions>, mouse: Res<ButtonInput<MouseButton>>, overlay: Option<Res<Overlay>>, buy: Res<BuyMenu>, scores: Res<crate::scoreboard::ScoreMouse>) {
    // Automated screenshot runs never take the user's mouse.
    if mouse.just_pressed(MouseButton::Left) && std::env::var_os("BOOTLEG16_SCREENSHOT").is_none() && !overlay_open(overlay) && !buy.is_open() && !scores.on {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}
