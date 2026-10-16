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

impl Server {
    /// Whether this player may buy now (time, place) — prices aside.
    pub(super) fn can_buy(&self, c: &Remote) -> bool {
        if !c.vitals.alive() || c.team == Team::Spectator {
            return false;
        }
        if self.rules.practice || self.round.phase == Phase::Warmup {
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
        let free = self.rules.practice || self.round.phase == Phase::Warmup;
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
                    if let Some(old) = l.drop_active(now) {
                        let o = c.state.movement.origin;
                        self.ground.push(GroundGun { gun: old, origin: o, velocity: [0.0, 0.0, -1.0], dropper: c.id, dropper_wait_until: tick + REPICK_TICKS });
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
        }
    }
}
