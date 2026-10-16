//! What a server is set up with: its rules (practice or a match) and what it needs from the
//! game install.
use super::*;

/// Practice room or a real match (the match settings are in `MatchConfig`).
#[derive(Clone, Debug)]
pub struct Rules {
    /// Practice room: everything below is set for practising.
    pub practice: bool,
    /// Ticks before a dead player comes back.
    pub respawn_ticks: u32,
    /// Reserve ammo is topped up all the time (you still reload).
    pub infinite_ammo: bool,
    /// Practice only: standing dummies on the other team to shoot at. (No bots: by decision,
    /// matches are people only.)
    pub dummies: u8,
}

impl Default for Rules {
    fn default() -> Self {
        Rules { practice: false, respawn_ticks: 3 * TICK_RATE, infinite_ammo: false, dummies: 0 }
    }
}

impl Rules {
    pub fn practice(dummies: u8) -> Rules {
        Rules { practice: true, respawn_ticks: TICK_RATE / 2, infinite_ammo: true, dummies }
    }
}

/// What the server needs from the game install besides the map.
pub struct Assets {
    pub hitboxes: Hitboxes,
    pub materials: Materials,
}

impl Assets {
    /// From a Half-Life install (the directory containing `cstrike/`).
    pub fn load(game_dir: &std::path::Path) -> Result<Assets, String> {
        let mdl = game_dir.join("cstrike/models/player/arctic/arctic.mdl");
        let model = goldsrc::mdl::Model::load(&mdl).map_err(|e| format!("{}: {e}", mdl.display()))?;
        let hitboxes = Hitboxes::from_model(&model, "carbine").ok_or("player model has no aim animations")?;
        // materials.txt is optional: without it every surface counts as concrete.
        let materials = std::fs::read_to_string(game_dir.join("cstrike/sound/materials.txt")).map(|t| Materials::parse(&t)).unwrap_or_default();
        Ok(Assets { hitboxes, materials })
    }
}
