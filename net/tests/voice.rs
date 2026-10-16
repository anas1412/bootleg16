//! Voice relay over localhost UDP: team voice reaches teammates anywhere, proximity voice
//! reaches everyone alive nearby (enemies too) and fades with distance.
mod common;
use common::*;
use net::protocol::{Team, VoiceChannel};
use sim::pmove::Input;

fn heard(g: &mut Game) -> Vec<(u8, VoiceChannel, u8, Vec<u8>)> {
    for _ in 0..16 {
        g.tick(Input::default(), Input::default());
    }
    g.b.take_voice()
}

#[test]
fn team_voice_everywhere_proximity_only_nearby_and_fading() {
    let Some(bsp) = dust2() else { return };
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
    assert_eq!((near[0].0, near[0].1, near[0].3.clone()), (a, VoiceChannel::Proximity, vec![9, 9]));
    g.server.teleport(b, [o[0] + 600.0, o[1], o[2]]);
    g.a.send_voice(VoiceChannel::Proximity, vec![9, 9]);
    let far = heard(&mut g);
    assert!(far.len() == 1 && far[0].2 < near[0].2, "{} vs {}", far.first().map(|f| f.2).unwrap_or(0), near[0].2);

    // Teammates hear team voice from anywhere at full volume.
    let mut g = setup(&bsp, 0, 0.0);
    g.connect_as(Some(Team::T), Some(Team::T));
    let b = g.b.id().unwrap();
    g.server.teleport(b, [o[0] + 4000.0, o[1], o[2]]);
    g.a.send_voice(VoiceChannel::Team, vec![5]);
    let t = heard(&mut g);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].2, 255);
}
