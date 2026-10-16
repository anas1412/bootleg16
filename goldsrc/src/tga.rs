//! TGA images (GoldSrc skyboxes in gfx/env): uncompressed or RLE, 24 or 32 bit, decoded to
//! RGBA8 rows top-down.

pub struct Tga {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub fn decode(d: &[u8]) -> Option<Tga> {
    let (id_len, cmap, kind) = (*d.first()? as usize, *d.get(1)?, *d.get(2)?);
    let w = u16::from_le_bytes([*d.get(12)?, *d.get(13)?]) as usize;
    let h = u16::from_le_bytes([*d.get(14)?, *d.get(15)?]) as usize;
    let (bpp, desc) = (*d.get(16)? as usize / 8, *d.get(17)?);
    if cmap != 0 || !matches!(kind, 2 | 10) || !matches!(bpp, 3 | 4) {
        return None;
    }
    let mut p = 18 + id_len;
    let n = w * h;
    let mut pixels: Vec<[u8; 4]> = Vec::with_capacity(n);
    let px = |p: usize| -> Option<[u8; 4]> {
        let s = d.get(p..p + bpp)?;
        Some([s[2], s[1], s[0], if bpp == 4 { s[3] } else { 255 }]) // stored BGR(A)
    };
    while pixels.len() < n {
        if kind == 2 {
            pixels.push(px(p)?);
            p += bpp;
        } else {
            let head = *d.get(p)?;
            p += 1;
            let count = (head & 0x7f) as usize + 1;
            if head & 0x80 != 0 {
                let c = px(p)?;
                p += bpp;
                pixels.extend(std::iter::repeat_n(c, count));
            } else {
                for _ in 0..count {
                    pixels.push(px(p)?);
                    p += bpp;
                }
            }
        }
    }
    pixels.truncate(n);
    // Bit 5 of the descriptor set = rows stored top-down; otherwise bottom-up (the usual).
    let top_down = desc & 0x20 != 0;
    let mut rgba = Vec::with_capacity(n * 4);
    for y in 0..h {
        let row = if top_down { y } else { h - 1 - y };
        for x in 0..w {
            rgba.extend_from_slice(&pixels[row * w + x]);
        }
    }
    Some(Tga { width: w as u32, height: h as u32, rgba })
}

#[cfg(test)]
mod tests {
    #[test]
    fn decodes_a_bottom_up_24_bit_image() {
        // 2x1, bottom-up: blue then red pixel (BGR order).
        let mut d = vec![0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1, 0, 24, 0];
        d.extend([255, 0, 0, 0, 0, 255]);
        let t = super::decode(&d).unwrap();
        assert_eq!((t.width, t.height), (2, 1));
        assert_eq!(t.rgba, vec![0, 0, 255, 255, 255, 0, 0, 255]);
    }
}
