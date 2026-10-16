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
    for map in ["de_dust2", "de_inferno", "de_nuke", "de_train", "de_mirage"] {
        let Some(bsp) = load(map) else {
            eprintln!("{map} not installed, skipping");
            continue;
        };
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
        hb.trace([0.0; 3], yaw, 0.0, false, sim::weapons::Id::M4a1, from, [d[0] / l, d[1] / l, d[2] / l], 1000.0).map(|h| h.group)
    };
    // From the front-above and from above, the head is the first thing hit.
    assert_eq!(shot([200.0, c[1], c[2] + 60.0], 0.0), Some(HitGroup::Head));
    assert_eq!(shot([c[0], c[1], 300.0], 0.0), Some(HitGroup::Head));
    // Horizontal shots by height: legs low, body in the middle, nothing over the head.
    let level = |z: f32, crouched: bool| hb.trace([0.0; 3], 0.0, 0.0, crouched, sim::weapons::Id::M4a1, [200.0, 0.0, z], [-1.0, 0.0, 0.0], 400.0).map(|h| h.group);
    assert!(matches!(level(-15.0, false), Some(HitGroup::LeftLeg | HitGroup::RightLeg)));
    assert!(matches!(level(10.0, false), Some(HitGroup::Chest | HitGroup::LeftArm | HitGroup::RightArm)));
    assert_eq!(level(30.0, false), None);
    // Close range at eye level (+17) to the face is a headshot; 9 units higher (8° up at
    // 64 units, as an old test aimed) goes over the head.
    let front = |z: f32, w| hb.trace([0.0; 3], 180.0, 0.0, false, w, [-64.0, -c[1], z], [1.0, 0.0, 0.0], 200.0).map(|h| h.group);
    assert_eq!(front(18.0, sim::weapons::Id::M4a1), Some(HitGroup::Head));
    assert_eq!(front(26.0, sim::weapons::Id::M4a1), None);
    // Hitboxes follow the weapon's pose: each gun's real head centre is a headshot, from the side.
    for (w, ext) in [(sim::weapons::Id::Ak47, "ak47"), (sim::weapons::Id::Knife, "knife"), (sim::weapons::Id::Awp, "rifle"), (sim::weapons::Id::Glock, "onehanded")] {
        for (stance, crouched) in [("ref", false), ("crouch", true)] {
            let p = model.pose(model.sequence(&format!("{stance}_aim_{ext}")).unwrap(), 0, 0.5);
            let hc = p[head.bone].apply([(head.mins[0] + head.maxs[0]) / 2.0, (head.mins[1] + head.maxs[1]) / 2.0, (head.mins[2] + head.maxs[2]) / 2.0]);
            let side = hb.trace([0.0; 3], 0.0, 0.0, crouched, w, [hc[0], -200.0, hc[2]], [0.0, 1.0, 0.0], 400.0).map(|h| h.group);
            assert_eq!(side, Some(HitGroup::Head), "{stance} {ext} head at {hc:?}");
        }
    }
    // A rotated player is hit the same way: shooting the head of a player facing +y.
    let rotated = [-c[1], c[0], c[2]];
    let from = [rotated[0], rotated[1], 300.0];
    let d = [0.0, 0.0, -1.0];
    assert_eq!(hb.trace([0.0; 3], 90.0, 0.0, false, sim::weapons::Id::M4a1, from, d, 1000.0).map(|h| h.group), Some(HitGroup::Head));
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

mod shooting {
    use super::*;
    use sim::hitbox::{HitGroup, Hitboxes};
    use sim::weapons::bullets::{fire_bullet, Target};
    use sim::weapons::{gun, Id, Shot};

    fn setup() -> Option<(World, Hitboxes, goldsrc::materials::Materials, [f32; 3])> {
        let bsp = load("de_dust2")?;
        let home = std::env::var("HOME").ok()?;
        let dir = std::env::var("BOOTLEG16_DIR").unwrap_or(format!("{home}/.steam/steam/steamapps/common/Half-Life"));
        let model = goldsrc::mdl::Model::load(format!("{dir}/cstrike/models/player/arctic/arctic.mdl")).ok()?;
        let mats = goldsrc::materials::Materials::parse(&std::fs::read_to_string(format!("{dir}/cstrike/sound/materials.txt")).ok()?);
        let world = World::from_bsp(&bsp);
        // Stand on the floor at the first T spawn.
        let s = bsp.entity_list().iter().find(|e| e.get("classname").map(String::as_str) == Some("info_player_deathmatch")).map(|e| vec3(&e["origin"]))?;
        let origin = world.trace(Hull::Standing, s, [s[0], s[1], s[2] - 200.0]).end;
        Some((world, Hitboxes::from_model(&model, "carbine")?, mats, origin))
    }

