//! Sprite (`.spr`, IDSP v2) reader for HUD icons, plus the `hud.txt` table that names
//! regions of those sprites (`d_ak47 640 640hud1 192 80 48 16`).
use std::{collections::HashMap, fs, io, path::Path};

/// The first frame of a sprite as white RGBA whose alpha is the icon's shape, so the
/// HUD can tint it any colour.
pub struct Sprite {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Sprite {
    pub fn load(path: impl AsRef<Path>) -> io::Result<Sprite> {
        Self::parse(&fs::read(path)?).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "not an IDSP v2 sprite"))
    }

    pub fn parse(data: &[u8]) -> Option<Sprite> {
        if data.get(..4)? != b"IDSP" {
            return None;
        }
        let i32_at = |o: usize| Some(i32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?));
        if i32_at(4)? != 2 {
            return None;
        }
        // Header is 40 bytes; texture format: 0 normal, 1 additive, 2 index-alpha, 3 alpha-test.
        let format = i32_at(12)?;
        let colors = u16::from_le_bytes(data.get(40..42)?.try_into().ok()?) as usize;
        let palette = data.get(42..42 + colors * 3)?;
        // First frame: group flag, origin x/y, width, height, then one palette index per pixel.
        let f = 42 + colors * 3;
        let (width, height) = (i32_at(f + 12)? as u32, i32_at(f + 16)? as u32);
        let pixels = data.get(f + 20..f + 20 + (width * height) as usize)?;
        let mut rgba = Vec::with_capacity(pixels.len() * 4);
        for &i in pixels {
            let c = palette.get(i as usize * 3..i as usize * 3 + 3)?;
            let alpha = match format {
                2 => i,
                3 => if i == 255 { 0 } else { 255 },
                // Additive HUD art: brightness is the shape. Lift it so glyphs read as solid
                // the way they do in game.
                _ => {
                    let v = c[0].max(c[1]).max(c[2]) as f32 / 255.0;
                    if v < 0.025 { 0 } else { (255.0 * (v.powf(0.45) * 1.15).min(1.0)) as u8 }
                }
            };
            rgba.extend_from_slice(&[255, 255, 255, alpha]);
        }
        Some(Sprite { width, height, rgba })
    }
}

/// One named region of a HUD sprite.
#[derive(Clone, Debug, PartialEq)]
pub struct HudEntry {
    pub sprite: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// The 640-wide entries of a `hud.txt`-style table, by name.
pub fn hud_table(text: &str) -> HashMap<String, HudEntry> {
    text.lines()
        .filter_map(|l| {
            let p: Vec<&str> = l.split_whitespace().collect();
            let n = |i: usize| p.get(i)?.parse::<u32>().ok();
            if p.len() != 7 || p[1] != "640" {
                return None;
            }
            Some((p[0].to_string(), HudEntry { sprite: p[2].to_string(), x: n(3)?, y: n(4)?, w: n(5)?, h: n(6)? }))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_640_entries_only() {
        let t = hud_table("2\nd_ak47 320 320hud1 1 2 3 4\nd_ak47 640 640hud1 192 80 48 16\n");
        assert_eq!(t.len(), 1);
        assert_eq!(t["d_ak47"], HudEntry { sprite: "640hud1".into(), x: 192, y: 80, w: 48, h: 16 });
    }

    #[test]
    fn additive_sprite_becomes_white_alpha() {
        let mut d = b"IDSP".to_vec();
        for v in [2i32, 0, 1] {
            d.extend(v.to_le_bytes());
        }
        d.extend([0u8; 24]); // radius, max size, frames, beam length, sync type
        d.extend(2u16.to_le_bytes());
        d.extend([0, 0, 0, 255, 160, 0]); // black, amber
        for v in [0i32, 0, 0, 2, 1] {
            d.extend(v.to_le_bytes());
        }
        d.extend([0u8, 1]);
        let s = Sprite::parse(&d).unwrap();
        assert_eq!((s.width, s.height), (2, 1));
        assert_eq!(&s.rgba, &[255, 255, 255, 0, 255, 255, 255, 255]);
    }
}
