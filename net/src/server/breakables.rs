//! Breakable glass, vents and crates on the server: they take bullet and explosion damage,
//! the ones marked "only trigger" (dust2's crates) break when the bomb goes off at their
//! site, and all come back at round start. Clients get the broken list in every snapshot.
use super::*;
use sim::breakables::Breakable;

#[derive(Default)]
pub(super) struct Pieces {
    list: Vec<Breakable>,
    health: Vec<f32>,
    broken: Vec<bool>,
    /// Bomb sites and the target each fires when the bomb explodes there.
    site_targets: Vec<([f32; 3], [f32; 3], String)>,
}

impl Pieces {
    pub(super) fn from_bsp(bsp: &Bsp) -> Pieces {
        let list = sim::breakables::from_bsp(bsp);
        Pieces { health: list.iter().map(|b| b.health).collect(), broken: vec![false; list.len()], list, site_targets: sim::breakables::bomb_site_targets(bsp) }
    }
}

impl Server {
    fn break_piece(&mut self, i: usize) {
        let p = &mut self.pieces;
        if p.broken[i] {
            return;
        }
        p.broken[i] = true;
        let b = &p.list[i];
        self.world.set_broken(b.model, true);
        self.events.push(GameEvent::Broke { model: b.model as u16, material: b.material, at: b.center });
    }

    /// A bullet hit brush model `model` for `damage`.
    pub(super) fn shoot_piece(&mut self, model: usize, damage: f32) {
        let Some(i) = self.pieces.list.iter().position(|b| b.model == model && !b.trigger_only) else { return };
        self.pieces.health[i] -= damage;
        if self.pieces.health[i] <= 0.0 {
            self.break_piece(i);
        }
    }

    /// An explosion at `at`: damage falls off to nothing at `radius` (GoldSrc RadiusDamage).
    pub(super) fn blast_pieces(&mut self, at: [f32; 3], radius: f32, damage: f32) {
        for i in 0..self.pieces.list.len() {
            let b = &self.pieces.list[i];
            if b.trigger_only || self.pieces.broken[i] {
                continue;
            }
            let d = ((b.center[0] - at[0]).powi(2) + (b.center[1] - at[1]).powi(2) + (b.center[2] - at[2]).powi(2)).sqrt();
            let dmg = damage * (1.0 - d / radius);
            if dmg > 0.0 {
                self.pieces.health[i] -= dmg;
                if self.pieces.health[i] <= 0.0 {
                    self.break_piece(i);
                }
            }
        }
    }

    /// The bomb went off: the site it was in fires its target (dust2's crates at that site break).
    pub(super) fn bomb_triggers(&mut self, at: [f32; 3]) {
        let names: Vec<String> = self
            .pieces
            .site_targets
            .iter()
            .filter(|(lo, hi, _)| (0..3).all(|k| at[k] >= lo[k] - 64.0 && at[k] <= hi[k] + 64.0))
            .map(|(_, _, t)| t.clone())
            .collect();
        for i in 0..self.pieces.list.len() {
            if !self.pieces.list[i].targetname.is_empty() && names.contains(&self.pieces.list[i].targetname) {
                self.break_piece(i);
            }
        }
    }

    /// New round: everything is whole again.
    pub(super) fn restore_pieces(&mut self) {
        for i in 0..self.pieces.list.len() {
            if self.pieces.broken[i] {
                self.world.set_broken(self.pieces.list[i].model, false);
            }
            self.pieces.broken[i] = false;
            self.pieces.health[i] = self.pieces.list[i].health;
        }
    }

    pub(super) fn broken_models(&self) -> Vec<u16> {
        self.pieces.list.iter().zip(&self.pieces.broken).filter(|(_, b)| **b).map(|(p, _)| p.model as u16).collect()
    }
}