    /// A direction from `origin` with at least `need` units of open floor ahead.
    fn open_direction(world: &World, origin: [f32; 3], need: f32) -> f32 {
        (0..72)
            .map(|i| i as f32 * 5.0)
            .find(|yaw| {
                let (s, c) = yaw.to_radians().sin_cos();
                (0..3).all(|k| {
                    let z = origin[2] - 20.0 + k as f32 * 25.0;
                    world.trace_line([origin[0], origin[1], z], [origin[0] + c * need, origin[1] + s * need, z]).fraction == 1.0
                })
            })
            .expect("some open direction")
    }

    fn ak_shot(pitch: f32, yaw: f32) -> Shot {
        let g = gun(Id::Ak47).unwrap();
        Shot { weapon: Id::Ak47, damage: 36, range_mod: 0.98, penetration: g.penetration, distance: g.distance, bullet: g.bullet, spread: 0.0, angles: [pitch, yaw], side_offset: 0.0, seed: 1, pellets: None, silenced: false }
    }

    #[test]
    fn ak_hits_a_player_500_units_away_with_falloff() {
        let Some((world, hb, mats, me)) = setup() else { return };
        let yaw = open_direction(&world, me, 700.0);
        let (s, c) = yaw.to_radians().sin_cos();
        let target = Target { id: 1, origin: [me[0] + c * 500.0, me[1] + s * 500.0, me[2]], yaw: yaw + 180.0, pitch: 0.0, crouched: false, weapon: Id::M4a1 };
        let eye = [me[0], me[1], me[2] + 17.0];
        // Aim level at chest height (target origin + 9).
        let pitch = -((target.origin[2] + 9.0 - eye[2]) / 500.0).atan().to_degrees();
        let r = fire_bullet(&world, &mats, &hb, eye, &ak_shot(pitch, yaw), 0, &[target]);
        assert_eq!(r.hits.len(), 1, "{r:?}");
        // 36 * 0.98^(500/500) = 35.28 -> 35 (integer), chest x1 / arms x1 / stomach x1.25.
        assert!(matches!(r.hits[0].group, HitGroup::Chest | HitGroup::LeftArm | HitGroup::RightArm | HitGroup::Stomach), "{:?}", r.hits[0]);
        assert!((r.hits[0].damage - 35.0 * sim::combat::hitgroup_multiplier(r.hits[0].group)).abs() < 0.01, "{:?}", r.hits[0]);
    }

    #[test]
    fn bullets_pass_through_one_player_into_the_next() {
        let Some((world, hb, mats, me)) = setup() else { return };
        let yaw = open_direction(&world, me, 700.0);
        let (s, c) = yaw.to_radians().sin_cos();
        let at = |d: f32, id| Target { id, origin: [me[0] + c * d, me[1] + s * d, me[2]], yaw: yaw + 180.0, pitch: 0.0, crouched: false, weapon: Id::M4a1 };
        let eye = [me[0], me[1], me[2] + 17.0];
        let pitch = -((me[2] + 9.0 - eye[2]) / 300.0).atan().to_degrees();
        let r = fire_bullet(&world, &mats, &hb, eye, &ak_shot(pitch, yaw), 0, &[at(200.0, 1), at(300.0, 2)]);
        assert_eq!(r.hits.iter().map(|h| h.victim).collect::<Vec<_>>(), vec![1, 2], "{r:?}");
        let base = |h: &sim::weapons::bullets::BulletHit| h.damage / sim::combat::hitgroup_multiplier(h.group);
        assert!(base(&r.hits[1]) < base(&r.hits[0]) * 0.8, "second victim takes less: {r:?}");
    }

    #[test]
    fn a_thick_wall_stops_the_bullet() {
        let Some((world, hb, mats, me)) = setup() else { return };
        // Shoot straight down: the bullet hits the floor and the target "under" it is safe.
        let below = Target { id: 1, origin: [me[0], me[1], me[2] - 300.0], yaw: 0.0, pitch: 0.0, crouched: false, weapon: Id::M4a1 };
        let eye = [me[0], me[1], me[2] + 17.0];
        let r = fire_bullet(&world, &mats, &hb, eye, &ak_shot(89.0, 0.0), 0, &[below]);
        assert!(r.hits.is_empty(), "{r:?}");
        assert!(!r.impacts.is_empty());
    }
}

#[test]
fn bomb_site_a_is_the_right_one_on_each_map() {
    // (map, A site centre, B site centre), from the maps themselves (de_dust2 names them) and
    // what's known of each layout. de_train isn't in the list: not verified yet.
    let expected = [("de_dust2", [1152.0, 2464.0], [-1536.0, 2688.0]), ("de_inferno", [1984.0, 456.0], [288.0, 2784.0]), ("de_nuke", [680.0, -776.0], [648.0, -904.0]), ("de_mirage", [-688.0, -1168.0], [-2312.0, 1288.0])];
    for (map, a, b) in expected {
        let Some(bsp) = load(map) else { continue };
        let sites = sim::zones::Zones::from_bsp(&bsp).sites_a_first(map);
        let near = |s: [f32; 3], p: [f32; 2]| (s[0] - p[0]).abs() < 1.0 && (s[1] - p[1]).abs() < 1.0;
        assert!(near(sites[0], a) && near(sites[1], b), "{map}: {sites:?}");
    }
}

