//! Looking into and steering a running server: for tests and admin tools.
use super::*;

impl Server {
    pub fn player_count(&self) -> usize {
        self.clients.len()
    }

    /// Server-side health of a player.
    pub fn vitals(&self, id: u8) -> Option<&Vitals> {
        self.clients.values().find(|c| c.id == id).map(|c| &c.vitals)
    }

    /// Server-side state of a player (for tests and admin tools).
    pub fn player(&self, id: u8) -> Option<&Player> {
        self.clients.values().find(|c| c.id == id).map(|c| &c.state.movement)
    }

    /// Set a player's health (tests and admin tools; 0 = dead).
    pub fn set_health(&mut self, id: u8, health: f32) {
        if let Some(c) = self.clients.values_mut().find(|c| c.id == id) {
            c.vitals.health = health;
        }
    }

    /// Move a player (tests and admin tools); their client corrects to it.
    pub fn teleport(&mut self, id: u8, origin: [f32; 3]) {
        if let Some(c) = self.clients.values_mut().find(|c| c.id == id) {
            c.state.movement.origin = origin;
            c.state.movement.velocity = [0.0; 3];
        }
    }

    /// Bytes in the last snapshot sent to a client.
    pub fn snapshot_bytes(&self) -> usize {
        self.snapshot_bytes
    }

    pub fn round_phase(&self) -> crate::rounds::Phase {
        self.round.phase
    }

    /// Pop a smoke cloud here (tests, admin).
    pub fn add_smoke(&mut self, at: [f32; 3]) {
        let now = self.tick as f64 * self.rate.tick() as f64;
        self.smokes.push((at, now + sim::grenades::SMOKE_SECONDS));
    }

    /// Money of a player (tests, admin).
    pub fn money(&self, id: u8) -> Option<i32> {
        self.clients.values().find(|c| c.id == id).map(|c| c.money)
    }

    pub fn player_state(&self, id: u8) -> Option<&PlayerState> {
        self.clients.values().find(|c| c.id == id).map(|c| &c.state)
    }

    pub fn tick(&self) -> u32 {
        self.tick
    }
}
