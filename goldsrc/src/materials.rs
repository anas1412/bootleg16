//! `sound/materials.txt`: which material (metal, wood, ...) each texture is.
//! Used for footstep sounds and bullet penetration.
use std::collections::HashMap;

/// Only this many characters of a texture name are compared (GoldSrc's limit).
const NAME_LEN: usize = 12;

#[derive(Default)]
pub struct Materials {
    by_name: HashMap<String, char>,
}

impl Materials {
    pub fn parse(text: &str) -> Materials {
        let mut by_name = HashMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with("//") {
                continue;
            }
            let mut it = line.split_whitespace();
            if let (Some(kind), Some(name)) = (it.next(), it.next()) {
                if let Some(c) = kind.chars().next().filter(|_| kind.len() == 1) {
                    by_name.insert(short(name), c.to_ascii_uppercase());
                }
            }
        }
        Materials { by_name }
    }

    /// Material letter for a texture: M metal, V vent, D dirt, S slosh, T tile,
    /// G grate, W wood, P computer, Y glass, N snow, X grass, C concrete (default).
    pub fn kind(&self, texture: &str) -> char {
        // Animated/random-tiling prefixes (+0, -1) and render prefixes ({ ! ~) are ignored.
        let mut t = texture;
        if t.starts_with(['-', '+']) && t.len() > 2 {
            t = &t[2..];
        }
        if t.starts_with(['{', '!', '~', ' ']) {
            t = &t[1..];
        }
        self.by_name.get(&short(t)).copied().unwrap_or('C')
    }
}

fn short(name: &str) -> String {
    name.chars().take(NAME_LEN).collect::<String>().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_and_truncation() {
        let m = Materials::parse("// comment\nM metal_door\nW {woodfence\nG grate_panel_big\n");
        assert_eq!(m.kind("METAL_DOOR"), 'M');
        assert_eq!(m.kind("+0metal_door"), 'M');
        assert_eq!(m.kind("grate_panel_small"), 'G'); // only the first 12 chars are compared
        assert_eq!(m.kind("metal_door_extra"), 'C'); // ...so a longer name is a different texture
        assert_eq!(m.kind("{woodfence"), 'C'); // listed with '{', looked up without it
        assert_eq!(m.kind("brick"), 'C');
    }
}
