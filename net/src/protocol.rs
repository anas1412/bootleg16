//! Messages between client and server (postcard-encoded, one per UDP datagram).
use serde::{Deserialize, Serialize};
use sim::doors::DoorNet;
use sim::pmove::{Input, Player};

/// Bump when messages change; mismatched clients are turned away.
pub const PROTOCOL_VERSION: u32 = 1;
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
    Inputs { first_seq: u32, inputs: Vec<Input> },
    Bye,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PlayerNet {
    pub id: u8,
    pub player: Player,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ServerMsg {
    Welcome { id: u8, map: String, tick_rate: u32 },
    Reject { reason: String },
    Snapshot {
        tick: u32,
        /// Last of your inputs the server has run; replay the ones after it.
        ack: u32,
        players: Vec<PlayerNet>,
        doors: Vec<DoorNet>,
    },
}

pub fn encode<T: Serialize>(m: &T) -> Vec<u8> {
    postcard::to_allocvec(m).expect("messages always serialize")
}

pub fn decode<'a, T: Deserialize<'a>>(b: &'a [u8]) -> Option<T> {
    postcard::from_bytes(b).ok()
}
