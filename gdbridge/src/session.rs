//! A game session: practice on our own server on this machine (as the Bevy client's Solo
//! practice), or a match the lobby found. The client plays with prediction; Godot calls `tick`
//! at the server's rate from `_physics_process` and reads where to put the camera.
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
    pub(crate) client: Option<net::Client>,
    /// Stops our server thread (also when the session goes away).
    stop: Option<Arc<AtomicBool>>,
    joined: bool,
    view: View,
    /// The eye and recoil punch before the last tick (frames are drawn between ticks).
    prev_eye: [f32; 3],
    prev_punch: [f32; 3],
    /// The longest defuse time left seen since a defuse began: the client only hears what's
    /// left, and the HUD's ring needs the whole length (`game::hud`).
    pub(crate) defuse_seen: std::cell::Cell<f32>,
    /// Who we heard last and when, by player id (`game::voices`).
    pub(crate) heard: std::collections::HashMap<u8, crate::game::Heard>,
    /// In a practice match: the bots' names (they join like players, so the server doesn't
    /// flag them; `game::players` does).
    pub(crate) bots: &'static [&'static str],
    base: Base<RefCounted>,
}

#[godot_api]
impl IRefCounted for Session {
    fn init(base: Base<RefCounted>) -> Self {
        Session {
            client: None,
            stop: None,
            joined: false,
            view: View::default(),
            prev_eye: [0.0; 3],
            prev_punch: [0.0; 3],
            defuse_seen: Default::default(),
            heard: Default::default(),
            bots: &[],
            base,
        }
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

    /// Start a practice match on this machine on a map (the text of its `.bmap`): the real
    /// rules (`practice::config`), you (`name`) on the Strays (attackers) and bots filling both teams.
    /// False (and an error in the log) if it can't start.
    #[func]
    fn start_match(&mut self, map: GString, name: GString) -> bool {
        self.stop();
        let text = map.to_string();
        let (addr, stop) = match crate::practice::start(&text, Team::T) {
            Ok(host) => host,
            Err(e) => {
                godot_error!("bootleg16: {e}");
                return false;
            }
        };
        let world = sim::mapfile::MapFile::parse(&text).map(|f| f.world()).unwrap_or_else(|_| sim::collision::World::flat_floor(-100_000.0));
        match net::Client::connect(addr, &name.to_string(), world) {
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
        self.bots = &crate::practice::NAMES;
        true
    }

    /// Go to a match the lobby found (`Lobby.take_found`): the server's address, your seat's
    /// ticket and key. The server names the map once it welcomes us (`map_name`); load that
    /// map and hand it over with `set_map` before playing. False if the address is bad.
    #[func]
    fn connect_match(&mut self, address: GString, ticket: i64, key: PackedByteArray, name: GString) -> bool {
        self.stop();
        let Ok(addr) = address.to_string().parse::<std::net::SocketAddr>() else {
            godot_error!("bootleg16: bad server address {address}");
            return false;
        };
        let Ok(key) = <[u8; 32]>::try_from(key.as_slice()) else {
            godot_error!("bootleg16: a seat key is 32 bytes");
            return false;
        };
        // no map yet: a floor far below until the server says which map it is
        match net::Client::connect(addr, &name.to_string(), sim::collision::World::flat_floor(-100_000.0)) {
            Ok(mut c) => {
                c.ticket = ticket as u64;
                c.key = Some(key);
                self.client = Some(c);
            }
            Err(e) => {
                godot_error!("bootleg16: can't open a connection ({e})");
                return false;
            }
        }
        // a reserved match puts us on our team itself
        self.joined = true;
        self.view.reset();
        true
    }

    /// "connecting", "connected", "rejected" (see `reject_reason`) or "" with no session.
    #[func]
    fn status(&self) -> GString {
        match self.client.as_ref().map(|c| &c.status) {
            Some(net::client::Status::Connecting) => "connecting",
            Some(net::client::Status::Connected { .. }) => "connected",
            Some(net::client::Status::Rejected) => "rejected",
            None => "",
        }
        .into()
    }

    /// Why the server refused us ("" if it didn't).
    #[func]
    fn reject_reason(&self) -> GString {
        GString::from(self.client.as_ref().and_then(|c| c.reject_reason.clone()).unwrap_or_default().as_str())
    }

    /// The map the server is running ("" until it welcomes us).
    #[func]
    fn map_name(&self) -> GString {
        GString::from(self.client.as_ref().and_then(|c| c.map.clone()).unwrap_or_default().as_str())
    }

    /// The map the rules collide with: the text of its `.bmap`. False if it isn't the same
    /// file the server runs (another version of the map: we'd walk through its walls).
    #[func]
    fn set_map(&mut self, text: GString) -> bool {
        let Some(c) = &mut self.client else { return false };
        let text = text.to_string();
        if c.map_hash != 0 && sim::mapfile::hash(text.as_bytes()) != c.map_hash {
            godot_error!("bootleg16: this map isn't the version the server runs");
            return false;
        }
        match sim::mapfile::MapFile::parse(&text) {
            Ok(file) => {
                c.world = file.world();
                true
            }
            Err(e) => {
                godot_error!("bootleg16: {e}");
                false
            }
        }
    }

    /// The match is over (back to the menu).
    #[func]
    fn match_ended(&self) -> bool {
        self.client.as_ref().is_some_and(|c| c.match_ended)
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

    /// How far each door has slid from where it was placed (Godot metres), in the map
    /// file's door order.
    #[func]
    fn door_offsets(&self) -> PackedVector3Array {
        self.client.as_ref().map_or_else(PackedVector3Array::new, |c| c.world.doors.doors.iter().map(|d| to_godot(d.transform().0)).collect())
    }

    /// How far each door has swung (radians, around the vertical), in door order.
    #[func]
    fn door_turns(&self) -> PackedFloat32Array {
        self.client.as_ref().map_or_else(PackedFloat32Array::new, |c| c.world.doors.doors.iter().map(|d| d.transform().1[1].to_radians()).collect())
    }

    /// Leave and stop the server.
    #[func]
    fn stop(&mut self) {
        if let Some(c) = &mut self.client {
            c.disconnect();
        }
        self.client = None;
        self.bots = &[];
        self.heard.clear();
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
