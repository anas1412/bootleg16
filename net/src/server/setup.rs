//! What a server is set up with: its rules (practice or a match) and what it needs from the
//! game install.
use super::*;

/// Practice room, deathmatch, or a real match (the match settings are in `MatchConfig`).
#[derive(Clone, Debug)]
pub struct Rules {
    /// Practice room: everything below is set for practising.
    pub practice: bool,
    /// Seconds before a dead player comes back.
    pub respawn_secs: f32,
    /// Reserve ammo is topped up all the time (you still reload).
    pub infinite_ammo: bool,
    /// Practice only: standing dummies on the other team to shoot at.
    pub dummies: u8,
    /// Deathmatch: no rounds, timers or bomb; back straight away at a random spawn away from
    /// enemies; buy anything anywhere for free; up to 20 players dropping in any time. When
    /// the time limit ends, the top scorer is announced and everyone's score starts over.
    pub deathmatch: bool,
    /// Deathmatch: seconds a game lasts (CS2: 10 minutes); the top scorer wins, scores start over.
    pub time_limit: f32,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { practice: false, respawn_secs: 3.0, infinite_ammo: false, dummies: 0, deathmatch: false, time_limit: 600.0 }
    }
}

impl Rules {
    pub fn practice(dummies: u8) -> Rules {
        Rules { practice: true, respawn_secs: 0.5, infinite_ammo: true, dummies, ..Rules::default() }
    }

    pub fn deathmatch() -> Rules {
        // CS2 deathmatch: back in 2 s, 10-minute games.
        Rules { deathmatch: true, respawn_secs: 2.0, ..Rules::default() }
    }

    /// No rounds: practice and deathmatch (buy anywhere, come back after dying).
    pub fn free_play(&self) -> bool {
        self.practice || self.deathmatch
    }
}

/// What the server needs from the game install besides the map.
pub struct Assets {
    pub hitboxes: Hitboxes,
    pub materials: Materials,
}

impl Assets {
    /// From the game data folder.
    pub fn load(game_dir: &std::path::Path) -> Result<Assets, String> {
        let mdl = crate::gamefiles::paths(game_dir, "models/player/arctic/arctic.mdl").into_iter().find(|p| p.exists()).ok_or("the game data folder is missing the player model (models/player/arctic/arctic.mdl)")?;
        let model = goldsrc::mdl::Model::load(&mdl).map_err(|e| format!("{}: {e}", mdl.display()))?;
        let hitboxes = Hitboxes::from_model(&model, "carbine").ok_or("player model has no aim animations")?;
        // materials.txt is optional: without it every surface counts as concrete.
        let materials = crate::gamefiles::paths(game_dir, "sound/materials.txt").iter().find_map(|p| std::fs::read_to_string(p).ok()).ok_or(()).map(|t| Materials::parse(&t)).unwrap_or_default();
        Ok(Assets { hitboxes, materials })
    }
}
