//! Buying: prices, who may buy what, and where and when (docs/RULES.md 3 and 4).
use super::*;
use crate::rounds::Phase;

/// Price of one item; None = can't be bought.
pub fn price(item: BuyItem) -> Option<i32> {
    match item {
        BuyItem::Weapon(id) => Some(data::def(id).price as i32),
        BuyItem::Kevlar => Some(650),
        BuyItem::KevlarHelmet => Some(1000),
        BuyItem::DefuseKit => Some(200),
        BuyItem::NightVision => Some(1250),
        BuyItem::PrimaryAmmo | BuyItem::SecondaryAmmo => None, // depends on the gun: `clip_price`
    }
}

/// What one clip of a gun's ammo costs (`AmmoCostType`).
fn clip_price(id: Id) -> i32 {
    use Id::*;
    match id {
        Glock | Mp5 | Tmp | Elite => 20,           // 9mm
        Usp | Mac10 | Ump45 => 25,                 // .45 ACP
        P228 => 50,                                // .357 SIG
        FiveSeven | P90 => 50,                     // 5.7mm
        Deagle => 40,                              // .50 AE
        Famas | Galil | M4a1 | Sg552 | Aug | Sg550 | M249 => 60, // 5.56mm
        Ak47 | Scout | G3sg1 => 80,                // 7.62mm
        Awp => 125,                                // .338 Magnum
        M3 | Xm1014 => 65,                         // buckshot
        _ => 0,
    }
}

/// Side-only items in the CS 1.6 buy menus.
pub fn allowed(team: Team, item: BuyItem) -> bool {
    use Id::*;
    match item {
        BuyItem::Weapon(Elite | Mac10 | Galil | Ak47 | Sg552 | G3sg1) => team == Team::T,
        BuyItem::Weapon(FiveSeven | Tmp | Famas | M4a1 | Aug | Sg550) | BuyItem::DefuseKit => team == Team::Ct,
        BuyItem::Weapon(Knife | C4) => false,
        _ => true,
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
            BuyItem::PrimaryAmmo | BuyItem::SecondaryAmmo => {
                let primary = item == BuyItem::PrimaryAmmo;
                let g = if primary { c.state.loadout.primary } else { c.state.loadout.secondary };
                g.and_then(|g| {
                    let full = data::gun(g.id)?.reserve;
                    let clip = data::gun(g.id)?.clip.max(1);
                    (g.reserve < full).then(|| (full - g.reserve).div_ceil(clip) as i32 * clip_price(g.id))
                })
            }
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
            BuyItem::PrimaryAmmo => {
                l.refill_ammo(true);
                true
            }
            BuyItem::SecondaryAmmo => {
                l.refill_ammo(false);
                true
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
            BuyItem::NightVision => false, // ponytail: no night vision effect yet, so not sold
        };
        if bought {
            c.money -= cost;
        }
    }
}
