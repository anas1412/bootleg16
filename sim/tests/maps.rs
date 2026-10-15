//! Movement, doors and ladders on the real maps. Skipped when CS 1.6 isn't installed.
use goldsrc::bsp::Bsp;
use sim::collision::{Hull, World};
use sim::doors::{Actor, DoorState};
use sim::pmove::{step, Input, MoveVars, Player};

const DT: f32 = 1.0 / 128.0;

fn load(map: &str) -> Option<Bsp> {
    let home = std::env::var("HOME").ok()?;
    let dir = std::env::var("BOOTLEG16_DIR").unwrap_or(format!("{home}/.steam/steam/steamapps/common/Half-Life"));
    Bsp::load(format!("{dir}/cstrike/maps/{map}.bsp")).ok()
}

fn vec3(s: &str) -> [f32; 3] {
    let v: Vec<f32> = s.split_whitespace().map(|x| x.parse().unwrap()).collect();
    [v[0], v[1], v[2]]
}

fn actor(p: &Player, using: bool) -> Actor {
    let h = p.hull();
    Actor {
        mins: [p.origin[0] + h.mins()[0], p.origin[1] + h.mins()[1], p.origin[2] + h.mins()[2]],
        maxs: [p.origin[0] + h.maxs()[0], p.origin[1] + h.maxs()[1], p.origin[2] + h.maxs()[2]],
        eye: p.eye(),
        forward: [1.0, 0.0, 0.0],
        using,
    }
}

/// One tick of the whole world for a single player.
fn tick(w: &mut World, p: &mut Player, input: &Input) {
    step(p, input, DT, w, &MoveVars::default());
    w.update(DT, &[actor(p, false)]);
}

#[test]
fn spawns_land_and_players_can_run() {
    for map in ["de_dust2", "de_inferno", "de_nuke", "de_train"] {
        let Some(bsp) = load(map) else { return eprintln!("{map} not installed, skipping") };
        let mut world = World::from_bsp(&bsp);
        let spawns: Vec<[f32; 3]> = bsp
            .entity_list()
            .iter()
            .filter(|e| matches!(e.get("classname").map(String::as_str), Some("info_player_start" | "info_player_deathmatch")))
            .map(|e| vec3(&e["origin"]))
            .collect();
        assert!(spawns.len() >= 10, "{map}: found {} spawns", spawns.len());
        for (i, &s) in spawns.iter().enumerate() {
            assert!(!world.is_solid(Hull::Standing, s), "{map} spawn {i} at {s:?} is inside a wall");
            let mut p = Player::new(s);
            for _ in 0..128 {
                tick(&mut world, &mut p, &Input::default());
            }
            assert!(p.on_ground, "{map} spawn {i} never landed: {:?}", p.origin);
            for t in 0..512 {
                let input = Input { forward: 400.0, yaw: t as f32 * 0.7, jump: t % 90 == 0, ..Default::default() };
                tick(&mut world, &mut p, &input);
                assert!(!world.is_solid(p.hull(), p.origin), "{map} spawn {i}: stuck at tick {t}: {:?}", p.origin);
            }
            assert!(p.origin[2] > s[2] - 600.0, "{map} spawn {i}: fell out of the map to {:?}", p.origin);
        }
    }
}

#[test]
fn nuke_glass_doors_open_from_their_trigger_and_close_again() {
    let Some(bsp) = load("de_nuke") else { return };
    let mut world = World::from_bsp(&bsp);
    let ents = bsp.entity_list();
    let trig = ents.iter().find(|e| e.get("target").map(String::as_str) == Some("GlassDoor1")).expect("trigger");
    let m = bsp.models[trig["model"][1..].parse::<usize>().unwrap()];
    let centre = [(m.mins[0] + m.maxs[0]) / 2.0, (m.mins[1] + m.maxs[1]) / 2.0, (m.mins[2] + m.maxs[2]) / 2.0];
    let inside = Actor { mins: [centre[0] - 16.0, centre[1] - 16.0, centre[2] - 36.0], maxs: [centre[0] + 16.0, centre[1] + 16.0, centre[2] + 36.0], eye: centre, forward: [1.0, 0.0, 0.0], using: false };
    let doors = |w: &World| w.doors.doors.iter().filter(|d| d.name.as_deref() == Some("GlassDoor1")).map(|d| d.state).collect::<Vec<_>>();
    assert!(doors(&world).iter().all(|s| *s == DoorState::Closed));
    for _ in 0..(128 * 3) {
        world.update(DT, &[inside]);
    }
    assert!(doors(&world).iter().all(|s| matches!(s, DoorState::Open(_))), "{:?}", doors(&world));
    // Walk away: after wait (10 s) plus travel they close again.
    for _ in 0..(128 * 20) {
        world.update(DT, &[]);
    }
    assert!(doors(&world).iter().all(|s| *s == DoorState::Closed), "{:?}", doors(&world));
}

