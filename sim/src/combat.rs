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
