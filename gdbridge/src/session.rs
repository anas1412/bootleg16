//! A practice session: our own game server on this machine (as the Bevy client's Solo
//! practice) on a box-built map, and the client playing on it with prediction. Godot calls
//! `tick` at the server's rate from `_physics_process` and reads where to put the camera.
use crate::units::{to_godot, view_to_sim, yaw_to_godot};
use client_core::view::{punch_at, vertical_fov, View};
use godot::prelude::*;
use net::protocol::Team;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Input buttons, as `tick` takes them (a bit each).
const JUMP: i64 = 1;
const CROUCH: i64 = 2;
const WALK: i64 = 4;
const USE: i64 = 8;
const ATTACK: i64 = 16;
const ATTACK2: i64 = 32;
const RELOAD: i64 = 64;
const DROP: i64 = 128;

#[derive(GodotClass)]
#[class(base = RefCounted)]
pub struct Session {
    client: Option<net::Client>,
    /// Stops our server thread (also when the session goes away).
    stop: Option<Arc<AtomicBool>>,
    joined: bool,
    view: View,
    /// The eye and recoil punch before the last tick (frames are drawn between ticks).
    prev_eye: [f32; 3],
    prev_punch: [f32; 3],
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for Session {
    fn init(base: Base<RefCounted>) -> Self {
        Session { client: None, stop: None, joined: false, view: View::default(), prev_eye: [0.0; 3], prev_punch: [0.0; 3], base }
    }
}

#[godot_api]
impl Session {
    /// Start a practice server on this machine on a map (the text of its `.bmap`, written by
    /// `tools/export_map.gd`) and join it as a terrorist, with `dummies` standing targets.
    /// False (and an error in the log) if it can't start.
    #[func]
    fn start_practice(&mut self, map: GString, dummies: i64) -> bool {
        self.stop();
        let text = map.to_string();
        let file = match sim::mapfile::MapFile::parse(&text) {
            Ok(m) => m,
            Err(e) => {
                godot_error!("bootleg16: {e}");
                return false;
            }
        };
        let assets = match net::server::Assets::load(&net::gamefiles::data_dir()) {
            Ok(a) => a,
            Err(e) => {
                godot_error!("bootleg16: {e}");
                return false;
            }
        };
        let hash = sim::mapfile::hash(text.as_bytes());
        let mut server = match net::Server::with_map(([127, 0, 0, 1], 0).into(), &file, hash, assets) {
            Ok(s) => s,
            Err(e) => {
                godot_error!("bootleg16: the practice server didn't start ({e})");
                return false;
            }
        };
        server.set_rules(net::server::Rules::practice(dummies.clamp(0, 20) as u8));
        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], server.local_addr().port()));
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        std::thread::spawn(move || server.run_until(&s));
        match net::Client::connect(addr, "player", file.world()) {
            Ok(c) => self.client = Some(c),
            Err(e) => {
                godot_error!("bootleg16: can't open a connection ({e})");
                stop.store(true, Ordering::Relaxed);
                return false;
            }
        }
        self.stop = Some(stop);
        self.joined = false;
        self.view.reset();
        true
    }

    /// The server has sent us the game at least once.
    #[func]
    fn has_snapshot(&self) -> bool {
        self.client.as_ref().is_some_and(|c| c.has_snapshot())
    }

    /// Steps a second: run `tick` this often (`Engine.physics_ticks_per_second`).
    #[func]
    fn tick_rate(&self) -> i64 {
        self.client.as_ref().map_or(net::tick::DEFAULT_TICK_RATE as i64, |c| c.rate().hz() as i64)
    }

    /// Welcomed by the server and in the game.
    #[func]
    fn is_playing(&self) -> bool {
        self.client.as_ref().is_some_and(|c| c.id().is_some() && c.vitals.alive())
    }

    /// One simulation step: movement (`forward`, `side` from -1 to 1), the camera's yaw and
    /// pitch (radians), the buttons held (bits: jump 1, crouch 2, walk 4, use 8, attack 16,
    /// attack2 32, reload 64, drop 128) and a weapon slot pressed this step (0 for none).
    #[func]
    fn tick(&mut self, forward: f32, side: f32, cam_yaw: f32, cam_pitch: f32, buttons: i64, slot: i64) {
        let Some(c) = &mut self.client else { return };
        if !self.joined && matches!(c.status, net::client::Status::Connected { .. }) {
            c.join_team(Some(Team::T));
            self.joined = true;
        }
        let (yaw, pitch) = view_to_sim(cam_yaw, cam_pitch);
        let b = |bit: i64| buttons & bit != 0;
        let input = sim::pmove::Input {
            forward: forward.clamp(-1.0, 1.0) * 400.0,
            side: side.clamp(-1.0, 1.0) * 400.0,
            yaw,
            pitch,
            jump: b(JUMP),
            crouch: b(CROUCH),
            walk: b(WALK),
            use_key: b(USE),
            attack: b(ATTACK),
            attack2: b(ATTACK2),
            reload: b(RELOAD),
            slot: slot.clamp(0, 255) as u8,
            drop: b(DROP),
        };
        self.prev_eye = c.me().eye();
        self.prev_punch = c.state.loadout.punch;
        c.tick(input);
    }

    /// Where the camera goes this frame (Godot metres): the predicted eye between the last two
    /// steps (`frac`: `Engine.get_physics_interpolation_fraction()`), with CS 1.6's bob and
    /// stair glide, and server corrections easing out. `dt`: this frame's length.
    #[func]
    fn eye(&mut self, frac: f32, dt: f32) -> Vector3 {
        let Some(c) = &mut self.client else { return Vector3::ZERO };
        let me = *c.me();
        let now = me.eye();
        let mut at: [f32; 3] = std::array::from_fn(|i| self.prev_eye[i] + (now[i] - self.prev_eye[i]) * frac);
        let moved = c.take_correction();
        let glide = self.view.correct(moved, dt);
        let speed = (me.velocity[0] * me.velocity[0] + me.velocity[1] * me.velocity[1]).sqrt();
        at[2] += self.view.update(dt, me.on_ground, speed, me.origin[2]);
        to_godot(std::array::from_fn(|i| at[i] + glide[i]))
    }

    /// The recoil punch to add to the camera this frame, degrees: x pitch (up), y yaw, z roll.
    #[func]
    fn punch(&self, frac: f32) -> Vector3 {
        let Some(c) = &self.client else { return Vector3::ZERO };
        let p = punch_at(self.prev_punch, c.state.loadout.punch, frac);
        // (GoldSrc's pitch punch is positive downwards)
        Vector3::new(-p[0], p[1], p[2])
    }

    /// The camera's vertical field of view in degrees (90 horizontal at 4:3, as CS; scopes zoom).
    #[func]
    fn fov(&self) -> f32 {
        let h = self.client.as_ref().and_then(|c| c.state.loadout.active_gun().map(|g| g.fov())).unwrap_or(90);
        vertical_fov(h as f32).to_degrees()
    }

    /// How fast you're moving across the ground (units a second, as CS shows it).
    #[func]
    fn speed(&self) -> f32 {
        self.client.as_ref().map_or(0.0, |c| {
            let v = c.me().velocity;
            (v[0] * v[0] + v[1] * v[1]).sqrt()
        })
    }

    /// Everyone else as drawn this frame: their feet (Godot metres).
    #[func]
    fn others_feet(&self, frac: f32) -> PackedVector3Array {
        let Some(c) = &self.client else { return PackedVector3Array::new() };
        c.others_at(frac).iter().filter(|o| o.alive).map(|o| to_godot([o.origin[0], o.origin[1], o.origin[2] - if o.crouched { 18.0 } else { 36.0 }])).collect()
    }

    /// Which way each of them faces (Godot yaw, radians), in `others_feet` order.
    #[func]
    fn others_yaw(&self, frac: f32) -> PackedFloat32Array {
        let Some(c) = &self.client else { return PackedFloat32Array::new() };
        c.others_at(frac).iter().filter(|o| o.alive).map(|o| yaw_to_godot(o.yaw)).collect()
    }

    /// The map's pieces broken right now (their numbers; the server puts them back each round).
    #[func]
    fn broken_pieces(&self) -> PackedInt32Array {
        self.client.as_ref().map_or_else(PackedInt32Array::new, |c| c.broken.iter().map(|m| *m as i32).collect())
    }

    /// Leave and stop the server.
    #[func]
    fn stop(&mut self) {
        if let Some(c) = &mut self.client {
            c.disconnect();
        }
        self.client = None;
        if let Some(s) = self.stop.take() {
            s.store(true, Ordering::Relaxed);
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if let Some(s) = self.stop.take() {
            s.store(true, Ordering::Relaxed);
        }
    }
}
