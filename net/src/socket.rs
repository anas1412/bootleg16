//! Non-blocking UDP with optional fake lag and packet loss, for testing real-world
//! conditions on one machine. Lag is counted in ticks of whoever owns the socket. Peers with
//! an encrypted channel get everything sent to them sealed (see `crypto`).
use crate::crypto::Channel;
use std::collections::{HashMap, VecDeque};
use std::net::{SocketAddr, UdpSocket};

pub struct Socket {
    udp: UdpSocket,
    /// One-way delay applied to both sends and receives.
    delay_ticks: u32,
    /// Chance (0..1) to drop each packet in each direction.
    loss: f32,
    now: u64,
    outgoing: VecDeque<(u64, SocketAddr, Vec<u8>)>,
    incoming: VecDeque<(u64, SocketAddr, Vec<u8>)>,
    rng: u64,
    /// Encrypted peers: their channel and its id (ticket).
    channels: HashMap<SocketAddr, (u64, Channel)>,
}

impl Socket {
    pub fn bind(addr: SocketAddr) -> std::io::Result<Socket> {
        let udp = UdpSocket::bind(addr)?;
        udp.set_nonblocking(true)?;
        Ok(Socket { udp, delay_ticks: 0, loss: 0.0, now: 0, outgoing: VecDeque::new(), incoming: VecDeque::new(), rng: 0x9e37_79b9_7f4a_7c15, channels: HashMap::new() })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.udp.local_addr().expect("bound socket has an address")
    }

    /// Simulate a connection with `ping_ms` round trip and `loss` packet loss.
    pub fn set_conditions(&mut self, ping_ms: u32, loss: f32) {
        self.delay_ticks = ping_ms * crate::protocol::TICK_RATE / 2000;
        self.loss = loss;
    }

    fn dropped(&mut self) -> bool {
        if self.loss <= 0.0 {
            return false;
        }
        // xorshift: deterministic and dependency-free.
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % 10_000) as f32 / 10_000.0 < self.loss
    }

    /// From now on, everything to and from `peer` is encrypted with this channel.
    pub fn secure(&mut self, peer: SocketAddr, id: u64, channel: Channel) {
        self.channels.insert(peer, (id, channel));
    }

    pub fn is_secure(&self, peer: SocketAddr) -> bool {
        self.channels.contains_key(&peer)
    }

    /// Decrypt a packet from an encrypted peer (None: not genuine, or a replay).
    pub fn open(&mut self, from: SocketAddr, packet: &[u8]) -> Option<Vec<u8>> {
        self.channels.get_mut(&from)?.1.open(packet)
    }

    /// Forget the channels of peers that are gone.
    pub fn keep_channels(&mut self, keep: impl Fn(&SocketAddr) -> bool) {
        self.channels.retain(|a, _| keep(a));
    }

    pub fn send(&mut self, to: SocketAddr, bytes: Vec<u8>) {
        let bytes = match self.channels.get_mut(&to) {
            Some((id, ch)) => ch.seal(*id, &bytes),
            None => bytes,
        };
        if self.dropped() {
            return;
        }
        if self.delay_ticks == 0 {
            let _ = self.udp.send_to(&bytes, to);
        } else {
            self.outgoing.push_back((self.now + self.delay_ticks as u64, to, bytes));
        }
    }

    /// Call once per tick: releases delayed packets.
    pub fn tick(&mut self) {
        self.now += 1;
        while self.outgoing.front().is_some_and(|p| p.0 <= self.now) {
            let (_, to, b) = self.outgoing.pop_front().unwrap();
            let _ = self.udp.send_to(&b, to);
        }
    }

    /// Everything that has arrived (and finished its fake delay).
    pub fn recv_all(&mut self) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut buf = vec![0u8; 65536];
        // At most this many a call: a flood waits in (and overflows) the OS queue instead of
        // our memory. 32 players send a few each per tick; this is far more.
        let mut room = 4096;
        while room > 0 {
            let Ok((n, from)) = self.udp.recv_from(&mut buf) else { break };
            room -= 1;
            if self.dropped() {
                continue;
            }
            self.incoming.push_back((self.now + self.delay_ticks as u64, from, buf[..n].to_vec()));
        }
        let mut out = Vec::new();
        while self.incoming.front().is_some_and(|p| p.0 <= self.now) {
            let (_, from, b) = self.incoming.pop_front().unwrap();
            out.push((from, b));
        }
        out
    }
}
