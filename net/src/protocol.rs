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
pub const PROTOCOL_VERSION: u32 = 12;
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
    /// Buy something (checked against money, buy time, buy zone and team).
    Buy { item: BuyItem },
    /// Pick a side; None = auto (the smaller team).
    JoinTeam { team: Option<Team> },
    Bye,
}

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
    pub player: Player,
    pub vitals: Vitals,
    /// View angles (degrees), so others can be drawn facing the right way.
    pub yaw: f32,
    pub pitch: f32,
    /// Weapon in hand.
    pub weapon: Id,
    pub team: Team,
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
    Welcome { id: u8, map: String, tick_rate: u32, practice: bool },
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
