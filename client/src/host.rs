//! Hosting a match on your own PC for friends: the server runs inside the game (as for Solo,
//! `ServerChoice::Host`) but open to everyone on `PORT`. Friends join from your name in their
//! Steam friends list ("Join game": our rich presence carries `+connect <address>`), or by
//! typing the address on the Play page. Outside your home they need the port forwarded on
//! your router. The address is your public IP, asked of a STUN server when hosting starts.
use bevy::prelude::*;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The port a hosted match listens on (UDP).
pub const PORT: u16 = net::protocol::DEFAULT_PORT;

/// We're hosting: the address friends join (once found).
#[derive(Resource)]
pub struct Hosting(Arc<Mutex<Option<String>>>);

impl Hosting {
    /// Start finding our public address in the background.
    pub fn start() -> Hosting {
        let found = Arc::new(Mutex::new(None));
        let f = found.clone();
        std::thread::spawn(move || {
            let ip = public_ip();
            info!("hosting: friends join {ip:?} port {PORT}");
            if let (Some(ip), Ok(mut f)) = (ip, f.lock()) {
                *f = Some(format!("{ip}:{PORT}"));
            }
        });
        Hosting(found)
    }

    pub fn address(&self) -> Option<String> {
        self.0.lock().ok()?.clone()
    }
}

/// Our address as the internet sees it (a STUN binding request, RFC 5389), or None offline.
fn public_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    for server in ["stun.l.google.com:19302", "stun.cloudflare.com:3478"] {
        let id: [u8; 12] = std::array::from_fn(|_| fastrand::u8(..));
        let mut req = vec![0x00, 0x01, 0x00, 0x00, 0x21, 0x12, 0xA4, 0x42];
        req.extend_from_slice(&id);
        if socket.send_to(&req, server).is_err() {
            continue;
        }
        let mut buf = [0u8; 512];
        if let Ok(n) = socket.recv(&mut buf) {
            if let Some(ip) = mapped_address(&buf[..n], &id) {
                return Some(ip);
            }
        }
    }
    None
}

/// The (XOR-)MAPPED-ADDRESS in a STUN binding response to our request `id` (IPv4).
fn mapped_address(msg: &[u8], id: &[u8; 12]) -> Option<IpAddr> {
    if msg.len() < 20 || msg[0..2] != [0x01, 0x01] || msg[8..20] != id[..] {
        return None;
    }
    let mut at = 20;
    while at + 4 <= msg.len() {
        let kind = u16::from_be_bytes([msg[at], msg[at + 1]]);
        let len = u16::from_be_bytes([msg[at + 2], msg[at + 3]]) as usize;
        let v = msg.get(at + 4..at + 4 + len)?;
        // value: 0, family (1 = IPv4), port, address
        if (kind == 0x0020 || kind == 0x0001) && len >= 8 && v[1] == 1 {
            let mut ip = [v[4], v[5], v[6], v[7]];
            if kind == 0x0020 {
                for (b, m) in ip.iter_mut().zip([0x21, 0x12, 0xA4, 0x42]) {
                    *b ^= m;
                }
            }
            return Some(IpAddr::from(ip));
        }
        at += 4 + len.div_ceil(4) * 4;
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_the_xor_mapped_address() {
        let id = [7u8; 12];
        // a binding success with XOR-MAPPED-ADDRESS 203.0.113.7:27015
        let mut msg = vec![0x01, 0x01, 0x00, 0x0C, 0x21, 0x12, 0xA4, 0x42];
        msg.extend_from_slice(&id);
        let port = 27015u16 ^ 0x2112;
        let ip = [203 ^ 0x21, 0 ^ 0x12, 113 ^ 0xA4, 7 ^ 0x42];
        msg.extend_from_slice(&[0x00, 0x20, 0x00, 0x08, 0x00, 0x01]);
        msg.extend_from_slice(&port.to_be_bytes());
        msg.extend_from_slice(&ip);
        assert_eq!(super::mapped_address(&msg, &id), Some("203.0.113.7".parse().unwrap()));
        assert_eq!(super::mapped_address(&msg, &[0u8; 12]), None);
    }
}
