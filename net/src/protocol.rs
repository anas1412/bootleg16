//! Messages between client and server (postcard-encoded, one per UDP datagram).
use serde::{Deserialize, Serialize};
use sim::doors::DoorNet;
use sim::combat::Vitals;
use sim::hitbox::HitGroup;
use sim::pmove::{Input, Player};
use sim::weapons::{Id, Loadout};

/// Bump when messages change; mismatched clients are turned away.
pub const PROTOCOL_VERSION: u32 = 7;
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ClientMsg {
    Hello { version: u32, name: String },
    /// Inputs numbered `first_seq`, `first_seq + 1`, ...
    Inputs { first_seq: u32, inputs: Vec<TimedInput> },
    /// Buy something (free while there's no economy).
    Buy { item: BuyItem },
    Bye,
}

/// What the buy menu can buy.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum BuyItem {
    Weapon(Id),
    PrimaryAmmo,
    SecondaryAmmo,
    Kevlar,
    KevlarHelmet,
    DefuseKit,
    NightVision,
}

/// One scoreboard row.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Score {
    pub id: u8,
    pub name: String,
    pub kills: i32,
    pub deaths: u32,
    pub alive: bool,
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
    pub player: Player,
    pub vitals: Vitals,
    /// View angles (degrees), so others can be drawn facing the right way.
    pub yaw: f32,
    pub pitch: f32,
    /// Weapon in hand.
    pub weapon: Id,
}

/// Things that happened since the last snapshot (effects, kill feed).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum GameEvent {
    /// A bullet from `from` that ended at `to` (wall impact or far away).
    Shot { shooter: u8, weapon: Id, from: [f32; 3], to: [f32; 3] },
    Damage { attacker: u8, victim: u8, amount: f32, group: HitGroup },
    /// `weapon` None = fell to death.
    Kill { killer: u8, victim: u8, weapon: Option<Id>, headshot: bool },
    /// An HE went off here.
    Explosion { at: [f32; 3] },
    /// A flashbang went off here; `blinded` lists who it caught and how badly.
    Flash { at: [f32; 3], blinded: Vec<(u8, sim::grenades::Blind)> },
}

/// A smoke cloud: where, and how many seconds it has left.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct SmokeNet {
    pub at: [f32; 3],
    pub left: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ServerMsg {
    Welcome { id: u8, map: String, tick_rate: u32 },
    Reject { reason: String },
    /// Scoreboard, sent a few times a second.
    Scores(Vec<Score>),
    Snapshot {
        tick: u32,
        /// Last of your inputs the server has run; replay the ones after it.
        ack: u32,
        players: Vec<PlayerNet>,
        doors: Vec<DoorNet>,
        /// Your own weapons, for prediction and the HUD.
        loadout: Loadout,
        events: Vec<GameEvent>,
        /// Grenades in flight or lying around, and smoke clouds.
        grenades: Vec<(sim::grenades::Kind, [f32; 3])>,
        smokes: Vec<SmokeNet>,
        /// Guns lying on the ground.
        ground: Vec<(Id, [f32; 3])>,
    },
}

pub fn encode<T: Serialize>(m: &T) -> Vec<u8> {
    postcard::to_allocvec(m).expect("messages always serialize")
}

pub fn decode<'a, T: Deserialize<'a>>(b: &'a [u8]) -> Option<T> {
    postcard::from_bytes(b).ok()
}
