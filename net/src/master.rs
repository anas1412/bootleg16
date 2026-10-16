//! The master server: game servers announce themselves to it every few seconds, and the game
//! asks it for the list. Official servers prove they're official with a secret token; the
//! master marks them so casual matchmaking only uses them. It never relays game traffic.
use crate::protocol::{decode, encode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

pub const MASTER_PORT: u16 = 27010;
/// Game servers announce this often, and drop off the list after missing a few.
pub const HEARTBEAT_SECONDS: u64 = 5;
const FORGET_AFTER: Duration = Duration::from_secs(HEARTBEAT_SECONDS * 3);
/// Padding of a list request: room for ~15 servers in the answer.
pub const LIST_PAD: usize = 1200;
/// List requests per second per IP.
const LIST_RATE: u32 = 4;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MasterMsg {
    /// Game server -> master: "I'm here". `port`: its game port (the heartbeat comes from it).
    Heartbeat { name: String, map: String, players: u8, max: u8, region: String, token: String },
    /// Game -> master: send me the list. The answer is never bigger than this request, so pad
    /// it (`LIST_PAD`): nobody can use the master to flood someone else.
    List { pad: Vec<u8> },
    /// Master -> game.
    Servers(Vec<ServerEntry>),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ServerEntry {
    pub addr: SocketAddr,
    pub name: String,
    pub map: String,
    pub players: u8,
    pub max: u8,
    pub region: String,
    /// Run by us (matchmaking only picks these).
    pub official: bool,
}

pub struct Master {
    udp: UdpSocket,
    /// Tokens that make a server official.
    tokens: Vec<String>,
    servers: HashMap<SocketAddr, (ServerEntry, Instant)>,
    /// List requests per IP in the current second.
    asked: HashMap<std::net::IpAddr, u32>,
    second: Instant,
}

impl Master {
    pub fn bind(addr: SocketAddr, tokens: Vec<String>) -> std::io::Result<Master> {
        let udp = UdpSocket::bind(addr)?;
        udp.set_read_timeout(Some(Duration::from_millis(200)))?;
        Ok(Master { udp, tokens, servers: HashMap::new(), asked: HashMap::new(), second: Instant::now() })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.udp.local_addr().expect("bound")
    }

    /// Handle what has arrived (waits up to 200 ms for something).
    pub fn step(&mut self) {
        let mut buf = [0u8; 2048];
        while let Ok((n, from)) = self.udp.recv_from(&mut buf) {
            match decode::<MasterMsg>(&buf[..n]) {
                Some(MasterMsg::Heartbeat { name, map, players, max, region, token }) => {
                    let official = !token.is_empty() && self.tokens.contains(&token);
                    let clip = |s: String, n: usize| s.chars().filter(|c| !c.is_control()).take(n).collect::<String>();
                    let entry = ServerEntry { addr: from, name: clip(name, 40), map: clip(map, 32), players, max, region: clip(region, 16), official };
                    self.servers.insert(from, (entry, Instant::now()));
                }
                Some(MasterMsg::List { pad }) if pad.len() >= LIST_PAD => {
                    if self.second.elapsed() >= Duration::from_secs(1) {
                        self.asked.clear();
                        self.second = Instant::now();
                    }
                    let asked = self.asked.entry(from.ip()).or_insert(0);
                    *asked += 1;
                    if *asked > LIST_RATE {
                        continue;
                    }
                    self.servers.retain(|_, (_, seen)| seen.elapsed() < FORGET_AFTER);
                    // As many servers as fit in the size of the request (official ones first).
                    // ponytail: one datagram, ~15 servers; page it when there are more.
                    let mut all: Vec<ServerEntry> = self.servers.values().map(|(e, _)| e.clone()).collect();
                    all.sort_by_key(|e| !e.official);
                    let mut list = Vec::new();
                    for e in all {
                        list.push(e);
                        if encode(&MasterMsg::Servers(list.clone())).len() > n {
                            list.pop();
                            break;
                        }
                    }
                    let _ = self.udp.send_to(&encode(&MasterMsg::Servers(list)), from);
                }
                _ => {}
            }
            let _ = self.udp.set_nonblocking(true);
        }
        let _ = self.udp.set_nonblocking(false);
    }

    pub fn run(&mut self) {
        loop {
            self.step();
        }
    }
}
