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
pub const PROTOCOL_VERSION: u32 = 30;
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
    /// `ticket`: your seat in a reserved match (0 = none).
    Hello { version: u32, name: String, cookie: u64, ticket: u64 },
    /// Tell me about yourself (no connection made). `pad` must be at least
    /// `INFO_PAD` bytes so the answer is never bigger than the question (no amplification).
    Info { pad: Vec<u8> },
    /// Inputs numbered `first_seq`, `first_seq + 1`, ...
    /// `events`: the last reliable event number received (the server resends newer ones).
    /// `snap`: the newest snapshot received (the server sends the next ones as changes to it).
    Inputs { first_seq: u32, inputs: Vec<TimedInput>, events: u32, snap: u32 },
    /// Buy something (checked against money, buy time, buy zone and team).
    Buy { item: BuyItem },
    /// Pick a side; None = auto (the smaller team).
    JoinTeam { team: Option<Team> },
    /// After winning the knife round: switch sides (true) or stay.
    SideVote { switch: bool },
    /// Ask your team to surrender (only once a teammate has abandoned the match).
    CallSurrender,
    /// Ask your team to vote this teammate out of the match.
    CallKick { player: u8 },
    /// Your answer to your team's vote (surrender or kick).
    Vote { yes: bool },
    /// Report a player in your match for review.
    Report { player: u8, reason: ReportReason },
    /// One 20 ms Opus voice frame while a push-to-talk key is held.
    Voice { channel: VoiceChannel, data: Vec<u8> },
    Bye,
    /// Which character to wear on each side (terrorists, counter-terrorists; 0..`CLASSES`).
    Looks { t: u8, ct: u8 },
}

/// Characters per side (CS 1.6's four classes each).
pub const CLASSES: u8 = 4;

/// Why a player was reported.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportReason {
    Cheating,
    /// Throwing the game, blocking, team damage on purpose.
    Griefing,
    /// Abusive voice or names.
    Abuse,
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
    /// Assists (CS2: over 40 damage to someone a teammate then killed).
    pub assists: u32,
    /// Round MVPs (the defuser, the planter whose bomb went off, else the winner with most kills).
    pub mvps: u32,
    /// The character they wear as a terrorist and as a counter-terrorist.
    pub looks: [u8; 2],
    /// Kills that were headshots.
    pub headshots: u32,
    /// Health taken off enemies, and rounds played: ADR is `damage / rounds`.
    pub damage: u32,
    pub rounds: u32,
    /// Round trip to the server (milliseconds), measured by the server.
    pub ping: u16,
    /// Their money: only teammates' (None for the other team).
    pub money: Option<i32>,
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
    /// Bit 0 crouched, bit 1 alive, bit 2 in the air (jumping, falling), bit 3 reloading.
    flags: u8,
    /// Weapon in hand.
    pub weapon: Id,
    pub team: Team,
    /// Health and kit: only in snapshots for their teammates (Team IDs).
    pub mate: Option<Mate>,
}

/// A teammate's health and equipment, as their Team ID shows it.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Mate {
    pub health: u8,
    pub armor: u8,
    pub helmet: bool,
    pub defuse_kit: bool,
    /// The gun in hand: rounds in the magazine and in reserve (None: knife, grenade, bomb).
    pub ammo: Option<(u16, u16)>,
}

impl PlayerNet {
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(id: u8, origin: [f32; 3], yaw: f32, pitch: f32, crouched: bool, alive: bool, airborne: bool, reloading: bool, weapon: Id, team: Team, mate: Option<Mate>) -> PlayerNet {
        PlayerNet {
            id,
            origin: origin.map(|v| (v * 8.0).round() as i32),
            yaw: (yaw.rem_euclid(360.0) / 360.0 * 65536.0).round() as u32 as u16,
            pitch: (pitch.clamp(-90.0, 90.0) / 180.0 * 65535.0).round() as i16,
            flags: crouched as u8 | (alive as u8) << 1 | (airborne as u8) << 2 | (reloading as u8) << 3,
            weapon,
            team,
            mate,
        }
    }
    pub fn airborne(&self) -> bool {
        self.flags & 4 != 0
    }
    pub fn reloading(&self) -> bool {
        self.flags & 8 != 0
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
    Footstep { player: u8, at: [f32; 3], step: sim::pmove::Step },
    /// The map fired a target by name (a trigger zone or a button): `ambient_generic` sounds
    /// with that name play (train's jet fly-by, birds, dogs).
    Fired { name: String },
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
    /// The knife round's winners (they now vote to stay or switch).
    KnifeWon { winners: Team },
    /// Sides are settled; the match starts.
    SidesChosen { switched: bool },
    /// This team surrendered (the other wins the match).
    Surrendered { team: Team },
    /// This player's team voted them out of the match.
    VotedOut { player: u8 },
    /// Deathmatch: this player reached the kill limit (everyone's score starts over).
    KillLimit { player: u8 },
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
                | GameEvent::Fired { .. }
                | GameEvent::RoundStart { .. }
                | GameEvent::RoundEnd { .. }
                | GameEvent::SidesSwapped
                | GameEvent::MatchOver { .. }
                | GameEvent::BombPlanted { .. }
                | GameEvent::BombDefused { .. }
                | GameEvent::BombExploded { .. }
                | GameEvent::KnifeWon { .. }
                | GameEvent::SidesChosen { .. }
                | GameEvent::Surrendered { .. }
                | GameEvent::VotedOut { .. }
                | GameEvent::KillLimit { .. }
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
    /// Before the match proper: waiting for players, the knife round, the side vote.
    pub stage: Stage,
    /// A player of the terrorists / counter-terrorists abandoned (they may vote to surrender).
    pub abandoned: [bool; 2],
    /// A surrender vote running.
    pub vote: Option<VoteNet>,
    /// The match is paused: one side has nobody connected.
    pub paused: bool,
}

/// A team's vote: to surrender, or (`kick`) to vote that player out.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct VoteNet {
    pub team: Team,
    pub kick: Option<u8>,
    pub yes: u8,
    pub no: u8,
    /// Yes votes it takes (a majority of the team still connected).
    pub needed: u8,
    pub left: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    Match,
    /// Reserved match waiting for its players.
    Waiting,
    Knife,
    /// The knife round's winners choose; seconds left.
    SideVote { winners: Team, left: f32 },
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
    /// The reserved match is over: back to the menu.
    MatchEnded,
    /// Answer to `ClientMsg::Info`.
    Info { version: u32, name: String, map: String, players: u8, max: u8 },
    /// Scoreboard, sent a few times a second.
    Scores(Vec<Score>),
    /// Every round so far this match: who won (the side they were on then) and how. Sent with
    /// the scoreboard, so late joiners and reconnects get it all.
    History(Vec<(Team, crate::rounds::EndReason)>),
    /// Someone talking. `gain`: how loud (proximity voice fades with distance; 255 = full).
    /// `muffled`: proximity voice from behind a wall (play it dull and quieter).
    Voice { from: u8, channel: VoiceChannel, gain: u8, muffled: bool, data: Vec<u8> },
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
    /// A snapshot as what changed since snapshot `base` (one you confirmed): see `delta`.
    Delta { base: u32, patch: Vec<u8> },
}

pub fn encode<T: Serialize>(m: &T) -> Vec<u8> {
    postcard::to_allocvec(m).expect("messages always serialize")
}

pub fn decode<'a, T: Deserialize<'a>>(b: &'a [u8]) -> Option<T> {
    postcard::from_bytes(b).ok()
}
