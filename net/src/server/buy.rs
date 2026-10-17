//! Buying: prices, who may buy what, and where and when (docs/RULES.md 3 and 4).
//! Ammo isn't sold: guns come with full ammo, and everyone's is topped up each round.
use super::*;
use crate::rounds::Phase;

/// Price of one item; None = can't be bought.
pub fn price(item: BuyItem) -> Option<i32> {
    match item {
        BuyItem::Weapon(id) => Some(data::def(id).price as i32),
        BuyItem::Kevlar => Some(650),
        BuyItem::KevlarHelmet => Some(1000),
        BuyItem::DefuseKit => Some(200),
    }
}

/// The guns on sale (a trimmed CS 1.6 list), and which side may buy each.
pub fn allowed(team: Team, item: BuyItem) -> bool {
    use Id::*;
    match item {
        BuyItem::Weapon(Glock | Usp | Deagle | Mp5 | Ump45 | P90 | Scout | Awp | HeGrenade | Flashbang | Smoke) => true,
        BuyItem::Weapon(Mac10 | Galil | Ak47) => team == Team::T,
        BuyItem::Weapon(Tmp | Famas | M4a1) | BuyItem::DefuseKit => team == Team::Ct,
        BuyItem::Weapon(_) => false,
        BuyItem::Kevlar | BuyItem::KevlarHelmet => true,
    }
}

/// One purchase this round: what it cost and what it changed (to put back on undo).
pub(super) struct Purchase {
    cost: i32,
    undo: Undo,
}

enum Undo {
    /// A gun into its slot, replacing `old` (dropped on the ground) or nothing.
    Gun { slot: data::Slot, id: Id, old: Option<sim::weapons::GunState> },
    /// A grenade (index into `Loadout::grenades`) and how many there were after it.
    Grenade { i: usize, after: u8 },
    /// Armor (and helmet), and what was worn before.
    Armor { armor: f32, helmet: bool },
    Kit,
}

impl Server {
    /// Sell back everything bought this round at what was paid, newest first, while you may
    /// still buy (time, place), as CS2 does. Something already used (a gun fired, a grenade
    /// thrown, armor damaged, a gun dropped) stays and isn't refunded. A gun swapped out comes
    /// back off the floor if it's still lying there.
    pub(super) fn undo_buys(&mut self, from: SocketAddr) {
        let Some(c) = self.clients.get(&from) else { return };
        if !self.can_buy(c) {
            return;
        }
        let c = self.clients.get_mut(&from).unwrap();
        let now = c.last_run as f64 * TICK as f64;
        let id = c.id;
        for p in std::mem::take(&mut c.purchases).into_iter().rev() {
            let l = &mut c.state.loadout;
            let undone = match p.undo {
                Undo::Gun { slot, id: gun, old } => {
                    let held = match slot {
                        data::Slot::Primary => &mut l.primary,
                        _ => &mut l.secondary,
                    };
                    let fresh = sim::weapons::GunState::new(gun);
                    if held.as_ref().is_some_and(|g| g.id == gun && g.clip == fresh.clip && g.reserve == fresh.reserve) {
                        // The swapped-out gun, if it's still on the floor where it was dropped.
                        let back = old.and_then(|o| self.ground.iter().position(|g| g.dropper == id && g.gun.id == o.id).map(|i| self.ground.remove(i).gun));
                        *held = back;
                        true
                    } else {
                        false
                    }
                }
                Undo::Grenade { i, after } if l.grenades[i] >= after => {
                    l.grenades[i] -= 1;
                    true
                }
                Undo::Grenade { .. } => false,
                Undo::Armor { armor, helmet } if c.vitals.armor >= combat::MAX_ARMOR => {
                    (c.vitals.armor, c.vitals.helmet) = (armor, helmet);
                    true
                }
                Undo::Armor { .. } => false,
                Undo::Kit if c.defuse_kit => {
                    c.defuse_kit = false;
                    true
                }
                Undo::Kit => false,
            };
            if undone {
                c.money += p.cost;
            }
        }
        // Holding something that's gone: take out the best that's left.
        let l = &mut c.state.loadout;
        let gone = match l.active {
            data::Slot::Primary => l.primary.is_none(),
            data::Slot::Secondary => l.secondary.is_none(),
            data::Slot::Grenade => l.grenades.iter().all(|n| *n == 0),
            _ => false,
        };
        if gone {
            let best = if l.primary.is_some() { data::Slot::Primary } else if l.secondary.is_some() { data::Slot::Secondary } else { data::Slot::Knife };
            l.switch_to(best, now);
        }
    }