#[test]
fn closed_doors_block_sight_open_ones_and_glass_dont() {
    let Some(bsp) = load("de_inferno") else { return };
    let mut world = World::from_bsp(&bsp);
    let (lo, hi) = world.doors.doors[0].bounds();
    // A line straight through the middle of the closed door.
    let thin = if hi[0] - lo[0] < hi[1] - lo[1] { 0 } else { 1 };
    let mid = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2] + 40.0];
    let (mut a, mut b) = (mid, mid);
    a[thin] = lo[thin] - 40.0;
    b[thin] = hi[thin] + 40.0;
    assert!(world.sight_blocked(a, b), "seen through a closed door");
    let mut p = Player::new(a);
    p.origin[thin] = lo[thin] - 17.0;
    for _ in 0..(128 * 2) {
        world.update(DT, &[actor(&p, false)]);
    }
    assert!(matches!(world.doors.doors[0].state, DoorState::Open(_)), "{:?}", world.doors.doors[0].state);
    assert!(!world.sight_blocked(a, b), "an open door still blocks sight");

    // Nuke's glass doors: closed, but see-through.
    let Some(bsp) = load("de_nuke") else { return };
    let world = World::from_bsp(&bsp);
    let d = world.doors.doors.iter().find(|d| d.name.as_deref() == Some("GlassDoor1")).unwrap();
    let (lo, hi) = d.bounds();
    let thin = if hi[0] - lo[0] < hi[1] - lo[1] { 0 } else { 1 };
    // Somewhere across its face a line meets the glass first (not its frame) and still sees through.
    let wide = 1 - thin;
    let through_glass = (1..10).flat_map(|i| (1..10).map(move |k| (i, k))).any(|(i, k)| {
        let mut a = [0.0; 3];
        a[wide] = lo[wide] + (hi[wide] - lo[wide]) * i as f32 / 10.0;
        a[2] = lo[2] + (hi[2] - lo[2]) * k as f32 / 10.0;
        let mut b = a;
        a[thin] = lo[thin] - 40.0;
        b[thin] = hi[thin] + 40.0;
        world.trace_line(a, b).model == d.model && !world.sight_blocked(a, b)
    });
    assert!(through_glass, "glass hides players");
}

#[test]
fn knife_line_first_then_a_crouch_box_sweep() {
    use sim::hitbox::{HitGroup, Hitboxes};
    use sim::weapons::bullets::{trace_melee, Target};
    use sim::weapons::Id;
    let Some(bsp) = load("de_dust2") else { return };
    let home = std::env::var("HOME").unwrap();
    let dir = std::env::var("BOOTLEG16_DIR").unwrap_or(format!("{home}/.steam/steam/steamapps/common/Half-Life"));
    let Ok(model) = goldsrc::mdl::Model::load(format!("{dir}/cstrike/models/player/arctic/arctic.mdl")) else { return };
    let hb = Hitboxes::from_model(&model, "carbine").unwrap();
    let world = World::from_bsp(&bsp);
    let s = bsp.entity_list().iter().find(|e| e.get("classname").map(String::as_str) == Some("info_player_deathmatch")).map(|e| vec3(&e["origin"])).unwrap();
    let me = world.trace(Hull::Standing, s, [s[0], s[1], s[2] - 200.0]).end;
    // An open direction: 100 units clear at eye height.
    let eye = [me[0], me[1], me[2] + 17.0];
    let yaw = (0..72).map(|i| i as f32 * 5.0).find(|y| {
        let (s, c) = y.to_radians().sin_cos();
        world.trace(Hull::Crouched, eye, [eye[0] + c * 100.0, eye[1] + s * 100.0, eye[2]]).fraction == 1.0
    }).unwrap();
    let (sn, cs) = yaw.to_radians().sin_cos();
    let end = [eye[0] + cs * 48.0, eye[1] + sn * 48.0, eye[2]];
    // Someone `side` units to the side of the swing, 40 ahead.
    let at = |side: f32| Target { id: 1, origin: [me[0] + cs * 40.0 - sn * side, me[1] + sn * 40.0 + cs * side, me[2]], yaw: yaw + 180.0, pitch: 0.0, crouched: false, weapon: Id::M4a1 };
    let hit = |side| trace_melee(&world, &hb, eye, end, 0, &[at(side)]);
    assert!(matches!(hit(0.0), Some((1, g)) if g != HitGroup::Generic), "straight on: the line hits a body part");
    // Off to the side: the line misses, the box (16 + their 16) still catches them.
    assert_eq!(hit(28.0), Some((1, HitGroup::Generic)));
    assert_eq!(hit(-28.0), Some((1, HitGroup::Generic)));
    assert_eq!(hit(40.0), None, "out of reach");
}
