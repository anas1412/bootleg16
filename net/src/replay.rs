//! Watching a recording (see `demo`) with the normal game: this plays the part of a server
//! (on this machine), and the game connects to it as a spectator and sees the match as it was:
//! everyone, everywhere, every event. `server --play <file>` runs it.
use crate::demo::{Frame, Reader};
use crate::protocol::{decode, encode, ClientMsg, GameEvent, ServerMsg};
use crate::socket::Socket;
use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

/// Reliable events (kill feed, round news) kept and resent, like the real server's outbox.
const OUTBOX: usize = 64;
/// The id viewers get (no player has it).
const VIEWER_ID: u8 = 255;

pub struct Replay {
    socket: Socket,
    reader: Reader,
    viewers: HashMap<SocketAddr, Instant>,
    /// Reliable events so far, numbered.
    outbox: VecDeque<(u32, GameEvent)>,
    numbered: u32,
    /// Frames per real snapshot (2: double speed).
    pub speed: u32,
    pub done: bool,
}

impl Replay {
    pub fn open(bind: SocketAddr, path: &std::path::Path) -> Result<Replay, String> {
        let reader = Reader::open(path)?;
        let socket = Socket::bind(bind).map_err(|e| e.to_string())?;
        Ok(Replay { socket, reader, viewers: HashMap::new(), outbox: VecDeque::new(), numbered: 0, speed: 1, done: false })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.socket.local_addr()
    }

    pub fn header(&self) -> &crate::demo::Header {
        &self.reader.header
    }

    /// Jump ahead to `tick` (the moment a report points at, say).
    pub fn skip_to(&mut self, tick: u32) {
        while let Some(f) = self.reader.next_frame() {
            self.keep_reliable(&f);
            if f.tick >= tick {
                break;
            }
        }
    }

    fn keep_reliable(&mut self, f: &Frame) {
        for e in f.events.iter().filter(|e| e.reliable()) {
            self.numbered += 1;
            self.outbox.push_back((self.numbered, e.clone()));
            while self.outbox.len() > OUTBOX {
                self.outbox.pop_front();
            }
        }
    }

    /// The rate the recording was made at (it replays at that speed).
    fn rate(&self) -> crate::tick::TickRate {
        crate::tick::TickRate::new(self.reader.header.tick_rate).unwrap_or_default()
    }

    /// One server tick: welcome viewers; every snapshot interval, the next frame(s) to all.
    pub fn step(&mut self, tick: u32) {
        self.socket.tick();
        for (from, bytes) in self.socket.recv_all() {
            match decode::<ClientMsg>(&bytes) {
                Some(ClientMsg::Hello { .. }) => {
                    self.viewers.insert(from, Instant::now());
                    let h = &self.reader.header;
                    let welcome = ServerMsg::Welcome { id: VIEWER_ID, map: h.map.clone(), map_hash: h.map_hash, tick_rate: h.tick_rate, practice: false };
                    self.socket.send(from, encode(&welcome));
                }
                Some(ClientMsg::Bye) => _ = self.viewers.remove(&from),
                Some(_) => _ = self.viewers.get_mut(&from).map(|t| *t = Instant::now()),
                None => {}
            }
        }
        self.viewers.retain(|_, t| t.elapsed() < Duration::from_secs(10));
        if tick % self.rate().snapshot_every() != 0 || self.done {
            return;
        }
        let mut frame = None;
        for _ in 0..self.speed.max(1) {
            match self.reader.next_frame() {
                Some(f) => {
                    self.keep_reliable(&f);
                    frame = Some(f);
                }
                None => {
                    self.done = true;
                    break;
                }
            }
        }
        let Some(f) = frame else { return };
        if let Some(scores) = &f.scores {
            let msg = encode(&ServerMsg::Scores(scores.clone()));
            for a in self.viewers.keys().copied().collect::<Vec<_>>() {
                self.socket.send(a, msg.clone());
            }
        }
        let msg = encode(&ServerMsg::Snapshot {
            tick: f.tick,
            ack: 0,
            players: f.players,
            you: None,
            doors: f.doors,
            loadout: sim::weapons::Loadout::knife_only(),
            events: f.events.into_iter().filter(|e| !e.reliable()).collect(),
            reliable: self.outbox.iter().cloned().collect(),
            broken: f.broken,
            grenades: f.grenades,
            smokes: f.smokes,
            ground: f.ground,
            kits: Vec::new(),
            round: f.round,
            bomb: f.bomb,
            me: crate::protocol::Me { money: 0, limits: Default::default(), can_buy: false, buy_left: None, defuse_kit: false, protected: false, hurt_from: Vec::new() },
        });
        for a in self.viewers.keys().copied().collect::<Vec<_>>() {
            self.socket.send(a, msg.clone());
        }
    }

    /// Play to the end at the tick rate.
    pub fn run(&mut self) {
        let tick_len = Duration::from_secs_f64(self.rate().tick() as f64);
        let mut next = Instant::now();
        let mut tick = 0u32;
        while !self.done {
            tick += 1;
            self.step(tick);
            next += tick_len;
            if let Some(wait) = next.checked_duration_since(Instant::now()) {
                std::thread::sleep(wait);
            }
        }
        // Let viewers see the last moment for a bit.
        std::thread::sleep(Duration::from_secs(3));
    }
}
