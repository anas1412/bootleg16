//! Match recordings ("demos"): what the server saw, every snapshot, nothing culled (every
//! player wherever they were, every event), so a report can be checked and a match rewatched.
//! A file is a header then frames, each length-prefixed postcard, all gzip-compressed. Written
//! as the match runs (a crash keeps what was written), one file per match.
use crate::protocol::{decode, encode, BombNet, GameEvent, PlayerNet, RoundNet, Score, SmokeNet, PROTOCOL_VERSION};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde::{Deserialize, Serialize};
use sim::doors::DoorNet;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

/// The demo format (separate from the network protocol: old demos stay readable as long as
/// this doesn't change).
pub const DEMO_VERSION: u32 = 1;
/// The biggest frame accepted when reading (bigger is a broken file).
const MAX_FRAME: usize = 1 << 20;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Header {
    pub version: u32,
    /// The game protocol it was recorded with (event types and such).
    pub protocol: u32,
    pub map: String,
    pub map_hash: u64,
    pub tick_rate: u32,
    /// Unix time at the start.
    pub started: u64,
    pub server: String,
}

/// One snapshot of everything.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Frame {
    pub tick: u32,
    pub players: Vec<PlayerNet>,
    /// (player id, health) for everyone.
    pub health: Vec<(u8, u8)>,
    pub events: Vec<GameEvent>,
    pub doors: Vec<DoorNet>,
    pub broken: Vec<u16>,
    pub grenades: Vec<(sim::grenades::Kind, [f32; 3])>,
    pub smokes: Vec<SmokeNet>,
    pub ground: Vec<(sim::weapons::Id, [f32; 3])>,
    pub round: RoundNet,
    /// The whole truth (who carries it, too).
    pub bomb: BombNet,
    /// The scoreboard, now and then.
    pub scores: Option<Vec<Score>>,
}

pub struct Recorder {
    out: GzEncoder<BufWriter<std::fs::File>>,
    pub path: PathBuf,
}

fn write_item<T: Serialize>(out: &mut impl Write, item: &T) -> std::io::Result<()> {
    let bytes = encode(item);
    out.write_all(&(bytes.len() as u32).to_le_bytes())?;
    out.write_all(&bytes)
}

fn read_item<T: for<'a> Deserialize<'a>>(input: &mut impl Read) -> Option<T> {
    let mut len = [0u8; 4];
    input.read_exact(&mut len).ok()?;
    let n = u32::from_le_bytes(len) as usize;
    if n > MAX_FRAME {
        return None;
    }
    let mut buf = vec![0u8; n];
    input.read_exact(&mut buf).ok()?;
    decode(&buf)
}

impl Recorder {
    /// Start a recording in `dir`, named after the time, server and map.
    pub fn start(dir: &Path, map: &str, map_hash: u64, server: &str) -> std::io::Result<Recorder> {
        std::fs::create_dir_all(dir)?;
        let started = crate::crypto::now();
        let clean = |s: &str| s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect::<String>();
        let path = dir.join(format!("{started}-{}-{}.bdem", clean(server), clean(map)));
        let file = std::fs::File::create(&path)?;
        let mut out = GzEncoder::new(BufWriter::new(file), Compression::fast());
        let header = Header { version: DEMO_VERSION, protocol: PROTOCOL_VERSION, map: map.into(), map_hash, tick_rate: crate::protocol::TICK_RATE, started, server: server.into() };
        write_item(&mut out, &header)?;
        Ok(Recorder { out, path })
    }

    pub fn frame(&mut self, frame: &Frame) -> std::io::Result<()> {
        write_item(&mut self.out, frame)
    }

    /// Close the file properly (the gzip trailer); after a crash it reads up to where it was.
    pub fn finish(self) -> std::io::Result<PathBuf> {
        self.out.finish()?.flush()?;
        Ok(self.path)
    }

    /// The file's name (reports point at it).
    pub fn name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

/// Reads a recording frame by frame.
pub struct Reader {
    input: GzDecoder<BufReader<std::fs::File>>,
    pub header: Header,
}

impl Reader {
    pub fn open(path: &Path) -> Result<Reader, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut input = GzDecoder::new(BufReader::new(file));
        let header: Header = read_item(&mut input).ok_or("not a bootleg16 recording")?;
        if header.version != DEMO_VERSION {
            return Err(format!("recording format {} (this build reads {DEMO_VERSION})", header.version));
        }
        Ok(Reader { input, header })
    }

    /// The next frame (None at the end, or where a crashed recording stops).
    pub fn next_frame(&mut self) -> Option<Frame> {
        read_item(&mut self.input)
    }
}

/// Delete recordings in `dir` older than `days` (keeps the disk from filling up).
pub fn prune(dir: &Path, days: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let max_age = std::time::Duration::from_secs(days * 24 * 3600);
    for e in entries.flatten() {
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > max_age);
        if old && e.path().extension().is_some_and(|x| x == "bdem") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}
