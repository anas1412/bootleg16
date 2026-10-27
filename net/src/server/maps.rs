//! Changing maps: a booking names the map (the lobby picks it from the pool, or a custom
//! game's veto does), and the server loads it from its game files before the players come.
use super::*;

impl Server {
    /// Where the server finds maps (`maps/<name>.bmap`, ours, or `.bsp` under this folder). Without it,
    /// it stays on the map it started with.
    pub fn set_game_dir(&mut self, dir: std::path::PathBuf) {
        self.maps = crate::config::pool_maps_in(&dir);
        if !self.maps.contains(&self.map) {
            self.maps.push(self.map.clone());
        }
        self.game_dir = Some(dir);
    }

    /// The pool maps this server can run (told to the lobby).
    pub(super) fn maps(&self) -> Vec<String> {
        if self.maps.is_empty() { vec![self.map.clone()] } else { self.maps.clone() }
    }

    /// Load another map: the world, its spawns, doors and breakables; everything lying around
    /// goes. Only between matches (nobody is in).
    pub(super) fn change_map(&mut self, name: &str) -> Result<(), String> {
        if name == self.map {
            return Ok(());
        }
        let dir = self.game_dir.clone().ok_or("no game files to load maps from")?;
        if !self.maps.iter().any(|m| m == name) {
            return Err(format!("{name} isn't on this server"));
        }
        // our own map format first (built in Godot), else a GoldSrc BSP
        let ours = dir.join(format!("maps/{name}.bmap"));
        if ours.exists() {
            let (map, hash) = sim::mapfile::MapFile::load(&ours)?;
            self.world = map.world();
            self.pieces = breakables::Pieces::default();
            self.map_hash = hash;
        } else {
            let bsp = Bsp::load(dir.join(format!("maps/{name}.bsp"))).map_err(|e| format!("{name}: {e}"))?;
            self.world = World::from_bsp(&bsp);
            self.pieces = breakables::Pieces::from_bsp(&bsp);
            self.map_hash = bsp.hash;
        }
        self.spawns_t = self.world.zones.spawns(sim::zones::Side::T);
        self.spawns_ct = self.world.zones.spawns(sim::zones::Side::Ct);
        self.map = name.to_string();
        self.history.clear();
        self.grenades.clear();
        self.smokes.clear();
        self.ground.clear();
        self.bodies.clear();
        self.kits.clear();
        self.bomb = Bomb::None;
        self.last_seen.clear();
        self.sightings = Default::default();
        println!("map: {name}");
        Ok(())
    }
}
