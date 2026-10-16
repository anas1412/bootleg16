//! Messages between client and server (postcard-encoded, one per UDP datagram).
use serde::{Deserialize, Serialize};
use sim::doors::DoorNet;
use sim::combat::Vitals;
use sim::hitbox::HitGroup;
use crate::rounds::{EndReason, Phase};
use sim::player::Limits;
use sim::pmove::{Input, Player};
use sim::weapons::{Id, Loadout};

/// Bump when messages change; mismatched clients are turned away.
pub const PROTOCOL_VERSION: u32 = 20;
pub const DEFAULT_PORT: u16 = 27015;
pub const TICK_RATE: u32 = 128;
pub const TICK: f32 = 1.0 / TICK_RATE as f32;
/// Snapshots go out every this many ticks (64 per second at 128 tick).
pub const SNAPSHOT_EVERY: u32 = 2;
/// Inputs repeated in every packet, so a few lost packets lose no keypresses.
pub const INPUT_REDUNDANCY: usize = 8;
/// Dropped after this long without a packet.
pub const TIMEOUT_TICKS: u32 = 5 * TICK_RATE;
pub const MAX_PLAYERS: usize = 32;
/// Minimum padding of an info request (see `ClientMsg::Info`).
pub const INFO_PAD: usize = 96;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ClientMsg {
    /// `cookie`: 0 at first; then the one from `ServerMsg::Challenge` (proves our address is real).
    Hello { version: u32, name: String, cookie: u64 },
    /// Server browser: tell me about yourself (no connection made). `pad` must be at least
    /// `INFO_PAD` bytes so the answer is never bigger than the question (no amplification).
    Info { pad: Vec<u8> },
    /// Inputs numbered `first_seq`, `first_seq + 1`, ...
    /// `events`: the last reliable event number received (the server resends newer ones).
    Inputs { first_seq: u32, inputs: Vec<TimedInput>, events: u32 },
    /// Buy something (checked against money, buy time, buy zone and team).
    Buy { item: BuyItem },
    /// Pick a side; None = auto (the smaller team).
    JoinTeam { team: Option<Team> },
    /// One 20 ms Opus voice frame while a push-to-talk key is held.
    Voice { channel: VoiceChannel, data: Vec<u8> },
    Bye,
}

/// The two voice channels, each on its own push-to-talk key.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceChannel {
    /// Your team, anywhere on the map, full volume.
    Team,
    /// Everyone alive near you, enemies too; quieter with distance.
    Proximity,
}

/// Opus voice: 48 kHz mono, 20 ms frames.
pub const VOICE_RATE: u32 = 48_000;
pub const VOICE_FRAME: usize = 960;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Team {
    Spectator,
    T,
    Ct,
}

/// What the buy menu can buy.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum BuyItem {
    Weapon(Id),
    Kevlar,
    KevlarHelmet,
    DefuseKit,
}

/// One scoreboard row.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Score {
    pub id: u8,
    pub name: String,
    pub kills: i32,
    pub deaths: u32,
    pub alive: bool,
    pub team: Team,
    /// A practice dummy.
    pub bot: bool,
}

/// An input plus the server time the client was showing other players at, so
/// the server can rewind them for hit detection (lag compensation).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct TimedInput {
    pub input: Input,
    pub view_tick: f32,
}

/// Lag compensation never rewinds further than this (CS default `sv_maxunlag` 0.5 s).
pub const MAX_UNLAG_TICKS: u32 = TICK_RATE / 2;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PlayerNet {
    pub id: u8,
    /// Position in 1/8 units (small varints on the wire).
    origin: [i32; 3],
    /// View angles in 1/65536 of a turn (yaw) and of half a turn (pitch).
    yaw: u16,
    pitch: i16,
    /// Bit 0 crouched, bit 1 alive.
    flags: u8,
    /// Weapon in hand.
    pub weapon: Id,
    pub team: Team,
}

impl PlayerNet {
    #[allow(clippy::too_many_arguments)]
    pub fn new(id: u8, origin: [f32; 3], yaw: f32, pitch: f32, crouched: bool, alive: bool, weapon: Id, team: Team) -> PlayerNet {
        PlayerNet {
            id,
            origin: origin.map(|v| (v * 8.0).round() as i32),
            yaw: (yaw.rem_euclid(360.0) / 360.0 * 65536.0).round() as u32 as u16,
            pitch: (pitch.clamp(-90.0, 90.0) / 180.0 * 65535.0).round() as i16,
            flags: crouched as u8 | (alive as u8) << 1,
            weapon,
            team,
        }
    }
    pub fn origin(&self) -> [f32; 3] {
        self.origin.map(|v| v as f32 / 8.0)
    }
    pub fn yaw(&self) -> f32 {
        self.yaw as f32 / 65536.0 * 360.0
    }
    pub fn pitch(&self) -> f32 {
        self.pitch as f32 / 65535.0 * 180.0
    }
    pub fn crouched(&self) -> bool {
        self.flags & 1 != 0
    }
    pub fn alive(&self) -> bool {
        self.flags & 2 != 0
    }
}

