//! Bots and a match on our own map format (the exported Godot test arena), no Valve files.
mod common;
use common::*;
use net::bot::BotMap;

#[test]
fn bots_find_their_way_from_spawn_to_both_bomb_sites() {
    let (map, hash) = arena();
    let bots = BotMap::from_map(map, hash);
    assert_eq!(bots.sites.len(), 2, "both sites");
    let none = std::collections::HashSet::new();
    let from = bots.nav.nearest(bots.spawns.0).expect("a walkable point by the Strays spawn");
    for (i, site) in bots.sites.iter().enumerate() {
        let to = bots.nav.nearest(*site).unwrap_or_else(|| panic!("no walkable point at site {i}"));
        // site A is up on the platform: only the stairs (or the ladder) lead there
        assert!(!bots.nav.path(from, to, &none).is_empty(), "no way to site {i} at {site:?}");
    }
}
