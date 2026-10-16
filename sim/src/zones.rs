//! Map areas with rules attached: where each team can buy, where the bomb can be planted,
//! and how big the bomb blast is. Read from the map's entities, so the server and the
//! client's prediction agree.
use crate::Vec3;
use goldsrc::bsp::Bsp;

/// Which side a zone belongs to (the BSP `team` key: 1 = terrorists, 2 = counter-terrorists).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    T,
    Ct,
}

#[derive(Clone, Debug, Default)]
pub struct Zones {
    /// Buy areas per side: boxes (mins, maxs).
    buy: Vec<(Side, Vec3, Vec3)>,
    /// Maps without `func_buyzone`: you can buy near your own team's spawn points.
    spawn_buy: Vec<(Side, Vec3)>,
    /// Bomb sites (`func_bomb_target`): boxes.
    pub bomb_sites: Vec<(Vec3, Vec3)>,
    /// `info_map_parameters bombradius` (default 500): the blast hurts out to 3.5 times this.
    pub bomb_radius: f32,
}

/// CS: without buy zones in the map, within this distance of your team's spawn points.
const SPAWN_BUY_RADIUS: f32 = 200.0;

impl Zones {
    pub fn from_bsp(bsp: &Bsp) -> Zones {
        let mut z = Zones { bomb_radius: 500.0, ..Default::default() };
        let bounds = |e: &std::collections::HashMap<String, String>| -> Option<(Vec3, Vec3)> {
            let i: usize = e.get("model")?.strip_prefix('*')?.parse().ok()?;
            let m = bsp.models.get(i)?;
            Some((m.mins, m.maxs))
        };
        let origin = |e: &std::collections::HashMap<String, String>| -> Option<Vec3> {
            let v: Vec<f32> = e.get("origin")?.split_whitespace().filter_map(|x| x.parse().ok()).collect();
            Some([*v.first()?, *v.get(1)?, *v.get(2)?])
        };
        for e in bsp.entity_list() {
            match e.get("classname").map(String::as_str) {
                Some("func_buyzone") => {
                    let side = if e.get("team").map(String::as_str) == Some("1") { Side::T } else { Side::Ct };
                    if let Some((lo, hi)) = bounds(&e) {
                        z.buy.push((side, lo, hi));
                    }
                }
                Some("func_bomb_target") => z.bomb_sites.extend(bounds(&e)),
                Some("info_player_deathmatch") => z.spawn_buy.extend(origin(&e).map(|o| (Side::T, o))),
                Some("info_player_start") => z.spawn_buy.extend(origin(&e).map(|o| (Side::Ct, o))),
                Some("info_map_parameters") => {
                    if let Some(r) = e.get("bombradius").and_then(|r| r.parse().ok()) {
                        z.bomb_radius = r;
                    }
                }
                _ => {}
            }
        }
        z
    }

    pub fn in_buy_zone(&self, side: Side, at: Vec3) -> bool {
        if self.buy.is_empty() {
            return self.spawn_buy.iter().any(|(s, o)| *s == side && (0..3).map(|i| (o[i] - at[i]).powi(2)).sum::<f32>() < SPAWN_BUY_RADIUS * SPAWN_BUY_RADIUS);
        }
        self.buy.iter().any(|(s, lo, hi)| *s == side && inside(at, *lo, *hi))
    }

    pub fn in_bomb_site(&self, at: Vec3) -> bool {
        self.bomb_sites.iter().any(|(lo, hi)| inside(at, *lo, *hi))
    }
}

fn inside(p: Vec3, lo: Vec3, hi: Vec3) -> bool {
    (0..3).all(|i| p[i] >= lo[i] && p[i] <= hi[i])
}
