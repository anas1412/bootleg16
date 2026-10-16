//! `func_breakable` brushes: glass, vents and crates that break when shot or blown up.
//! Read from the map so the server and clients agree on which pieces exist.
use crate::Vec3;
use goldsrc::bsp::Bsp;

/// What a breakable is made of (the BSP `material` key), for its break sound and debris.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Material {
    Glass,
    Wood,
    Metal,
    Flesh,
    Concrete,
    CeilingTile,
    Computer,
    UnbreakableGlass,
    Rocks,
}

impl Material {
    fn from_key(v: i32) -> Material {
        use Material::*;
        [Glass, Wood, Metal, Flesh, Concrete, CeilingTile, Computer, UnbreakableGlass, Rocks].get(v.max(0) as usize).copied().unwrap_or(Glass)
    }
}

#[derive(Clone, Debug)]
pub struct Breakable {
    /// The brush model (`*N`).
    pub model: usize,
    pub health: f32,
    pub material: Material,
    /// Spawnflag 1: only breaks when triggered (dust2's crates, by the bomb going off at its site).
    pub trigger_only: bool,
    pub targetname: String,
    /// Middle of the brush (world space), for the break sound and debris.
    pub center: Vec3,
}

pub fn from_bsp(bsp: &Bsp) -> Vec<Breakable> {
    bsp.entity_list()
        .iter()
        .filter(|e| e.get("classname").map(String::as_str) == Some("func_breakable"))
        .filter_map(|e| {
            let model: usize = e.get("model")?.strip_prefix('*')?.parse().ok()?;
            let m = bsp.models.get(model)?;
            let num = |k: &str| e.get(k).and_then(|v| v.parse::<f32>().ok());
            let flags = num("spawnflags").unwrap_or(0.0) as i32;
            Some(Breakable {
                model,
                health: num("health").unwrap_or(1.0).max(1.0),
                material: Material::from_key(num("material").unwrap_or(0.0) as i32),
                trigger_only: flags & 1 != 0,
                targetname: e.get("targetname").cloned().unwrap_or_default(),
                center: std::array::from_fn(|i| (m.mins[i] + m.maxs[i]) / 2.0),
            })
        })
        .filter(|b| b.material != Material::UnbreakableGlass)
        .collect()
}

/// Targets fired when the bomb goes off in a site: the site's `target` key (dust2: `tgt_a`).
pub fn bomb_site_targets(bsp: &Bsp) -> Vec<(Vec3, Vec3, String)> {
    bsp.entity_list()
        .iter()
        .filter(|e| e.get("classname").map(String::as_str) == Some("func_bomb_target"))
        .filter_map(|e| {
            let model: usize = e.get("model")?.strip_prefix('*')?.parse().ok()?;
            let m = bsp.models.get(model)?;
            Some((m.mins, m.maxs, e.get("target")?.clone()))
        })
        .collect()
}
