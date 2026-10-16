//! Voice relay: the server passes Opus frames on, never decoding them. Team voice goes to the
//! speaker's whole team; proximity voice to everyone alive in range, quieter further away.
//! (Only a loudness is sent with proximity voice, not where the speaker is.)
use super::*;

/// Biggest voice frame passed on (a 20 ms Opus voice frame is ~60-120 bytes).
const MAX_VOICE_BYTES: usize = 400;

impl Server {
    pub(super) fn relay_voice(&mut self, from: SocketAddr, channel: VoiceChannel, data: Vec<u8>) {
        let Some(s) = self.clients.get(&from) else { return };
        if data.len() > MAX_VOICE_BYTES || s.team == Team::Spectator {
            return;
        }
        // The dead talk to their team only.
        if channel == VoiceChannel::Proximity && !s.vitals.alive() {
            return;
        }
        let (id, team, mouth) = (s.id, s.team, s.state.movement.eye());
        let range = self.round.cfg.voice_range.max(1.0);
        let to: Vec<(SocketAddr, u8)> = self
            .clients
            .iter()
            .filter(|(a, c)| **a != from && !c.bot)
            .filter_map(|(a, c)| match channel {
                VoiceChannel::Team => (c.team == team).then_some((*a, 255)),
                VoiceChannel::Proximity => {
                    let e = c.state.movement.eye();
                    let d = ((e[0] - mouth[0]).powi(2) + (e[1] - mouth[1]).powi(2) + (e[2] - mouth[2]).powi(2)).sqrt();
                    let gain = 1.0 - d / range;
                    (c.vitals.alive() && gain > 0.0).then_some((*a, (gain * 255.0).round().max(1.0) as u8))
                }
            })
            .collect();
        for (addr, gain) in to {
            let msg = ServerMsg::Voice { from: id, channel, gain, data: data.clone() };
            self.socket.send(addr, encode(&msg));
        }
    }
}
