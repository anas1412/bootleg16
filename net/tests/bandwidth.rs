//! How big snapshots are with a full server (10 players), sent 64 times a second (as changes
//! to the last one the client confirmed: see `net::delta`).
mod common;
use common::*;
use net::protocol::Team;
use sim::pmove::Input;

#[test]
fn snapshot_size_with_ten_players() {
    let bsp = arena_map();
    let mut g = setup(&bsp, 0, 0.0);
    g.server.set_rules(net::server::Rules::practice(8));
    // Spectators get everyone (the worst case: all 10 players in every snapshot).
    g.connect_as(Some(Team::Spectator), Some(Team::Spectator));
    assert!(g.a.others().len() >= 8, "sees {}", g.a.others().len());
    // The dummies run around (moving players are the expensive ones).
    let dummies: Vec<(u8, [f32; 3])> = g.a.others().iter().map(|o| (o.id, o.origin)).collect();
    let mut sizes = Vec::new();
    for t in 0..256 {
        for (i, (id, o)) in dummies.iter().enumerate() {
            let a = (t + i * 30) as f32 * 0.02;
            g.server.teleport(*id, [o[0] + a.cos() * 100.0, o[1] + a.sin() * 100.0, o[2]]);
        }
        g.tick(Input { forward: 400.0, yaw: t as f32, ..Default::default() }, Input::default());
        sizes.push(g.server.snapshot_bytes());
    }
    let avg = sizes.iter().sum::<usize>() as f32 / sizes.len() as f32;
    let per_second = avg * 64.0 / 1024.0;
    eprintln!("snapshot ~{avg:.0} bytes -> {per_second:.1} KB/s per player");
    // In full they're about 265 bytes (16.5 KB/s).
    assert!(per_second < 6.0, "{per_second:.1} KB/s per player");
}
