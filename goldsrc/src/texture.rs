//! GoldSrc miptex decoding (shared by BSP-embedded textures and WAD3 files).

/// A decoded texture: mip level 0 only, as RGBA8.
#[derive(Debug, Clone)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Decode a miptex block: name[16], width, height, 4 mip offsets, pixels, then a
/// u16 palette size and an RGB palette after the 4th mip. Returns None for
/// headers without pixels (external textures) or malformed data.
pub fn decode_miptex(data: &[u8]) -> Option<Texture> {
    let u32_at = |o: usize| Some(u32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?));
    let name = cstr(data.get(..16)?);
    let (w, h) = (u32_at(16)?, u32_at(20)?);
    let mip0 = u32_at(24)? as usize;
    if mip0 == 0 || w == 0 || h == 0 || w > 4096 || h > 4096 {
        return None;
    }
    let (w_, h_) = (w as usize, h as usize);
    // Palette follows mip 3 (w/8 * h/8 pixels) plus a 2-byte count.
    let pal = u32_at(36)? as usize + (w_ / 8) * (h_ / 8) + 2;
    let palette = data.get(pal..pal + 768)?;
    let pixels = data.get(mip0..mip0 + w_ * h_)?;
    // '{' textures are alpha-tested: palette index 255 is transparent.
    let masked = name.starts_with('{');
    let mut rgba = Vec::with_capacity(w_ * h_ * 4);
    for &i in pixels {
        let c = &palette[i as usize * 3..i as usize * 3 + 3];
        if masked && i == 255 {
            rgba.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    Some(Texture { name, width: w, height: h, rgba })
}

/// Decode a decal miptex: palette entry 255 is the colour, the pixel index the opacity.
pub fn decode_decal(data: &[u8]) -> Option<Texture> {
    let u32_at = |o: usize| Some(u32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?));
    let (w, h, mip0) = (u32_at(16)?, u32_at(20)?, u32_at(24)? as usize);
    if mip0 == 0 || w == 0 || h == 0 || w > 4096 || h > 4096 {
        return None;
    }
    let (w_, h_) = (w as usize, h as usize);
    let pal = u32_at(36)? as usize + (w_ / 8) * (h_ / 8) + 2;
    let c = data.get(pal + 255 * 3..pal + 256 * 3)?;
    let rgba = data.get(mip0..mip0 + w_ * h_)?.iter().flat_map(|&i| [c[0], c[1], c[2], i]).collect();
    Some(Texture { name: cstr(data.get(..16)?), width: w, height: h, rgba })
}

pub(crate) fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 8x8 miptex with a 2-colour palette; index 255 is masked for '{' names.
    fn miptex(name: &str) -> Vec<u8> {
        let (w, h) = (8usize, 8usize);
        let mut d = vec![0u8; 40];
        d[..name.len()].copy_from_slice(name.as_bytes());
        d[16..20].copy_from_slice(&(w as u32).to_le_bytes());
        d[20..24].copy_from_slice(&(h as u32).to_le_bytes());
        let mut off = 40;
        for level in 0..4 {
            d[24 + level * 4..28 + level * 4].copy_from_slice(&(off as u32).to_le_bytes());
            off += (w >> level) * (h >> level);
        }
        let mut pixels = vec![1u8; w * h];
        pixels[0] = 255;
        d.extend(pixels);
        d.extend(vec![0u8; off - d.len()]);
        d.extend(256u16.to_le_bytes());
        let mut pal = vec![0u8; 768];
        pal[3..6].copy_from_slice(&[10, 20, 30]);
        pal[765..768].copy_from_slice(&[0, 0, 255]);
        d.extend(pal);
        d
    }

    #[test]
    fn decodes_palette_and_mask() {
        let t = decode_miptex(&miptex("wall")).unwrap();
        assert_eq!((t.width, t.height, t.name.as_str()), (8, 8, "wall"));
        assert_eq!(&t.rgba[4..8], &[10, 20, 30, 255]);
        assert_eq!(&t.rgba[..4], &[0, 0, 255, 255]);
        let masked = decode_miptex(&miptex("{fence")).unwrap();
        assert_eq!(&masked.rgba[..4], &[0, 0, 0, 0]);
    }

    #[test]
    fn decals_take_palette_255_as_colour_and_the_index_as_opacity() {
        let t = decode_decal(&miptex("{shot1")).unwrap();
        assert_eq!(&t.rgba[..4], &[0, 0, 255, 255], "index 255: solid");
        assert_eq!(&t.rgba[4..8], &[0, 0, 255, 1], "index 1: nearly clear");
    }
}
