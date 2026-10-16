//! Server protection over localhost UDP: joining needs the server's cookie, a flood from one
//! address is cut off, and junk / oversized packets are ignored.
mod common;
use common::*;
use net::protocol::{decode, encode, ClientMsg, ServerMsg, PROTOCOL_VERSION};
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

fn recv(udp: &UdpSocket) -> Vec<ServerMsg> {
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    while let Ok((n, _)) = udp.recv_from(&mut buf) {
        out.extend(decode::<ServerMsg>(&buf[..n]));
    }
    out
}

#[test]
fn join_needs_the_cookie_and_floods_are_cut_off() {
    let Some(bsp) = dust2() else { return };
    let mut server = net::Server::new(SocketAddr::from(([127, 0, 0, 1], 0)), "de_dust2", &bsp, assets()).unwrap();
    let addr = server.local_addr();
    let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
    udp.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
    let hello = |cookie| encode(&ClientMsg::Hello { version: PROTOCOL_VERSION, name: "x".into(), cookie, ticket: 0 });
    // A Hello without the cookie gets a challenge, no slot.
    udp.send_to(&hello(0), addr).unwrap();
    server.step();
    let msgs = recv(&udp);
    let Some(ServerMsg::Challenge { cookie }) = msgs.first().cloned() else { panic!("{msgs:?}") };
    assert_eq!(server.player_count(), 0);
    // A wrong cookie (a spoofer can't see ours) gets nothing but another challenge.
    udp.send_to(&hello(cookie ^ 1), addr).unwrap();
    server.step();
    assert!(recv(&udp).iter().all(|m| matches!(m, ServerMsg::Challenge { .. })));
    assert_eq!(server.player_count(), 0);
    // Echoing it joins.
    udp.send_to(&hello(cookie), addr).unwrap();
    server.step();
    assert!(recv(&udp).iter().any(|m| matches!(m, ServerMsg::Welcome { .. })));
    assert_eq!(server.player_count(), 1);

    // A stranger flooding info requests: only the first ~20 a second get an answer.
    let flood = UdpSocket::bind("127.0.0.2:0").unwrap();
    flood.set_read_timeout(Some(Duration::from_millis(20))).unwrap();
    let info = encode(&ClientMsg::Info { pad: vec![0; net::protocol::INFO_PAD] });
    for _ in 0..500 {
        flood.send_to(&info, addr).unwrap();
    }
    std::thread::sleep(Duration::from_millis(20));
    server.step();
    let answers = recv(&flood).len();
    assert!((1..=25).contains(&answers), "{answers} answers to 500 requests");
    // Oversized and garbage packets: ignored, no crash.
    flood.send_to(&[0xffu8; 1500], addr).ok();
    flood.send_to(&[1, 2, 3], addr).unwrap();
    server.step();
}
