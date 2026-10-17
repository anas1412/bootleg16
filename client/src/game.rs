//! Playing: the connection (`Walker`), keyboard and mouse into sim inputs, and the camera.
//! Also defines the order the other in-game plugins run in each frame (`Frame`).
use crate::binds::{Action, Bind, Binds};
use crate::buy::BuyMenu;
use crate::level::Spawns;
use crate::menu::{overlay_open, AppState, Overlay, PendingClient, Settings};
use crate::notice::Notice;
use crate::{map, sound, viewmodel};
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

/// Hitboxes and surface materials, to trace our own shots as the server will (holes at once).
#[derive(Resource)]
pub(crate) struct ShotAssets(net::server::Assets);

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            Update,
            (Frame::Camera, Frame::Events, Frame::World, Frame::Sound, Frame::Ui).chain().run_if(in_state(AppState::InGame)).run_if(resource_exists::<Walker>),
        )
            .add_systems(Startup, |mut c: Commands| match net::server::Assets::load(&crate::game_dir()) {
                Ok(a) => c.insert_resource(ShotAssets(a)),
                Err(e) => warn!("no hitboxes, own bullet holes come from the server: {e}"),
            })
            .add_systems(OnEnter(AppState::InGame), enter)
            .add_systems(Update, connection.run_if(in_state(AppState::InGame)).run_if(resource_exists::<Walker>))
            .add_systems(OnExit(AppState::InGame), |mut c: Commands| c.remove_resource::<Reconnecting>())
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
/// (The loading screen already loaded the map and put its collision in the client.)
fn enter(mut commands: Commands, mut pending: ResMut<PendingClient>, settings: Res<crate::menu::Settings>) {
    let Some(mut client) = pending.0.take() else { return };
    // Your characters (Characters page), sent until the server has them.
    if let Some([t, ct]) = settings.looks {
        client.set_looks(t, ct);
    }
    commands.insert_resource(Walker {
        client,
        prev_eye: [0.0; 3],
        noclip: false,
        slot_held: 0,
        drop_held: false,
        wheel: 0.0,
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

/// The server has gone quiet: shown after `QUIET` seconds while we try to rejoin (same seat);
/// after `give_up` we leave with a notice. Esc leaves at once.
#[derive(Resource)]
pub struct Reconnecting {
    /// Seconds since the server was last heard from.
    pub since: f32,
    pub give_up: f32,
}

const QUIET: f32 = 3.0;
const GIVE_UP: f32 = 30.0;

fn connection(mut commands: Commands, mut w: ResMut<Walker>, keys: Res<ButtonInput<KeyCode>>, reconnecting: Option<ResMut<Reconnecting>>, mut next: ResMut<NextState<AppState>>) {
    if w.client.status == net::client::Status::Rejected {
        // e.g. voted out, or the match is gone by the time we're back
        let why = w.client.reject_reason.clone().unwrap_or_default();
        commands.insert_resource(Notice::error("Disconnected", why));
        next.set(AppState::Menu);
        return;
    }
    let since = w.client.silence();
    match reconnecting {
        None if since > QUIET => {
            w.client.rejoin();
            commands.insert_resource(Reconnecting { since, give_up: GIVE_UP });
        }
        None => {}
        Some(_) if since <= QUIET => commands.remove_resource::<Reconnecting>(),
        Some(_) if since > GIVE_UP => {
            commands.insert_resource(Notice::error("Connection lost", "The server stopped answering.").with_retry());
            next.set(AppState::Menu);
        }
        Some(_) if keys.just_pressed(KeyCode::Escape) => next.set(AppState::Menu),
        Some(mut r) => r.since = since,
    }
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
    shot_assets: Option<Res<ShotAssets>>,
    mut hits: MessageWriter<crate::impacts::Hit>,
    mut tracers: ResMut<crate::tracers::Tracers>,
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
    // Screenshot runs: `BOOTLEG16_SLOT=3` picks that weapon slot (3 = knife).
    if auto {
        if let Some(n) = std::env::var("BOOTLEG16_SLOT").ok().and_then(|v| v.parse().ok()) {
            slot = n;
        }
    }
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
        attack: (on(Action::Fire) && (captured || matches!(binds.get(Action::Fire), Bind::Key(_)))) || (auto && std::env::var_os("BOOTLEG16_FIRE").is_some()),
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
    // Our own shots show at once (predicted, traced like the server does: same spread seed,
    // penetration and players in the way); other players' come from server events. Tracers
    // (`tracers`) start at the barrel.
    use sim::weapons::bullets::{fire_bullet, fire_pellets, view_vectors, Target};
    let me_id = w.client.id().unwrap_or(u8::MAX);
    let targets: Vec<Target> = w.client.others().iter().filter(|o| o.alive).map(|o| Target { id: o.id, origin: o.origin, yaw: o.yaw, pitch: o.pitch, crouched: o.crouched, weapon: o.weapon, anim: None }).collect();
    for shot in fired.shots {
        let eye = w.client.me().eye();
        let (f, right, up) = view_vectors(shot.angles[0], shot.angles[1]);
        let mut end = w.client.world.trace_line(eye, std::array::from_fn(|i| eye[i] + f[i] * 8192.0)).end;
        // Where the crosshair points (the view includes the recoil kick): the tracer goes there,
        // not along the bullet's random spread, so a spray never throws streaks off to the side.
        let crosshair = end;
        if let Some(a) = &shot_assets {
            let r = if shot.pellets.is_some() { fire_pellets(&w.client.world, &a.0.hitboxes, eye, &shot, me_id, &targets) } else { fire_bullet(&w.client.world, &a.0.materials, &a.0.hitboxes, eye, &shot, me_id, &targets) };
            for i in r.impacts.iter().filter(|i| !i.sky) {
                hits.write(crate::impacts::Hit::Wall { at: i.point, normal: i.normal });
                w.cues.push(sound::Cue::Impact { at: i.point, normal: i.normal });
            }
            if let Some(i) = r.impacts.first() {
                end = i.point;
            }
        }
        // The tracer: from the barrel to where the shot ended. The view model camera has the
        // world camera's field of view, so the model's muzzle (camera space: x right, y up,
        // -z forward) maps straight onto the world.
        if let Some(m) = vm.muzzle {
            let muzzle = std::array::from_fn(|i| eye[i] + right[i] * m.x + up[i] * m.y - f[i] * m.z);
            tracers.shot(muzzle, crosshair);
        }
    }
}

/// A chase camera on someone at `origin` facing `yaw`: behind them (in front if there's no
/// room there, or first when `front_first`), never in a wall. Returns (camera, what it looks at).
fn chase(world: &sim::collision::World, origin: [f32; 3], yaw: f32, front_first: bool) -> ([f32; 3], [f32; 3]) {
    let eye = [origin[0], origin[1], origin[2] + 20.0];
    let (s, c) = yaw.to_radians().sin_cos();
    let spot = if front_first { [1.0f32, -1.0] } else { [-1.0, 1.0] }
        .iter()
        .map(|side| {
            let want = [eye[0] + c * 100.0 * side, eye[1] + s * 100.0 * side, eye[2] + 10.0];
            let t = world.trace_line(eye, want);
            (t.fraction, std::array::from_fn(|i| eye[i] + (want[i] - eye[i]) * t.fraction * 0.9))
        })
        .find(|(f, _)| *f > 0.6)
        .map(|(_, p)| p)
        .unwrap_or(eye);
    (spot, eye)
}

fn move_camera(
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut w: ResMut<Walker>,
    watch: Res<crate::spectate::Watch>,
    mut tf: Single<&mut Transform, With<FlyCam>>,
) {
    // Screenshot runs can park the camera: BOOTLEG16_CAM=x,y,z (GoldSrc units).
    if let Some(p) = std::env::var("BOOTLEG16_CAM").ok().map(|v| v.split(',').filter_map(|x| x.trim().parse::<f32>().ok()).collect::<Vec<_>>()).filter(|p| p.len() == 3) {
        tf.translation = Vec3::from(map::to_y_up([p[0], p[1], p[2]]));
        return;
    }
    // free flight, for looking around maps in practice (V is team voice)
    if keys.just_pressed(KeyCode::KeyN) && w.client.practice {
        w.noclip = !w.noclip;
    }
    // Dead, spectating (or forced with BOOTLEG16_SPECTATE): through a living teammate's eyes;
    // with the whole team out, from behind where the last of you went down (spectate.rs).
    if crate::spectate::spectating(&w) && !w.noclip {
        if let Some(o) = crate::spectate::in_eye(&w, &watch) {
            let h = if o.crouched { sim::pmove::VIEW_HEIGHT_CROUCHED } else { sim::pmove::VIEW_HEIGHT_STANDING };
            tf.translation = Vec3::from(map::to_y_up([o.origin[0], o.origin[1], o.origin[2] + h]));
            tf.rotation = Quat::from_euler(EulerRot::YXZ, (o.yaw - 90.0).to_radians(), -o.pitch.to_radians(), 0.0);
            return;
        }
        let watched = crate::spectate::target(&w, &watch).map(|o| (o.origin, o.yaw)).or(if watch.out { watch.last_down } else { None });
        if let Some((origin, yaw)) = watched {
            let front_first = std::env::var("BOOTLEG16_SPECTATE").is_ok_and(|v| v == "front");
            let (spot, eye) = chase(&w.client.world, origin, yaw, front_first);
            tf.translation = Vec3::from(map::to_y_up(spot));
            tf.look_at(Vec3::from(map::to_y_up(eye)), Vec3::Y);
        }
        return;
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
