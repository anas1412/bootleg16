//! The buy menu's contents: what each side can buy, by category, in CS's order. Each item has
//! a key (the weapon's CS name, or `kevlar`, `kevlar_helmet`, `defuse_kit`) that Godot hands
//! back to buy it. Prices and who may buy what stay the server's (`net::server::buy`).
use net::protocol::{BuyItem, Team};
use sim::weapons::Id;

/// (category, name shown, item) for everything `team` may buy.
pub fn layout(team: Team) -> Vec<(&'static str, &'static str, BuyItem)> {
    use BuyItem::*;
    use Id::*;
    let ct = team == Team::Ct;
    let pick = |t: (&'static str, Id), c: (&'static str, Id)| if ct { (c.0, Weapon(c.1)) } else { (t.0, Weapon(t.1)) };
    let mut items = vec![
        ("Pistols", ("Glock-18", Weapon(Glock))),
        ("Pistols", ("USP", Weapon(Usp))),
        ("Pistols", ("P228", Weapon(P228))),
        ("Pistols", ("Desert Eagle", Weapon(Deagle))),
        ("SMGs", pick(("MAC-10", Mac10), ("TMP", Tmp))),
        ("SMGs", ("MP5", Weapon(Mp5))),
        ("SMGs", ("UMP45", Weapon(Ump45))),
        ("SMGs", ("P90", Weapon(P90))),
        ("Rifles", pick(("Galil", Galil), ("FAMAS", Famas))),
        ("Rifles", pick(("AK-47", Ak47), ("M4A1", M4a1))),
        ("Rifles", ("Scout", Weapon(Scout))),
        ("Rifles", ("AWP", Weapon(Awp))),
        ("Gear", ("Kevlar", Kevlar)),
        ("Gear", ("Kevlar + Helmet", KevlarHelmet)),
        ("Gear", ("Flashbang", Weapon(Flashbang))),
        ("Gear", ("HE Grenade", Weapon(HeGrenade))),
        ("Gear", ("Smoke Grenade", Weapon(Smoke))),
    ];
    if ct {
        items.push(("Gear", ("Defuse Kit", DefuseKit)));
    }
    items.into_iter().map(|(cat, (name, item))| (cat, name, item)).collect()
}

/// An item's key.
pub fn key(item: BuyItem) -> &'static str {
    match item {
        BuyItem::Weapon(id) => sim::weapons::data::def(id).name,
        BuyItem::Kevlar => "kevlar",
        BuyItem::KevlarHelmet => "kevlar_helmet",
        BuyItem::DefuseKit => "defuse_kit",
    }
}

/// The item a key names (from either side's list).
pub fn parse(k: &str) -> Option<BuyItem> {
    [Team::T, Team::Ct].into_iter().flat_map(layout).map(|(_, _, item)| item).find(|item| key(*item) == k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_on_sale_is_one_the_server_sells_that_side() {
        for team in [Team::T, Team::Ct] {
            for (_, name, item) in layout(team) {
                assert!(net::server::buy::allowed(team, item), "{name} isn't sold to {team:?}");
                assert!(net::server::buy::price(item).is_some_and(|p| p > 0), "{name} has no price");
                assert_eq!(parse(key(item)), Some(item), "{name}'s key");
            }
        }
        assert_eq!(parse("ak47"), Some(BuyItem::Weapon(Id::Ak47)));
        assert_eq!(parse("nothing"), None);
    }
}
