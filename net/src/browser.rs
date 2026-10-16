//! The game's side of finding servers: ask the master for the list, then ask each server for
//! its details and time the answer (ping). Blocking with short timeouts: run it off the main thread.
use crate::master::{MasterMsg, ServerEntry};
use crate::protocol::{decode, encode, ClientMsg, ServerMsg, PROTOCOL_VERSION};
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

/// A server as the list shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub entry: ServerEntry,
    /// Round trip, None = didn't answer (offline, or blocked).
    pub ping_ms: Option<u32>,
    /// Runs our protocol version (else joining would be refused).
    pub compatible: bool,
}

/// The master's list, each server pinged. Gives up after `wait` per step.
pub fn fetch(master: SocketAddr, wait: Duration) -> std::io::Result<Vec<Listed>> {
    let udp = UdpSocket::bind(if master.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" })?;
    udp.set_read_timeout(Some(Duration::from_millis(50)))?;
    udp.send_to(&encode(&MasterMsg::List), master)?;
    let mut entries = Vec::new();
    let start = Instant::now();
    let mut buf = [0u8; 4096];
    while start.elapsed() < wait && entries.is_empty() {
        if let Ok((n, from)) = udp.recv_from(&mut buf) {
            if from == master {
                if let Some(MasterMsg::Servers(list)) = decode(&buf[..n]) {
                    entries = list;
                    break;
                }
            }
        }
    }
    // Ping them all at once.
    let sent = Instant::now();
    for e in &entries {
        let _ = udp.send_to(&encode(&ClientMsg::Info { pad: vec![0; crate::protocol::INFO_PAD] }), e.addr);
    }
    let mut out: Vec<Listed> = entries.into_iter().map(|entry| Listed { entry, ping_ms: None, compatible: false }).collect();
    while sent.elapsed() < wait && out.iter().any(|l| l.ping_ms.is_none()) {
        let Ok((n, from)) = udp.recv_from(&mut buf) else { continue };
        if let Some(ServerMsg::Info { version, players, .. }) = decode(&buf[..n]) {
            if let Some(l) = out.iter_mut().find(|l| l.entry.addr == from && l.ping_ms.is_none()) {
                l.ping_ms = Some(sent.elapsed().as_millis() as u32);
                l.compatible = version == PROTOCOL_VERSION;
                l.entry.players = players; // fresher than the master's
            }
        }
    }
    out.sort_by_key(|l| (l.ping_ms.is_none(), !l.entry.official, l.ping_ms));
    Ok(out)
}

/// Casual matchmaking: the official server to join, if any. Best ping, preferring one that
/// already has players (so games fill up), never a full one or one on another version.
pub fn find_match(list: &[Listed]) -> Option<&Listed> {
    list.iter()
        .filter(|l| l.entry.official && l.compatible && l.ping_ms.is_some() && l.entry.players < l.entry.max)
        .min_by_key(|l| {
            let ping = l.ping_ms.unwrap_or(999);
            // Up to 40 ms worse is fine to join people already playing.
            let busy_bonus = if l.entry.players > 0 { 40 } else { 0 };
            ping.saturating_sub(busy_bonus)
        })
}