/// Things that happened since the last snapshot (effects, kill feed).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum GameEvent {
    /// A bullet (or a shotgun's pellets) from `from` that ended at `to` (wall impact or far
    /// away). `holes`: every wall hit, as (point, surface normal), for bullet holes and dust.
    Shot { shooter: u8, weapon: Id, silenced: bool, from: [f32; 3], to: [f32; 3], holes: Vec<([f32; 3], [f32; 3])> },
    /// `at`: where a bullet or knife hit them (None for explosions and falls).
    Damage { attacker: u8, victim: u8, amount: f32, group: HitGroup, sound: HitSound, at: Option<[f32; 3]> },
    /// Someone's footstep (everyone else hears it; you hear your own predicted ones).
    Footstep { player: u8, at: [f32; 3] },
    GrenadeBounce { kind: sim::grenades::Kind, at: [f32; 3] },
    /// `weapon` None = fell to death.
    Kill { killer: u8, victim: u8, weapon: Option<Id>, headshot: bool },
    /// An HE went off here.
    Explosion { at: [f32; 3] },
    /// A flashbang went off here; `blinded` lists who it caught and how badly.
    Flash { at: [f32; 3], blinded: Vec<(u8, sim::grenades::Blind)> },
    /// Freeze time of round `round` began (everyone is back at their spawn).
    RoundStart { round: u32 },
    RoundEnd { winner: Team, reason: EndReason },
    /// Teams swapped sides (halftime, overtime halves).
    SidesSwapped,
    /// The match is decided (None = draw); it starts over after the final scoreboard.
    MatchOver { winner: Option<Team> },
    BombPlanted { by: u8, at: [f32; 3] },
    /// Someone started defusing (for the sound); `kit`: the fast way.
    Defusing { by: u8, kit: bool },
    BombDefused { by: u8 },
    BombExploded { at: [f32; 3] },
    /// A breakable broke (for its sound and debris; the state itself is `Snapshot::broken`).
    Broke { model: u16, material: sim::breakables::Material, at: [f32; 3] },
}

impl GameEvent {
    /// Events that must arrive (kill feed, round and bomb news). The rest are sounds and effects
    /// that are useless if late, so a lost one is simply lost.
    pub fn reliable(&self) -> bool {
        matches!(
            self,
            GameEvent::Kill { .. }
                | GameEvent::RoundStart { .. }
                | GameEvent::RoundEnd { .. }
                | GameEvent::SidesSwapped
                | GameEvent::MatchOver { .. }
                | GameEvent::BombPlanted { .. }
                | GameEvent::BombDefused { .. }
                | GameEvent::BombExploded { .. }
        )
    }
}

/// Killer id for deaths nobody caused (the bomb).
pub const WORLD: u8 = u8::MAX;

/// The round as the HUD shows it.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct RoundNet {
    pub phase: Phase,
    pub round: u32,
    /// Rounds won: terrorists, counter-terrorists.
    pub score: [u32; 2],
    /// Seconds left: of freeze time, of the round, or until the next round.
    pub left: f32,
}

/// Where the bomb is (who carries it is only told to terrorists).
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum BombNet {
    None,
    Carried(u8),
    Dropped([f32; 3]),
    /// `left`: seconds to the explosion; `defuse`: who is defusing and seconds they still need.
    Planted { at: [f32; 3], left: f32, defuse: Option<(u8, f32)> },
}

/// Things only you need to know about yourself.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Me {
    pub money: i32,
    /// What the round allows you (your prediction applies it too).
    pub limits: Limits,
    /// Buy time and in your buy zone (or practice).
    pub can_buy: bool,
    pub defuse_kit: bool,
}

/// Which impact sound a hit makes.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitSound {
    Flesh,
    Kevlar,
    /// Headshot stopped by a helmet ("ding").
    Helmet,
    Headshot,
    Explosion,
    /// Knife connected: a slash or a stab.
    KnifeSlash,
    KnifeStab,
}

/// A smoke cloud: where, and how many seconds it has left.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct SmokeNet {
    pub at: [f32; 3],
    pub left: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ServerMsg {
    /// `practice`: practice room rules (no rounds, free buys, infinite ammo, dummies).
    /// `map_hash`: the server's map file (`Bsp::hash`); a client with a different file must leave.
    Welcome { id: u8, map: String, map_hash: u64, tick_rate: u32, practice: bool },
    Reject { reason: String },
    /// Echo this in your Hello to join (stops spoofed addresses from taking slots).
    Challenge { cookie: u64 },
    /// Answer to `ClientMsg::Info`.
    Info { version: u32, name: String, map: String, players: u8, max: u8 },
    /// Scoreboard, sent a few times a second.
    Scores(Vec<Score>),
    /// Someone talking. `gain`: how loud (proximity voice fades with distance; 255 = full).
    Voice { from: u8, channel: VoiceChannel, gain: u8, data: Vec<u8> },
    Snapshot {
        tick: u32,
        /// Last of your inputs the server has run; replay the ones after it.
        ack: u32,
        /// Other players you can see (compact: what drawing them needs).
        players: Vec<PlayerNet>,
        /// Your own full movement state and health, for prediction (None while connecting).
        you: Option<(Player, Vitals)>,
        doors: Vec<DoorNet>,
        /// Your own weapons, for prediction and the HUD.
        loadout: Loadout,
        events: Vec<GameEvent>,
        /// Kill feed, round and bomb news, numbered: resent in every snapshot until acknowledged.
        reliable: Vec<(u32, GameEvent)>,
        /// Breakables currently broken (their brush model numbers).
        broken: Vec<u16>,
        /// Grenades in flight or lying around, and smoke clouds.
        grenades: Vec<(sim::grenades::Kind, [f32; 3])>,
        smokes: Vec<SmokeNet>,
        /// Guns lying on the ground.
        ground: Vec<(Id, [f32; 3])>,
        round: RoundNet,
        bomb: BombNet,
        me: Me,
    },
}

pub fn encode<T: Serialize>(m: &T) -> Vec<u8> {
    postcard::to_allocvec(m).expect("messages always serialize")
}

pub fn decode<'a, T: Deserialize<'a>>(b: &'a [u8]) -> Option<T> {
    postcard::from_bytes(b).ok()
}
