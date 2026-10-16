//! Two lobbies on two machines, one active and one standby: the active one streams its state
//! (`persist::snapshot`) every couple of seconds over TCP, sealed with the official token's
//! key (only our lobbies can read or forge it). The standby keeps that state warm, tracks the
//! game servers (they announce to both), and answers no players. If the active one goes
//! quiet, the standby takes over within seconds, with everyone's sessions: players' games
//! switch over by themselves (`LobbyClient` knows both addresses) and carry on encrypted.
//!
//! If both end up active (both started at once, or the link broke while both stayed up), the
//! one active longer wins and the other steps down, taking the winner's state.
use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};

/// The active lobby sends its state this often; the standby takes over after this much silence.
const SEND_EVERY: Duration = Duration::from_secs(2);
const TAKE_OVER_AFTER: Duration = Duration::from_secs(6);
/// The biggest state frame accepted (a lot of players; bigger is junk).
const MAX_FRAME: usize = 64 << 20;

#[derive(Serialize, Deserialize)]
struct Frame {
    /// When the sender became active (unix seconds), and a tie-break.
    active_since: u64,
    tie: [u8; 32],
    snapshot: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Active,
    Standby,
}

pub struct Peer {
    out: Sender<Vec<u8>>,
    frames: Receiver<Vec<u8>>,
    heard: Instant,
    sent: Instant,
    since: u64,
}

impl Peer {
    /// Listen for the other lobby on `port`, and keep connecting to it at `peer`.
    pub fn start(port: u16, peer: SocketAddr) -> std::io::Result<Peer> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        let (frames_tx, frames) = channel();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let tx = frames_tx.clone();
                std::thread::spawn(move || read_frames(stream, tx));
            }
        });
        let (out, out_rx) = channel::<Vec<u8>>();
        std::thread::spawn(move || write_frames(peer, out_rx));
        Ok(Peer { out, frames, heard: Instant::now(), sent: Instant::now() - SEND_EVERY, since: 0 })
    }
}

fn read_frames(mut stream: TcpStream, tx: Sender<Vec<u8>>) {
    let mut len = [0u8; 4];
    while stream.read_exact(&mut len).is_ok() {
        let n = u32::from_le_bytes(len) as usize;
        if n > MAX_FRAME {
            return;
        }
        let mut frame = vec![0u8; n];
        if stream.read_exact(&mut frame).is_err() || tx.send(frame).is_err() {
            return;
        }
    }
}

/// Keep a connection to the peer and send what comes (the newest frame only: old state is
/// useless); reconnect when it drops.
fn write_frames(peer: SocketAddr, rx: Receiver<Vec<u8>>) {
    let mut stream: Option<TcpStream> = None;
    while let Ok(mut frame) = rx.recv() {
        while let Ok(newer) = rx.try_recv() {
            frame = newer;
        }
        if stream.is_none() {
            stream = TcpStream::connect_timeout(&peer, Duration::from_secs(1)).ok();
        }
        if let Some(s) = &mut stream {
            let ok = s.write_all(&(frame.len() as u32).to_le_bytes()).and_then(|_| s.write_all(&frame));
            if ok.is_err() {
                stream = None;
            }
        }
    }
}

impl LobbyServer {
    /// Pair with another lobby (both run this, pointing at each other). Starts as standby:
    /// if nobody is active it takes over after a few seconds.
    pub fn pair(&mut self, port: u16, peer: SocketAddr) -> std::io::Result<()> {
        if self.keys.is_empty() {
            return Err(std::io::Error::other("pairing needs the official token (it seals the link)"));
        }
        self.peer = Some(Peer::start(port, peer)?);
        self.role = Role::Standby;
        Ok(())
    }

    /// Once a step: send our state (active) or take the other's (standby), and decide roles.
    pub(super) fn peer_step(&mut self) {
        let Some(peer) = &mut self.peer else { return };
        let mut newest = None;
        while let Ok(f) = peer.frames.try_recv() {
            newest = Some(f);
        }
        if let Some(sealed) = newest {
            let frame = self.keys[0].open(&sealed).and_then(|(_, body)| decode::<Frame>(&body));
            if let Some(f) = frame {
                let peer = self.peer.as_mut().unwrap();
                peer.heard = Instant::now();
                // Both active: the one active longer stays (a tie: the bigger tie-break).
                let mine = (peer.since, self.key.public());
                let yield_to_them = self.role == Role::Standby || (f.active_since, std::cmp::Reverse(f.tie)) < (mine.0, std::cmp::Reverse(mine.1));
                if yield_to_them {
                    if self.role == Role::Active {
                        println!("the other lobby is active: standing by");
                    }
                    self.role = Role::Standby;
                    self.restore(&f.snapshot);
                }
            }
        }
        let peer = self.peer.as_mut().unwrap();
        match self.role {
            Role::Standby if peer.heard.elapsed() > TAKE_OVER_AFTER => {
                println!("the other lobby went quiet: taking over");
                self.role = Role::Active;
                peer.since = crate::crypto::now();
                // Everyone restored is fresh: give them time to find us.
                for p in self.players.values_mut() {
                    p.seen = Instant::now();
                }
            }
            Role::Active if peer.sent.elapsed() > SEND_EVERY => {
                peer.sent = Instant::now();
                let frame = Frame { active_since: peer.since, tie: self.key.public(), snapshot: self.snapshot() };
                let sealed = self.keys[0].seal(&encode(&frame));
                let _ = self.peer.as_ref().unwrap().out.send(sealed);
            }
            _ => {}
        }
    }
}
