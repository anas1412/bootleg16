//! Map areas with rules attached: where each team can buy, where the bomb can be planted,
//! and how big the bomb blast is. Read from the map's entities, so the server and the
//! client's prediction agree.
use crate::collision::Hull;
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
    /// Buy areas: boxes (mins, maxs), for one side or (None, `team` 0) both.
    buy: Vec<(Option<Side>, Vec3, Vec3)>,
    /// Maps without `func_buyzone`: you can buy near your own team's spawn points.
    spawn_buy: Vec<(Side, Vec3)>,
    /// Bomb sites (`func_bomb_target`): boxes.
    pub bomb_sites: Vec<(Vec3, Vec3)>,
    /// Each site's own name where the map gives one ('a' / 'b' from targets like `tgt_a`).
    site_names: Vec<Option<char>>,
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
                    // CBuyZone::BuyTouch: team 0 (or none) lets both sides buy
                    let side = match e.get("team").map(String::as_str) {
                        Some("1") => Some(Side::T),
                        Some("2") => Some(Side::Ct),
                        _ => None,
                    };
                    if let Some((lo, hi)) = bounds(&e) {
                        z.buy.push((side, lo, hi));
                    }
                }
                Some("func_bomb_target") => {
                    if let Some(b) = bounds(&e) {
                        z.bomb_sites.push(b);
                        let name = e.get("target").or(e.get("targetname")).and_then(|t| match t.to_ascii_lowercase().chars().last() {
                            Some(c @ ('a' | 'b')) if t.len() > 1 && !t[..t.len() - 1].ends_with(|c: char| c.is_ascii_alphabetic()) => Some(c),
                            _ => None,
                        });
                        z.site_names.push(name);
                    }
                }
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

    /// Whether a player there (`hull`: their box) may buy. CS 1.6: the zone is a trigger, so
    /// any part of the player touching it counts; without zones, within 200 units of one of
    /// your team's spawn points (from the origin, `CBasePlayer::HandleSignals`).
    pub fn in_buy_zone(&self, side: Side, at: Vec3, hull: Hull) -> bool {
        if self.buy.is_empty() {
            return self.spawn_buy.iter().any(|(s, o)| *s == side && (0..3).map(|i| (o[i] - at[i]).powi(2)).sum::<f32>() < SPAWN_BUY_RADIUS * SPAWN_BUY_RADIUS);
        }
        self.buy.iter().any(|(s, lo, hi)| s.is_none_or(|s| s == side) && touches(at, hull, *lo, *hi))
    }

    /// The bomb sites' centres, A first. Names come from the map where it has them (de_dust2's
    /// `tgt_a`); else from what's known of the map (Inferno: B is the north site, up banana;
    /// Nuke: A is the upper site; Mirage, BubkeZ's original: A is the south site; Train: A is
    /// the big middle yard, B the lower yard by the CT spawn, so A is the west one); else the
    /// east site is A.
    pub fn sites_a_first(&self, map: &str) -> Vec<Vec3> {
        let centre = |(lo, hi): &(Vec3, Vec3)| [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2] + 36.0];
        let mut sites: Vec<(Vec3, Option<char>)> = self.bomb_sites.iter().zip(&self.site_names).map(|(b, n)| (centre(b), *n)).collect();
        if sites.iter().all(|(_, n)| n.is_some()) {
            sites.sort_by_key(|(_, n)| *n);
        } else {
            match map {
                "de_inferno" => sites.sort_by(|a, b| a.0[1].total_cmp(&b.0[1])), // north (bigger y) is B
                "de_nuke" => sites.sort_by(|a, b| b.0[2].total_cmp(&a.0[2])),    // upper is A
                "de_mirage" => sites.sort_by(|a, b| a.0[1].total_cmp(&b.0[1])),  // south (smaller y) is A
                "de_train" => sites.sort_by(|a, b| a.0[0].total_cmp(&b.0[0])),   // west (smaller x) is A
                _ => sites.sort_by(|a, b| b.0[0].total_cmp(&a.0[0])),
            }
        }
        sites.into_iter().map(|(c, _)| c).collect()
    }

    /// A side's spawn points (`info_player_deathmatch`: terrorists, `info_player_start`:
    /// counter-terrorists), in map order.
    pub fn spawns(&self, side: Side) -> Vec<Vec3> {
        self.spawn_buy.iter().filter(|(s, _)| *s == side).map(|(_, o)| *o).collect()
    }

    /// Whether a player there (`hull`) is on a bomb site (touching its trigger, as CS 1.6).
    pub fn in_bomb_site(&self, at: Vec3, hull: Hull) -> bool {
        self.bomb_sites.iter().any(|(lo, hi)| touches(at, hull, *lo, *hi))
    }
}

/// A player's box at `p` overlaps the box lo..hi (how a trigger is touched).
fn touches(p: Vec3, hull: Hull, lo: Vec3, hi: Vec3) -> bool {
    let (mins, maxs) = (hull.mins(), hull.maxs());
    (0..3).all(|i| p[i] + maxs[i] >= lo[i] && p[i] + mins[i] <= hi[i])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zone_counts_once_any_part_of_you_touches_it_and_team_0_is_both_sides() {
        let z = Zones { buy: vec![(Some(Side::T), [0.0, 0.0, 0.0], [100.0, 100.0, 100.0]), (None, [500.0, 0.0, 0.0], [600.0, 100.0, 100.0])], ..Default::default() };
        // centre 10 units outside, but the 16-unit half width reaches in
        assert!(z.in_buy_zone(Side::T, [110.0, 50.0, 50.0], Hull::Standing));
        assert!(!z.in_buy_zone(Side::T, [120.0, 50.0, 50.0], Hull::Standing));
        assert!(!z.in_buy_zone(Side::Ct, [50.0, 50.0, 50.0], Hull::Standing));
        assert!(z.in_buy_zone(Side::Ct, [550.0, 50.0, 50.0], Hull::Crouched));
        // standing reaches 36 below its centre, crouched 18
        assert!(z.in_buy_zone(Side::T, [50.0, 50.0, 130.0], Hull::Standing));
        assert!(!z.in_buy_zone(Side::T, [50.0, 50.0, 130.0], Hull::Crouched));
    }
}
