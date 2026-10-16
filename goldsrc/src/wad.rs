//! WAD3 texture archive reader.
use crate::texture::{cstr, decode_miptex, Texture};
use std::{collections::HashMap, fs, io, path::Path};

const TYP_MIPTEX: u8 = 0x43;

/// Raw WAD file with a name index; textures are decoded on demand.
pub struct Wad {
    data: Vec<u8>,
    /// lowercase name -> (offset, size)
    entries: HashMap<String, (usize, usize)>,
}

impl Wad {
    pub fn load(path: impl AsRef<Path>) -> io::Result<Wad> {
        Self::parse(fs::read(path)?).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "not a WAD3 file"))
    }

    pub fn parse(data: Vec<u8>) -> Option<Wad> {
        if data.get(..4)? != b"WAD3" {
            return None;
        }
        let i32_at = |o: usize| Some(i32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?));
        let (count, dir) = (i32_at(4)? as usize, i32_at(8)? as usize);
        let mut entries = HashMap::new();
        for i in 0..count {
            let e = dir + i * 32;
            let (pos, size) = (i32_at(e)? as usize, i32_at(e + 4)? as usize);
            let (typ, compressed) = (*data.get(e + 12)?, *data.get(e + 13)?);
            if typ == TYP_MIPTEX && compressed == 0 {
                entries.insert(cstr(data.get(e + 16..e + 32)?).to_ascii_lowercase(), (pos, size));
            }
        }
        Some(Wad { data, entries })
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }

    /// Case-insensitive lookup, like the engine.
    pub fn texture(&self, name: &str) -> Option<Texture> {
        let &(pos, size) = self.entries.get(&name.to_ascii_lowercase())?;
        decode_miptex(self.data.get(pos..pos + size)?)
    }

    /// A decal (`decals.wad`): one colour (palette entry 255) everywhere, each pixel's palette
    /// index as its opacity (0 clear .. 255 solid), the way GoldSrc draws decals.
    pub fn decal(&self, name: &str) -> Option<Texture> {
        let &(pos, size) = self.entries.get(&name.to_ascii_lowercase())?;
        crate::texture::decode_decal(self.data.get(pos..pos + size)?)
    }
}
