//! Health, armor and death. Damage formulas live with the weapons.
use serde::{Deserialize, Serialize};

pub const MAX_HEALTH: f32 = 100.0;
pub const MAX_ARMOR: f32 = 100.0;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vitals {
    pub health: f32,
    pub armor: f32,
    pub helmet: bool,
}

impl Default for Vitals {
    fn default() -> Self {
        Vitals { health: MAX_HEALTH, armor: 0.0, helmet: false }
    }
}

impl Vitals {
    pub fn alive(&self) -> bool {
        self.health > 0.0
    }

    /// Take `health_damage` and `armor_damage` (already split by the armor rules).
    /// Returns true if this killed the player.
    pub fn hurt(&mut self, health_damage: f32, armor_damage: f32) -> bool {
        if !self.alive() {
            return false;
        }
        self.armor = (self.armor - armor_damage).max(0.0);
        if self.armor == 0.0 {
            self.helmet = false;
        }
        self.health -= health_damage;
        !self.alive()
    }
}

use crate::hitbox::HitGroup;

/// Hit group damage multiplier (`TraceAttack`).
pub fn hitgroup_multiplier(g: HitGroup) -> f32 {
    match g {
        HitGroup::Head => 4.0,
        HitGroup::Stomach => 1.25,
        HitGroup::LeftLeg | HitGroup::RightLeg => 0.75,
        HitGroup::Shield => 0.0,
        _ => 1.0,
    }
}

/// Whether kevlar protects this body part (head also needs the helmet; legs never).
fn armored(v: &Vitals, g: HitGroup) -> bool {
    v.armor > 0.0
        && match g {
            HitGroup::Head => v.helmet,
            HitGroup::LeftLeg | HitGroup::RightLeg | HitGroup::Shield => false,
            _ => true,
        }
}

/// Health lost from one bullet or knife hit (`TakeDamage`), applying the armor rules.
/// `damage` already includes the hit group multiplier. Returns (health damage, armor damage).
/// `armor_factor` is the attacker's weapon (AWP 1.95, AK 1.55, ...). `teammate`: x0.35.
pub fn bullet_damage(v: &Vitals, mut damage: f32, group: HitGroup, armor_factor: f32, teammate: bool) -> (f32, f32) {
    if teammate {
        damage *= 0.35;
    }
    if !armored(v, group) {
        return (damage.trunc(), 0.0);
    }
    let ratio = 0.5 * armor_factor;
    let mut health = ratio * damage;
    let mut armor_loss = (damage - health) * 0.5;
    if armor_loss > v.armor {
        // Not enough armor left: it absorbs twice its value, the rest goes through.
        health = damage - v.armor * 2.0;
        armor_loss = v.armor;
    } else if armor_loss < 0.0 {
        armor_loss = 1.0;
    }
    (health.trunc(), armor_loss)
}

/// Health lost from an explosion (HE, C4): generic hit group, armor absorbs 1x.
pub fn explosion_damage(v: &Vitals, damage: f32) -> (f32, f32) {
    if v.armor <= 0.0 {
        return (damage.trunc(), 0.0);
    }
    let mut health = 0.5 * damage;
    let mut armor_loss = (damage - health) * 0.5;
    if armor_loss > v.armor {
        health = damage - v.armor;
        armor_loss = v.armor;
    }
    (health.trunc(), armor_loss)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn armored_vitals(helmet: bool) -> Vitals {
        Vitals { health: 100.0, armor: 100.0, helmet }
    }

    #[test]
    fn known_cs_damage_numbers() {
        // AK-47 (36 damage, armor factor 1.55) at point blank.
        let body = bullet_damage(&armored_vitals(true), 36.0, HitGroup::Chest, 1.55, false);
        assert_eq!(body.0, 27.0, "AK chest on kevlar");
        let head = bullet_damage(&armored_vitals(true), 36.0 * 4.0, HitGroup::Head, 1.55, false);
        assert_eq!(head.0, 111.0, "AK headshot through a helmet kills");
        let bare = bullet_damage(&Vitals::default(), 36.0 * 4.0, HitGroup::Head, 1.55, false);
        assert_eq!(bare.0, 144.0);
        // Legs are never armored.
        assert_eq!(bullet_damage(&armored_vitals(true), 27.0, HitGroup::LeftLeg, 1.55, false).0, 27.0);
        // Helmetless head hit ignores the vest.
        assert_eq!(bullet_damage(&armored_vitals(false), 144.0, HitGroup::Head, 1.55, false).0, 144.0);
        // AWP body shot on kevlar: 115 * 0.975 = 112 -> kills.
        assert_eq!(bullet_damage(&armored_vitals(false), 115.0, HitGroup::Chest, 1.95, false).0, 112.0);
        // Teammate damage is 35%.
        assert_eq!(bullet_damage(&Vitals::default(), 100.0, HitGroup::Chest, 1.0, true).0, 35.0);
    }

    #[test]
    fn thin_armor_lets_damage_through() {
        let v = Vitals { health: 100.0, armor: 2.0, helmet: false };
        // AK 36: wants armor loss (36-27.9)*0.5 = 4.05 > 2 -> health = 36 - 4 = 32.
        assert_eq!(bullet_damage(&v, 36.0, HitGroup::Chest, 1.55, false), (32.0, 2.0));
    }
}
