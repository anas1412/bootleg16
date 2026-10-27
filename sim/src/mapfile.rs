//! Our map format (`.bmap`, JSON): what the rules need from a map built in Godot, written by
//! the Godot exporter (`godot/tools/export_map.gd`). Everything is in GoldSrc units (z up,
//! 1 unit = 1 inch), so the simulation reads it as it is. The look of the map lives in the
//! Godot scene beside it; this file is only what players, bullets and the rules touch.
use crate::breakables::Breakable;
use crate::brush::{Brush, Contents};
use crate::collision::{Ladder, World};
use crate::zones::{Side, Zones};
use crate::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MapFile {
    pub name: String,
    pub brushes: Vec<BrushDef>,
    pub spawns: Vec<Spawn>,
    /// Where each side buys. With none, CS's rule: near your own side's spawns.
    #[serde(default)]
    pub buy_zones: Vec<Zone>,
    #[serde(default)]
    pub bomb_sites: Vec<Site>,
    #[serde(default)]
    pub ladders: Vec<BoxDef>,
    /// Pieces that break (their brushes carry the same `model`).
    #[serde(default)]
    pub breakables: Vec<BreakableDef>,
    /// Doors (their brushes carry the same `model` and are relative to the door's pivot).
    /// Their order is the order the server sends their positions in.
    #[serde(default)]
    pub doors: Vec<DoorDef>,
    /// The bomb's blast reaches 3.5 times this (CS: `info_map_parameters bombradius`).
    #[serde(default = "bomb_radius")]
    pub bomb_radius: f32,
}

