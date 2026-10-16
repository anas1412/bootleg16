//! Playing: the connection (`Walker`), keyboard and mouse into sim inputs, and the camera.
//! Also defines the order the other in-game plugins run in each frame (`Frame`).
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
    /// Kill feed lines and seconds left.
    pub feed: Vec<(String, f32)>,
    /// Events for `effects::grenade_effects` (explosions, flashes).
    pub pending_fx: Vec<net::protocol::GameEvent>,
    /// Sounds to play this frame.
    pub cues: Vec<sound::Cue>,
}

#[derive(Component)]
pub struct FlyCam {
    yaw: f32,
    pitch: f32,
}

/// Take over the connection, swap in the real map collision, add the camera.
fn enter(mut commands: Commands, mut pending: ResMut<PendingClient>, config: Res<GameConfig>, mut next: ResMut<NextState<AppState>>) {
    let Some(mut client) = pending.0.take() else { return };
    let path = game_dir().join("cstrike/maps").join(format!("{}.bsp", config.map));
    match goldsrc::bsp::Bsp::load(&path) {
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
        feed: Vec::new(),
        pending_fx: Vec::new(),
        cues: Vec::new(),
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

/// One simulation tick from the keyboard and mouse. WASD move, Space jump, Ctrl crouch,
/// Shift walk, E use, mouse fire, R reload, 1-5 weapon slots.
#[allow(clippy::too_many_arguments)]
fn walk(
    mut vm: ResMut<viewmodel::ViewModel>,
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
    let slot_keys = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5];
    // While the buy menu is open, number keys belong to the menu.
    let mut slot = if buy.is_open() { 0 } else { slot_keys.iter().position(|k| keys.pressed(*k)).map(|i| i as u8 + 1).unwrap_or(0) };
    if keys.pressed(KeyCode::KeyQ) {
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
    let drop = keys.pressed(KeyCode::KeyG) && !w.drop_held;
    w.drop_held = keys.pressed(KeyCode::KeyG);
    let axis = |pos: KeyCode, neg: KeyCode| 400.0 * (keys.pressed(pos) as i32 - keys.pressed(neg) as i32) as f32;
    let input = sim::pmove::Input {
        forward: axis(KeyCode::KeyW, KeyCode::KeyS),
        side: axis(KeyCode::KeyD, KeyCode::KeyA),
        // Bevy yaw 0 looks down -Z, which is GoldSrc yaw 90.
        yaw: cam.yaw.to_degrees() + 90.0,
        // Bevy pitch is positive looking up; GoldSrc is positive looking down.
        pitch: -cam.pitch.to_degrees(),
        jump: keys.pressed(KeyCode::Space),
        crouch: keys.pressed(KeyCode::ControlLeft),
        walk: keys.pressed(KeyCode::ShiftLeft),
        use_key: keys.pressed(KeyCode::KeyE),
        attack: captured && mouse.pressed(MouseButton::Left),
        attack2: captured && mouse.pressed(MouseButton::Right),
        reload: keys.pressed(KeyCode::KeyR),
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
    if events.footstep || events.landed {
        let me = *w.client.me();
        let feet = [me.origin[0], me.origin[1], me.origin[2] + me.hull().mins()[2]];
        w.cues.push(sound::Cue::Footstep { feet, own: true, ladder: me.on_ladder, water: me.water_level > 0 });
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
    mut tf: Single<&mut Transform, With<FlyCam>>,
) {
    if keys.just_pressed(KeyCode::KeyV) {
        w.noclip = !w.noclip;
    }
    // Dead, spectating (or forced with BOOTLEG16_SPECTATE): watch another player.
    let spectate = !w.client.vitals.alive() || std::env::var_os("BOOTLEG16_SPECTATE").is_some();
    if spectate && !w.noclip {
        // Watch a teammate if one is alive (as in CS), else anyone.
        let others = w.client.others();
        let mine = w.client.team();
        if let Some(o) = others.iter().find(|o| o.alive && o.team == mine).or_else(|| others.iter().find(|o| o.alive)).cloned() {
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
        tf.translation = p.lerp(c, a);
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
fn grab_cursor(mut cursor: Single<&mut CursorOptions>, mouse: Res<ButtonInput<MouseButton>>, overlay: Option<Res<Overlay>>, buy: Res<BuyMenu>) {
    // Automated screenshot runs never take the user's mouse.
    if mouse.just_pressed(MouseButton::Left) && std::env::var_os("BOOTLEG16_SCREENSHOT").is_none() && !overlay_open(overlay) && !buy.is_open() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}