#[test]
fn inferno_rotating_door_opens_on_touch_and_lets_you_through() {
    let Some(bsp) = load("de_inferno") else { return };
    let mut world = World::from_bsp(&bsp);
    let door = world.doors.doors[0].clone();
    let (lo, hi) = door.bounds();
    // Stand right next to the closed door's thin side, on whichever side has room.
    let thin = if hi[0] - lo[0] < hi[1] - lo[1] { 0 } else { 1 };
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2] + 37.0];
    let mut start = mid;
    start[thin] = lo[thin] - 17.0;
    if world.is_solid(Hull::Standing, start) {
        start[thin] = hi[thin] + 17.0;
    }
    let p = Player::new(start);
    for _ in 0..(128 * 2) {
        world.update(DT, &[actor(&p, false)]);
    }
    let d = &world.doors.doors[0];
    assert!(matches!(d.state, DoorState::Open(_) | DoorState::Opening), "{:?}", d.state);
    assert!(d.transform().1[1].abs() > 45.0, "door only rotated to {:?}", d.transform().1);
}

#[test]
fn train_ladder_can_be_climbed() {
    let Some(bsp) = load("de_train") else { return };
    let mut world = World::from_bsp(&bsp);
    // Find a ladder with standing room in front of its base.
    let ladders = world.ladders.clone();
    let mut climbed = 0;
    for l in &ladders {
        let thin = if l.maxs[0] - l.mins[0] < l.maxs[1] - l.mins[1] { 0 } else { 1 };
        for side in [-1.0f32, 1.0] {
            let mut o = [(l.mins[0] + l.maxs[0]) / 2.0, (l.mins[1] + l.maxs[1]) / 2.0, l.mins[2] + 38.0];
            o[thin] = if side < 0.0 { l.mins[thin] - 16.5 } else { l.maxs[thin] + 16.5 };
            if world.is_solid(Hull::Standing, o) {
                continue;
            }
            // Face the ladder and look up while pressing forward.
            let yaw = if thin == 0 { if side < 0.0 { 0.0 } else { 180.0 } } else if side < 0.0 { 90.0 } else { 270.0 };
            let mut p = Player::new(o);
            let start_z = p.origin[2];
            for _ in 0..128 {
                tick(&mut world, &mut p, &Input { forward: 400.0, yaw, pitch: -60.0, ..Default::default() });
            }
            if p.origin[2] > start_z + 60.0 {
                climbed += 1;
            }
            break;
        }
    }
    assert!(climbed == ladders.len(), "climbed {climbed} of {} ladders", ladders.len());
}

#[test]
fn bullets_hit_floor_walls_and_sky_on_dust2() {
    let Some(bsp) = load("de_dust2") else { return };
    let world = World::from_bsp(&bsp);
    let spawns: Vec<[f32; 3]> = bsp
        .entity_list()
        .iter()
        .filter(|e| e.get("classname").map(String::as_str) == Some("info_player_deathmatch"))
        .map(|e| vec3(&e["origin"]))
        .collect();
    let mut sky_hits = 0;
    for s in &spawns {
        // Where a player lands, the floor is 36 units below its origin.
        let landed = world.trace(Hull::Standing, *s, [s[0], s[1], s[2] - 200.0]).end;
        let down = world.trace_line(landed, [s[0], s[1], s[2] - 200.0]);
        assert!(!down.start_solid && (down.end[2] - (landed[2] - 36.0)).abs() < 1.0, "floor under {landed:?} at {:?}", down.end);
        assert!(down.normal[2] > 0.7);
        // Every direction on the ground plane ends at a wall within the map.
        for yaw in (0..360).step_by(45) {
            let (y, x) = (yaw as f32).to_radians().sin_cos();
            let t = world.trace_line(*s, [s[0] + x * 8192.0, s[1] + y * 8192.0, s[2]]);
            assert!(t.fraction < 1.0, "shot from {s:?} at yaw {yaw} left the map");
        }
        sky_hits += world.trace_line(*s, [s[0], s[1], s[2] + 8192.0]).hit_sky as i32;
    }
    // T spawn on dust2 is outdoors.
    assert!(sky_hits > 0, "no spawn has open sky above it");
}