fn bomb_radius() -> f32 {
    500.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BrushDef {
    /// Outward face normals and distances (inside: dot(n, p) < d for all).
    pub planes: Vec<(Vec3, f32)>,
    /// Bounds of its corners.
    pub lo: Vec3,
    pub hi: Vec3,
    /// solid (default), clear (glass, fences: see through), sky, water.
    #[serde(default)]
    pub contents: ContentsDef,
    /// Surface material letter (CS's materials: C concrete, M metal, W wood...).
    #[serde(default = "concrete")]
    pub material: char,
    /// Map piece (0 = the world).
    #[serde(default)]
    pub model: usize,
}

fn concrete() -> char {
    'C'
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentsDef {
    #[default]
    Solid,
    Clear,
    Sky,
    Water,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SideDef {
    T,
    Ct,
}

impl From<SideDef> for Side {
    fn from(s: SideDef) -> Side {
        match s {
            SideDef::T => Side::T,
            SideDef::Ct => Side::Ct,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spawn {
    pub side: SideDef,
    /// A point at about waist height: the player drops onto the floor below it.
    pub origin: Vec3,
    /// The way it faces (degrees, GoldSrc: 0 = +x, counter-clockwise).
    #[serde(default)]
    pub yaw: f32,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BoxDef {
    pub lo: Vec3,
    pub hi: Vec3,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Zone {
    /// Which side buys here (none: both).
    #[serde(default)]
    pub side: Option<SideDef>,
    pub lo: Vec3,
    pub hi: Vec3,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Site {
    /// 'a' or 'b'.
    pub letter: char,
    pub lo: Vec3,
    pub hi: Vec3,
    /// Pieces with this `targetname` break when the bomb goes off here (dust2's crates).
    #[serde(default)]
    pub target: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DoorDef {
    pub model: usize,
    /// "slide" or "swing".
    pub kind: String,
    pub pivot: Vec3,
    /// Sliding: how far it moves (units). Swinging: degrees it turns (around the vertical).
    pub open: Vec3,
    /// Units (sliding) or degrees (swinging) a second.
    #[serde(default = "door_speed")]
    pub speed: f32,
    /// Seconds open before it closes; negative: stays open.
    #[serde(default = "door_wait")]
    pub wait: f32,
    /// Opened by a button or trigger with this name (else by walking into it or E).
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub use_only: bool,
    #[serde(default)]
    pub toggle: bool,
    #[serde(default)]
    pub start_open: bool,
    #[serde(default)]
    pub one_way: bool,
    /// Its node in the map scene (the client moves it).
    #[serde(default)]
    pub node: String,
}

fn door_speed() -> f32 {
    100.0
}

fn door_wait() -> f32 {
    3.0
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BreakableDef {
    /// Its brushes' `model`.
    pub model: usize,
    pub health: f32,
    /// glass, wood, metal, flesh, concrete, ceilingtile, computer, rocks: its break sound
    /// and debris.
    pub material: String,
    /// Only the bomb at a site naming it breaks it (not shots or grenades).
    #[serde(default)]
    pub trigger_only: bool,
    #[serde(default)]
    pub targetname: String,
    /// Its node in the map scene (the client hides it while broken).
    #[serde(default)]
    pub node: String,
}

impl MapFile {
    pub fn parse(text: &str) -> Result<MapFile, String> {
        serde_json::from_str(text).map_err(|e| format!("not a bootleg16 map: {e}"))
    }

    pub fn load(path: &std::path::Path) -> Result<(MapFile, u64), String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let map = MapFile::parse(std::str::from_utf8(&bytes).map_err(|e| format!("{}: {e}", path.display()))?)?;
        Ok((map, hash(&bytes)))
    }

    /// What players, bullets and sight collide with, and the map's zones.
    pub fn world(&self) -> World {
        let brushes = self
            .brushes
            .iter()
            .map(|b| {
                let contents = match b.contents {
                    ContentsDef::Solid => Contents::Solid,
                    ContentsDef::Clear => Contents::Clear,
                    ContentsDef::Sky => Contents::Sky,
                    ContentsDef::Water => Contents::Water,
                };
                Brush::new(b.planes.clone(), b.lo, b.hi, contents, b.material as u8, b.model)
            })
            .collect();
        let mut w = World::from_brushes(brushes);
        w.ladders = self.ladders.iter().map(|l| Ladder { mins: l.lo, maxs: l.hi }).collect();
        w.zones = self.zones();
        for d in &self.doors {
            let (mins, maxs) = self.model_bounds(d.model);
            let rotating = d.kind == "swing";
            w.doors.add(crate::doors::DoorSpec {
                model: d.model,
                name: (!d.name.is_empty()).then(|| d.name.clone()),
                pivot: d.pivot,
                mins,
                maxs,
                rotating,
                // (a swing turns around the vertical: GoldSrc's yaw)
                open_delta: if rotating { [0.0, d.open[1], 0.0] } else { d.open },
                speed: d.speed,
                wait: d.wait,
                use_only: d.use_only,
                toggle: d.toggle,
                start_open: d.start_open,
                one_way: d.one_way,
                move_sound: 1,
                stop_sound: 1,
            });
        }
        w.link_brush_doors();
        w
    }

    pub fn zones(&self) -> Zones {
        Zones::from_parts(
            self.buy_zones.iter().map(|z| (z.side.map(Side::from), z.lo, z.hi)).collect(),
            self.spawns.iter().map(|s| (Side::from(s.side), s.origin)).collect(),
            self.bomb_sites.iter().map(|s| ((s.lo, s.hi), Some(s.letter.to_ascii_lowercase()))).collect(),
            self.bomb_radius,
        )
    }

    /// The bounds of a piece's brushes (a door's: relative to its pivot).
    fn model_bounds(&self, model: usize) -> (Vec3, Vec3) {
        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
        for r in self.brushes.iter().filter(|r| r.model == model) {
            for i in 0..3 {
                lo[i] = lo[i].min(r.lo[i]);
                hi[i] = hi[i].max(r.hi[i]);
            }
        }
        (lo, hi)
    }

    /// The pieces that break, with their middles (from their brushes).
    pub fn breakables(&self) -> Vec<Breakable> {
        use crate::breakables::Material::*;
        self.breakables
            .iter()
            .map(|b| {
                let (lo, hi) = self.model_bounds(b.model);
                let material = match b.material.as_str() {
                    "wood" => Wood,
                    "metal" => Metal,
                    "flesh" => Flesh,
                    "concrete" => Concrete,
                    "ceilingtile" => CeilingTile,
                    "computer" => Computer,
                    "rocks" => Rocks,
                    _ => Glass,
                };
                Breakable { model: b.model, health: b.health.max(1.0), material, trigger_only: b.trigger_only, targetname: b.targetname.clone(), center: std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0) }
            })
            .collect()
    }

    /// Bomb sites that break pieces when the bomb goes off there: (lo, hi, target).
    pub fn site_targets(&self) -> Vec<(Vec3, Vec3, String)> {
        self.bomb_sites.iter().filter(|s| !s.target.is_empty()).map(|s| (s.lo, s.hi, s.target.clone())).collect()
    }

    /// The map's bounds (bots sample their walking graph inside them).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for b in &self.brushes {
            for i in 0..3 {
                lo[i] = lo[i].min(b.lo[i]);
                hi[i] = hi[i].max(b.hi[i]);
            }
        }
        (lo, hi)
    }
}

/// The map file's id (FNV-1a of its bytes, as for BSPs): a client with a different file
/// must not play on it.
pub fn hash(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_map_reads_and_plays() {
        let text = r#"{
            "name": "tiny",
            "brushes": [
                {"planes": [], "lo": [-512, -512, -64], "hi": [512, 512, 0]},
                {"planes": [], "lo": [200, -512, 0], "hi": [216, 512, 128], "contents": "clear", "material": "Y", "model": 3}
            ],
            "spawns": [{"side": "t", "origin": [0, 0, 40], "yaw": 90}, {"side": "ct", "origin": [100, 0, 40]}],
            "bomb_sites": [{"letter": "A", "lo": [-300, -300, 0], "hi": [-200, -200, 64]}]
        }"#;
        let map = MapFile::parse(text).unwrap();
        let w = map.world();
        // standing on the floor, blocked by the glass (seen through)
        let t = w.trace(crate::collision::Hull::Standing, [0.0, 0.0, 40.0], [0.0, 0.0, -100.0]);
        assert!((t.end[2] - 36.0).abs() < 0.1, "{t:?}");
        let line = w.trace_line([0.0, 0.0, 50.0], [400.0, 0.0, 50.0]);
        assert!((line.end[0] - 200.0).abs() < 0.1 && line.model == 3);
        assert!(!w.sight_blocked([0.0, 0.0, 50.0], [400.0, 0.0, 50.0]));
        assert_eq!(w.zones.spawns(Side::Ct), vec![[100.0, 0.0, 40.0]]);
        assert!(w.zones.in_bomb_site([-250.0, -250.0, 36.0], crate::collision::Hull::Standing));
        assert_eq!(w.zones.sites_a_first("tiny").len(), 1);
        // no buy zones: you buy near your side's spawn
        assert!(w.zones.in_buy_zone(Side::T, [10.0, 0.0, 36.0], crate::collision::Hull::Standing));
        assert_ne!(hash(text.as_bytes()), hash(b"other"));
    }

    /// A door 64 wide filling a gap in a wall at x 200 (between y -32 and 32).
    fn door_map(kind: &str, open: &str) -> World {
        let text = format!(
            r#"{{"name": "doors", "spawns": [],
            "brushes": [
                {{"planes": [], "lo": [-512, -512, -64], "hi": [512, 512, 0]}},
                {{"planes": [], "lo": [200, -512, 0], "hi": [216, -32, 128]}},
                {{"planes": [], "lo": [200, 32, 0], "hi": [216, 512, 128]}},
                {{"planes": [], "lo": [0, 0, 0], "hi": [16, 64, 112], "model": 1}}
            ],
            "doors": [{{"model": 1, "kind": "{kind}", "pivot": [200, -32, 0], "open": {open}, "speed": 400, "wait": 5}}]}}"#
        );
        MapFile::parse(&text).unwrap().world()
    }

    fn walk_through(mut w: World) -> bool {
        use crate::pmove::{step, Input, MoveVars, Player};
        let mut p = Player::new([100.0, 0.0, 40.0]);
        for _ in 0..(3 * 128) {
            step(&mut p, &Input { forward: 400.0, ..Default::default() }, 1.0 / 128.0, &w, &MoveVars::default());
            let (mins, maxs) = (crate::add(p.origin, p.hull().mins()), crate::add(p.origin, p.hull().maxs()));
            w.update(1.0 / 128.0, &[crate::doors::Actor { mins, maxs, eye: p.eye(), forward: [1.0, 0.0, 0.0], using: false }]);
        }
        p.origin[0] > 260.0
    }

    #[test]
    fn doors_block_until_they_open_and_you_walk_through() {
        // shut, the doorway stops a line; walking into it opens it (touch) and you pass
        let w = door_map("slide", "[0, 0, 110]");
        assert!(w.trace_line([100.0, 0.0, 50.0], [300.0, 0.0, 50.0]).fraction < 1.0);
        assert!(walk_through(w), "the sliding door let nobody through");
        let w = door_map("swing", "[0, 90, 0]");
        assert!(w.trace_line([100.0, 0.0, 50.0], [300.0, 0.0, 50.0]).fraction < 1.0);
        assert!(walk_through(w), "the swinging door let nobody through");
    }
}

