//! Players are solid to each other: running at someone stops at their edge.
mod common;
use common::*;
use net::protocol::Team;
use sim::pmove::Input;

fn flat_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}

#[test]
fn a_player_running_at_another_stops_at_their_edge() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::T));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let (pa, pb) = (g.server.player(a).unwrap().origin, g.server.player(b).unwrap().origin);
    let yaw = (pb[1] - pa[1]).atan2(pb[0] - pa[0]).to_degrees();
    let mut closest = f32::MAX;
    for _ in 0..400 {
        g.tick(Input { forward: 400.0, yaw, ..Default::default() }, Input::default());
        let (pa, pb) = (g.server.player(a).unwrap().origin, g.server.player(b).unwrap().origin);
        closest = closest.min(flat_distance(pa, pb));
    }
    // The boxes are 32 wide: centres never get much closer than that.
    assert!(closest > 28.0, "walked into the other player: closest {closest}");
    assert!(closest < 60.0, "never reached them ({closest}): the test proves nothing");
}
