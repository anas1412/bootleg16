//! How big snapshots are with a full server (10 players), sent 64 times a second.
mod common;
use common::*;
use net::protocol::Team;
use sim::pmove::Input;

#[test]
fn snapshot_size_with_ten_players() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.server.set_rules(net::server::Rules::practice(8));
    // Spectators get everyone (the worst case: all 10 players in every snapshot).
    g.connect_as(Some(Team::Spectator), Some(Team::Spectator));
    assert!(g.a.others().len() >= 8, "sees {}", g.a.others().len());
    let mut sizes = Vec::new();
    for t in 0..256 {
        g.tick(Input { forward: 400.0, yaw: t as f32, ..Default::default() }, Input::default());
        sizes.push(g.server.snapshot_bytes());
    }
    let avg = sizes.iter().sum::<usize>() as f32 / sizes.len() as f32;
    let per_second = avg * 64.0 / 1024.0;
    eprintln!("snapshot ~{avg:.0} bytes -> {per_second:.1} KB/s per player");
    assert!(per_second < 18.0, "{per_second:.1} KB/s per player");
}
