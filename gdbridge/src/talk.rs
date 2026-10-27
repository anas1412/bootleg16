//! Talking in a match: voice chat (two push-to-talk channels; what you hear is played, panned
//! towards whoever said it near you, duller through a wall) and your assistant's lines, both
//! from `client_core`. Godot calls `voice_frame` once a frame and reads `voices` and
//! `assistant_lines` for the HUD.
use crate::game::side;
use crate::session::Session;
use client_core::assistant::{Known, Prefs};
use client_core::voice::{self, Voice, VoiceSettings};
use godot::prelude::*;
use net::protocol::VoiceChannel;

/// GoldSrc units from a player's origin up to their ears (standing).
const EAR: f32 = 28.0;

#[godot_api(secondary)]
impl Session {
    /// Once a frame in a match: a talk key held (`team`, `near`), and `menu` (a menu has the
    /// keys, so an open mic stays shut). Sends what the mic heard, plays what came in.
    #[func]
    fn voice_frame(&mut self, team: bool, near: bool, menu: bool) {
        let Some(c) = &mut self.client else { return };
        let v = self.voice.get_or_insert_with(|| Voice::new(VoiceSettings::load()));
        if team {
            v.push(VoiceChannel::Team);
        } else if near {
            v.push(VoiceChannel::Proximity);
        } else if v.open_mic() && !menu {
            v.push(VoiceChannel::Team);
        }
        for (channel, data) in v.take_outgoing() {
            c.send_voice(channel, data);
        }
        let heard = c.take_voice();
        if heard.is_empty() {
            return;
        }
        // where your ears are and which way is your right, in the simulation's frame
        let (ear, right) = match c.me_as_remote() {
            Some(m) => {
                let yaw = m.yaw.to_radians();
                ([m.origin[0], m.origin[1], m.origin[2] + EAR], [yaw.sin(), -yaw.cos(), 0.0])
            }
            None => ([0.0; 3], [1.0, 0.0, 0.0]),
        };
        let others = c.others_at(1.0);
        for h in heard {
            let name = c.scores.iter().find(|s| s.id == h.from).map_or(String::new(), |s| s.name.clone());
            let at = others.iter().find(|o| o.id == h.from && o.alive).map(|o| o.origin);
            let pan = voice::pan(h.channel, ear, right, at);
            v.play(h, &name, pan);
        }
    }

    /// Who's talking now, you first while you send: `id` (-1 for you), `you`, `name`, `team`,
    /// `channel` ("team" or "near"), `muffled` (a wall between you), `level` (0 to 1, the
    /// voice's loudness for its waveform).
    #[func]
    fn voices(&self) -> Array<VarDictionary> {
        let (Some(c), Some(v)) = (&self.client, &self.voice) else { return Array::new() };
        v.talking()
            .iter()
            .filter_map(|t| {
                let (id, name, team) = match t.from {
                    None => (-1, "You".to_string(), side(c.team())),
                    Some(id) => {
                        let s = c.scores.iter().find(|s| s.id == id)?;
                        (id as i64, s.name.clone(), side(s.team))
                    }
                };
                let channel = if t.channel == VoiceChannel::Team { "team" } else { "near" };
                Some(vdict! {
                    "id" => id, "you" => t.from.is_none(), "name" => name.as_str(), "team" => team,
                    "channel" => channel, "muffled" => t.muffled, "level" => t.level,
                })
            })
            .collect()
    }

    /// Your assistant's lines since the last call, oldest first: `who` (0 Claudette, 1
    /// ChildGBT, 2 the Twins, 3 Grumpk), `mood` (0 neutral to 8 annoyed), `text`, `second`
    /// (the Twins' brother answering his sister). It only says what your team already knows.
    /// The arguments are your settings: which assistant (anything else: none), your name,
    /// whether it talks during live rounds, how chatty (0 to 2), whether it relays team news.
    #[func]
    fn assistant_lines(&mut self, who: i64, name: GString, in_round: bool, chattiness: i64, relay: bool) -> Array<VarDictionary> {
        let Some(c) = &self.client else { return Array::new() };
        let events = std::mem::take(&mut self.brain_events);
        let prefs = Prefs { who: who.max(0) as usize, name: name.to_string(), in_round, chattiness: chattiness.clamp(0, 2) as u8, relay };
        let t = self.started.elapsed().as_secs_f32();
        self.brain
            .update(&events, &Known::from_client(c), &prefs, t)
            .into_iter()
            .map(|l| vdict! { "who" => l.who as i64, "mood" => l.mood as i64, "text" => l.text.as_str(), "second" => l.second })
            .collect()
    }
}