#[test]
fn hitboxes_from_the_real_player_model() {
    let Some(home) = std::env::var("HOME").ok() else { return };
    let dir = std::env::var("BOOTLEG16_DIR").unwrap_or(format!("{home}/.steam/steam/steamapps/common/Half-Life"));
    let Ok(model) = goldsrc::mdl::Model::load(format!("{dir}/cstrike/models/player/arctic/arctic.mdl")) else { return };
    use sim::hitbox::{HitGroup, Hitboxes};
    let hb = Hitboxes::from_model(&model, "carbine").expect("aim sequences");
    // Aim at the posed head's real centre (the rifle aim pose leans the head forward).
    let seq = model.sequence("ref_aim_carbine").unwrap();
    let pose = model.pose(seq, 0, 0.5);
    let head = model.hitboxes.iter().find(|h| h.group == 1).unwrap();
    let c = pose[head.bone].apply([(head.mins[0] + head.maxs[0]) / 2.0, (head.mins[1] + head.maxs[1]) / 2.0, (head.mins[2] + head.maxs[2]) / 2.0]);
    assert!(c[2] > 14.0 && c[2] < 24.0, "head centre at {c:?}");
    let shot = |from: [f32; 3], yaw: f32| {
        let d = [c[0] - from[0], c[1] - from[1], c[2] - from[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        hb.trace([0.0; 3], yaw, 0.0, false, from, [d[0] / l, d[1] / l, d[2] / l], 1000.0).map(|h| h.group)
    };
    // From the front-above and from above, the head is the first thing hit.
    assert_eq!(shot([200.0, c[1], c[2] + 60.0], 0.0), Some(HitGroup::Head));
    assert_eq!(shot([c[0], c[1], 300.0], 0.0), Some(HitGroup::Head));
    // Horizontal shots by height: legs low, body in the middle, nothing over the head.
    let level = |z: f32, crouched: bool| hb.trace([0.0; 3], 0.0, 0.0, crouched, [200.0, 0.0, z], [-1.0, 0.0, 0.0], 400.0).map(|h| h.group);
    assert!(matches!(level(-15.0, false), Some(HitGroup::LeftLeg | HitGroup::RightLeg)));
    assert!(matches!(level(10.0, false), Some(HitGroup::Chest | HitGroup::LeftArm | HitGroup::RightArm)));
    assert_eq!(level(30.0, false), None);
    // A rotated player is hit the same way: shooting the head of a player facing +y.
    let rotated = [-c[1], c[0], c[2]];
    let from = [rotated[0], rotated[1], 300.0];
    let d = [0.0, 0.0, -1.0];
    assert_eq!(hb.trace([0.0; 3], 90.0, 0.0, false, from, d, 1000.0).map(|h| h.group), Some(HitGroup::Head));
}

#[test]
fn bullets_know_what_texture_they_hit() {
    let Some(bsp) = load("de_dust2") else { return };
    let world = World::from_bsp(&bsp);
    let s = bsp.entity_list().iter().find(|e| e.get("classname").map(String::as_str) == Some("info_player_deathmatch")).map(|e| vec3(&e["origin"])).unwrap();
    let below = [s[0], s[1], s[2] - 500.0];
    let floor = world.texture_at(s, below).expect("floor texture");
    // Every texture name the trace can return comes from the map's own texture list.
    assert!(bsp.textures.iter().any(|t| t.name == floor), "{floor}");
    let mats = goldsrc::materials::Materials::parse(&std::fs::read_to_string(format!(
        "{}/.steam/steam/steamapps/common/Half-Life/cstrike/sound/materials.txt",
        std::env::var("HOME").unwrap()
    )).unwrap_or_default());
    eprintln!("dust2 T spawn floor: {floor} -> material {}", mats.kind(floor));
}
