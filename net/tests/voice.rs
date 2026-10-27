//! Voice relay over localhost UDP: team voice reaches teammates anywhere, proximity voice
//! reaches everyone alive nearby (enemies too) and fades with distance.
mod common;
use common::*;
use net::protocol::{Team, VoiceChannel};
use sim::pmove::Input;

fn heard(g: &mut Game) -> Vec<net::client::Heard> {
    for _ in 0..16 {
        g.tick(Input::default(), Input::default());
    }
    g.b.take_voice()
}

#[test]
fn team_voice_everywhere_proximity_only_nearby_and_fading() {
    let bsp = arena_map();
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::Ct)); // enemies, at opposite ends of the map
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    // Team voice never reaches the other team.
    g.a.send_voice(VoiceChannel::Team, vec![1, 2, 3]);
    assert!(heard(&mut g).is_empty());
    // Proximity voice from across the map: out of range.
    g.a.send_voice(VoiceChannel::Proximity, vec![1, 2, 3]);
    assert!(heard(&mut g).is_empty());
    // Close by, the enemy hears it, loud; further away, quieter.
    let o = g.server.player(a).unwrap().origin;
    g.server.teleport(b, [o[0] + 100.0, o[1], o[2]]);
    g.a.send_voice(VoiceChannel::Proximity, vec![9, 9]);
    let near = heard(&mut g);
    assert_eq!(near.len(), 1);
    assert_eq!((near[0].from, near[0].channel, near[0].data.clone(), near[0].muffled), (a, VoiceChannel::Proximity, vec![9, 9], false));
    g.server.teleport(b, [o[0] + 600.0, o[1], o[2]]);
    g.a.send_voice(VoiceChannel::Proximity, vec![9, 9]);
    let far = heard(&mut g);
    assert!(far.len() == 1 && far[0].gain < near[0].gain, "{} vs {}", far.first().map(|f| f.gain).unwrap_or(0), near[0].gain);

    // Teammates hear team voice from anywhere at full volume.
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::T));
    let b = g.b.id().unwrap();
    g.server.teleport(b, [o[0] + 4000.0, o[1], o[2]]);
    g.a.send_voice(VoiceChannel::Team, vec![5]);
    let t = heard(&mut g);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].gain, 255);
}

#[test]
fn proximity_voice_through_a_wall_is_muffled() {
    let bsp = arena_map();
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::Ct));
    let (a, b) = (g.a.id().unwrap(), g.b.id().unwrap());
    let o = g.server.player(a).unwrap().origin;
    // A standing spot within earshot with a wall between the two.
    let world = bsp.world();
    let eye = |p: [f32; 3]| [p[0], p[1], p[2] + 17.0];
    let spot = (1..12)
        .flat_map(|r| (0..16).map(move |k| (r as f32 * 40.0, k as f32 * 22.5)))
        .map(|(r, deg)| [o[0] + r * deg.to_radians().cos(), o[1] + r * deg.to_radians().sin(), o[2]])
        .find(|p| !world.is_solid(sim::collision::Hull::Standing, *p) && world.sight_blocked(eye(o), eye(*p)))
        .expect("a spot behind a wall near the T spawn");
    for _ in 0..8 {
        g.server.teleport(b, spot);
        g.tick(Input::default(), Input::default());
    }
    g.server.teleport(b, spot);
    g.a.send_voice(VoiceChannel::Proximity, vec![7]);
    let h = heard(&mut g);
    assert!(h.len() == 1 && h[0].muffled, "{h:?}");
}
