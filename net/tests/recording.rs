//! Match recordings: a booked match is recorded (everyone, everything), the file closes at the
//! end, a report points at it, and replaying it shows a watching game the same players at the
//! same places.
mod common;
use common::*;
use net::server::{Group, MatchSetup};
use sim::pmove::Input;
use std::time::Duration;

#[test]
fn a_match_is_recorded_and_plays_back() {
    let Some(bsp) = dust2() else { return };
    let dir = std::env::temp_dir().join(format!("bootleg16-demos-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut g = setup_with(&bsp, 0, 0.0, net::config::MatchConfig { demos: dir.to_string_lossy().into(), name: "rec test".into(), ..Default::default() });
    g.server.reserve(MatchSetup { players: vec![(1, Group::A), (2, Group::B)], knife_round: false, keys: Vec::new(), map: String::new() });
    g.a.ticket = 1;
    g.b.ticket = 2;
    // Two seconds of play, A walking.
    for _ in 0..256 {
        g.tick(Input { forward: 400.0, ..Default::default() }, Input::default());
    }
    let a = g.a.id().unwrap();
    let walked_to = g.server.player(a).unwrap().origin;
    // Everyone leaves: the match frees the server, and the recording closes.
    g.a.disconnect();
    g.b.disconnect();
    for _ in 0..95 * 128 {
        g.server.step();
    }
    let files: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).collect();
    assert_eq!(files.len(), 1, "one recording: {files:?}");
    let name = files[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.contains("rec_test") && name.ends_with("-test.bdem"), "{name}");
    // Read back: A is in it, walking, and was where the server had them.
    let mut r = net::demo::Reader::open(&files[0]).unwrap();
    assert_eq!(r.header.map, "test");
    let mut frames = 0;
    let mut a_moved = false;
    while let Some(f) = r.next_frame() {
        frames += 1;
        if let Some(p) = f.players.iter().find(|p| p.id == a) {
            a_moved |= dist(p.origin(), walked_to) < 8.0;
        }
    }
    assert!(frames > 100, "{frames} frames");
    assert!(a_moved, "A's walk is in the recording");
    // Played back to a watching game: it sees A.
    let mut replay = net::replay::Replay::open("127.0.0.1:0".parse().unwrap(), &files[0]).unwrap();
    let mut viewer = net::Client::connect(replay.local_addr(), "viewer", sim::collision::World::from_bsp(&bsp)).unwrap();
    let mut seen = false;
    for t in 1..400 {
        replay.step(t);
        viewer.tick(Input::default());
        seen |= viewer.others().iter().any(|o| o.id == a);
        std::thread::sleep(Duration::from_micros(300));
    }
    assert!(seen, "the replay showed A to the viewer");
    let _ = std::fs::remove_dir_all(&dir);
}
