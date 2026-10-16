//! Multiplayer: an authoritative server and a predicting client over plain UDP.
//!
//! The client sends its inputs (the last few each packet, so a lost packet loses
//! nothing). The server runs every player's movement with the same `sim` code,
//! moves doors, and sends snapshots of everything back. The client shows its own
//! movement immediately (prediction) and corrects itself from each snapshot by
//! rewinding to the server's state and replaying the inputs the server hasn't seen.
pub mod bot;
pub mod client;
pub mod config;
pub mod crypto;
pub mod demo;
pub mod lobby;
pub mod protocol;
pub mod replay;
pub mod rounds;
pub mod secure;
pub mod server;
mod socket;

pub use client::Client;
pub use server::Server;