    /// Whether this player may buy now (time, place) — prices aside.
    pub(super) fn can_buy(&self, c: &Remote) -> bool {
        if !c.vitals.alive() || c.team == Team::Spectator {
            return false;
        }
        if self.rules.free_play() || self.round.phase == Phase::Warmup {
            return true; // anywhere, any time
        }
        let side = if c.team == Team::Ct { sim::zones::Side::Ct } else { sim::zones::Side::T };
        self.round.can_buy(self.tick) && self.world.zones.in_buy_zone(side, c.state.movement.origin)
    }

    /// Buy for the player at `from`, if they may and can afford it.
    pub(super) fn buy(&mut self, from: SocketAddr, item: BuyItem) {
        let Some(c) = self.clients.get(&from) else { return };
        if !self.can_buy(c) || !allowed(c.team, item) {
            return;
        }
        let free = self.rules.free_play() || self.round.phase == Phase::Warmup;
        let tick = self.tick;
        let c = self.clients.get_mut(&from).unwrap();
        let now = c.last_run as f64 * TICK as f64;
        // What it costs; None = nothing to buy (already have it).
        let cost = match item {
            BuyItem::Kevlar if c.vitals.armor >= combat::MAX_ARMOR => None,
            // Already wearing full armor: only the helmet is paid for.
            BuyItem::KevlarHelmet if c.vitals.helmet && c.vitals.armor >= combat::MAX_ARMOR => None,
            BuyItem::KevlarHelmet if c.vitals.armor >= combat::MAX_ARMOR => Some(350),
            BuyItem::DefuseKit if c.defuse_kit => None,
            other => price(other),
        };
        let Some(cost) = cost else { return };
        let cost = if free { 0 } else { cost };
        if c.money < cost {
            return;
        }
        let l = &mut c.state.loadout;
        let mut old = None;
        let before = (c.vitals.armor, c.vitals.helmet);
        let bought = match item {
            BuyItem::Weapon(id) => {
                // Buying a gun for a taken slot drops the old one, like CS.
                let taken = match data::def(id).slot {
                    data::Slot::Primary => l.primary.is_some(),
                    data::Slot::Secondary => l.secondary.is_some(),
                    _ => false,
                };
                if taken && sim::weapons::is_gun(id) {
                    l.switch_to(data::def(id).slot, now);
                    if let Some(dropped) = l.drop_active(now) {
                        let o = c.state.movement.origin;
                        old = Some(dropped);
                        self.ground.push(GroundGun { gun: dropped, origin: o, velocity: [0.0, 0.0, -1.0], dropper: c.id, dropper_wait_until: tick + REPICK_TICKS });
                    }
                }
                c.state.loadout.give(id, now)
            }
            BuyItem::Kevlar => {
                c.vitals.armor = combat::MAX_ARMOR;
                true
            }
            BuyItem::KevlarHelmet => {
                c.vitals.armor = combat::MAX_ARMOR;
                c.vitals.helmet = true;
                true
            }
            BuyItem::DefuseKit => {
                c.defuse_kit = true;
                true
            }
        };
        if bought {
            c.money -= cost;
            let l = &c.state.loadout;
            let undo = match item {
                BuyItem::Weapon(id @ (Id::HeGrenade | Id::Flashbang | Id::Smoke)) => {
                    let i = match id {
                        Id::HeGrenade => 0,
                        Id::Flashbang => 1,
                        _ => 2,
                    };
                    Undo::Grenade { i, after: l.grenades[i] }
                }
                BuyItem::Weapon(id) => Undo::Gun { slot: data::def(id).slot, id, old },
                BuyItem::Kevlar | BuyItem::KevlarHelmet => Undo::Armor { armor: before.0, helmet: before.1 },
                BuyItem::DefuseKit => Undo::Kit,
            };
            c.purchases.push(Purchase { cost, undo });
        }
    }
}
