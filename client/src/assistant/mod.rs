//! The AI assistant: a companion that talks to you during matches (only there), in one of
//! four voices. v1 picks hand-written placeholder lines by rules (`lines`, `speaker`);
//! generated lines from the server come later (plan: OpenCode, free models first).
//! Same information for all four assistants, and only what your team already knows.
mod lines;
mod speaker;
mod uplink;
pub mod view;

use crate::effects::GameNews;
use crate::game::{Frame, Walker};
use crate::menu::{AppState, Settings};
use bevy::prelude::*;
use lines::{fill, Lines, Mood};
use net::protocol::{GameEvent, Team};
use speaker::{Kind, Now, Speaker};

/// One of the four AI assistants.
pub struct Assistant {
    pub name: &'static str,
    /// Ring colour of its face.
    pub color: Color,
}

pub const ASSISTANTS: [Assistant; 4] = [
    // Assertive and careful.
    Assistant { name: "Claudette", color: Color::srgb(0.910, 0.529, 0.373) },
    // A child's voice, cold and polite (Red Queen archetype).
    Assistant { name: "ChildGBT", color: Color::srgb(0.435, 0.890, 0.757) },
    // A tsundere sister and the calm brother who balances her.
    Assistant { name: "Twins", color: Color::srgb(0.663, 0.545, 1.0) },
    // Roaster and troll.
    Assistant { name: "Grumpk", color: Color::srgb(0.831, 1.0, 0.227) },
];

/// `Settings::assistant` value meaning "no assistant".
pub const OFF: u8 = ASSISTANTS.len() as u8;

/// Every trigger the lines file must cover (checked by a test).
#[cfg(test)]
const TRIGGERS: [&str; 21] = [
    "greet_first", "greet_back", "hello", "practice_start", "match_start", "buy_low", "buy_rich", "round_won", "round_lost", "you_died", "streak",
    "bomb_planted", "bomb_defused", "last_alive", "halftime", "match_won", "match_lost", "teammate_down",
    "bomb_dropped", "bomb_yours", "enemy_spotted",
];

/// Below this you can't buy a rifle and armor; at or above the other, you're rich.
const LOW_MONEY: i32 = 2000;
const RICH_MONEY: i32 = 5000;

/// The last line said. Twins: `text` holds her line and his after a newline; `mood2` is his.
pub struct Said {
    pub who: usize,
    pub mood: Mood,
    pub mood2: Mood,
    pub text: String,
}

#[derive(Resource)]
pub struct Brain {
    lines: Lines,
    speaker: Speaker,
    pub said: Option<Said>,
    /// Bumped on every new line (the views redraw on change).
    pub version: u32,
    practice_hello: bool,
    streak: u32,
    clutch_said: bool,
    /// Last bomb state and visible enemies seen, to notice changes (same data as the HUD/radar).
    last_bomb: Option<net::protocol::BombNet>,
    seen_enemies: Vec<u8>,
}

pub struct AssistantPlugin;

impl Plugin for AssistantPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Brain {
            lines: Lines::parse(include_str!("../../assets/assistant/lines.txt")),
            speaker: Speaker::default(),
            said: None,
            version: 0,
            practice_hello: false,
            streak: 0,
            clutch_said: false,
            last_bomb: None,
            seen_enemies: Vec::new(),
        })
        .init_resource::<view::Faces>()
        .add_systems(Startup, |mut s: ResMut<Settings>| {
            // Screenshot runs pick the assistant (0-3) without touching the saved settings.
            if let Some(i) = std::env::var("BOOTLEG16_ASSISTANT").ok().and_then(|v| v.parse::<u8>().ok()) {
                s.assistant = i.min(OFF);
            }
        })
        .add_systems(OnEnter(AppState::InGame), new_match)
        .add_systems(Update, in_game.in_set(Frame::Ui).run_if(resource_exists::<Walker>))
        .add_systems(Update, (uplink::draw, uplink::animate, uplink::light, uplink::reveal).chain().after(in_game).run_if(in_state(AppState::InGame)));
        view::plugin(app);
    }
}

impl Brain {
    /// Say a line for `trigger` if the assistant is on and the rules allow it now.
    fn say(&mut self, trigger: &str, kind: Kind, now: Now, settings: &Settings, slots: &[(&str, String)]) {
        let who = settings.assistant as usize;
        if who >= ASSISTANTS.len() || !self.speaker.allow(kind, now) {
            return;
        }
        let Some((mood, his, text)) = self.lines.pick(trigger, who) else { return };
        let mut all = vec![("name", settings.name.clone())];
        all.extend(slots.iter().cloned());
        // Twins: her line, then his on the next line.
        let text = fill(&text, &all).replace(" / ", "\n");
        self.said = Some(Said { who, mood, mood2: his.unwrap_or(mood.brother_to()), text });
        self.version += 1;
    }
}

/// A new match starts clean: no gap left over from the menus, no streak, practice hello due.
fn new_match(mut brain: ResMut<Brain>) {
    brain.speaker = Speaker::default();
    brain.practice_hello = false;
    brain.streak = 0;
    brain.clutch_said = false;
    brain.last_bomb = None;
    brain.seen_enemies.clear();
}

/// Match events into lines.
fn in_game(mut news: MessageReader<GameNews>, w: Res<Walker>, settings: Res<Settings>, time: Res<Time>, mut brain: ResMut<Brain>) {
    let c = &w.client;
    let me = c.id();
    let my_team = c.team();
    let now = || Now {
        t: time.elapsed_secs(),
        live: c.round.phase == net::rounds::Phase::Live,
        alive: c.vitals.alive(),
        in_round: settings.assistant_in_round,
        chattiness: settings.chattiness,
    };
    let name_of = |id: u8| c.scores.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default();
    let team_of = |id: u8| c.scores.iter().find(|s| s.id == id).map(|s| s.team);
    if c.practice && !brain.practice_hello && me.is_some() {
        brain.practice_hello = true;
        brain.say("practice_start", Kind::Personal, now(), &settings, &[]);
    }
    // What the radar and scoreboard show, the assistant knows too.
    use net::protocol::BombNet;
    if !c.practice && my_team != Team::Spectator {
        let bomb = c.bomb;
        if brain.last_bomb != Some(bomb) {
            match (brain.last_bomb, bomb) {
                (Some(BombNet::Carried(_)), BombNet::Dropped(_)) => brain.say("bomb_dropped", Kind::Relay, now(), &settings, &[]),
                (_, BombNet::Carried(id)) if Some(id) == me && !matches!(brain.last_bomb, Some(BombNet::Carried(_))) => brain.say("bomb_yours", Kind::Relay, now(), &settings, &[]),
                _ => {}
            }
            brain.last_bomb = Some(bomb);
        }
        // Enemies the server shows you (only ones in sight): a line when new ones appear.
        let visible: Vec<u8> = c.others().into_iter().filter(|r| r.alive && r.team != my_team && r.team != Team::Spectator).map(|r| r.id).collect();
        let new = visible.iter().filter(|id| !brain.seen_enemies.contains(id)).count();
        if new > 0 && settings.relay {
            let n = visible.len().to_string();
            brain.say("enemy_spotted", Kind::Relay, now(), &settings, &[("count", n)]);
        }
        brain.seen_enemies = visible;
    }
    for GameNews(e) in news.read() {
        match e {
            GameEvent::Shot { shooter, .. } if Some(*shooter) == me => brain.speaker.combat(now().t),
            GameEvent::Damage { attacker, victim, .. } if Some(*attacker) == me || Some(*victim) == me => brain.speaker.combat(now().t),
            GameEvent::RoundStart { round } => {
                brain.speaker.new_round();
                brain.streak = 0;
                brain.clutch_said = false;
                if c.practice || my_team == Team::Spectator {
                    continue;
                }
                if *round == 1 {
                    brain.say("match_start", Kind::Personal, now(), &settings, &[]);
                }
                let money = c.me_info.money;
                let slots = [("money", money.to_string())];
                if money < LOW_MONEY {
                    brain.say("buy_low", Kind::Personal, now(), &settings, &slots);
                } else if money >= RICH_MONEY {
                    brain.say("buy_rich", Kind::Personal, now(), &settings, &slots);
                }
            }
            GameEvent::RoundEnd { winner, .. } if my_team != Team::Spectator => {
                let t = if *winner == my_team { "round_won" } else { "round_lost" };
                brain.say(t, Kind::Personal, now(), &settings, &[]);
            }
            GameEvent::Kill { killer, victim, .. } => {
                if Some(*victim) == me {
                    brain.streak = 0;
                    brain.say("you_died", Kind::Personal, now(), &settings, &[]);
                } else if Some(*killer) == me {
                    brain.streak += 1;
                    if brain.streak == 3 || brain.streak == 5 {
                        let s = brain.streak.to_string();
                        brain.say("streak", Kind::Relay, now(), &settings, &[("streak", s)]);
                    }
                } else if settings.relay && team_of(*victim) == Some(my_team) {
                    brain.say("teammate_down", Kind::Relay, now(), &settings, &[("teammate", name_of(*victim))]);
                }
                // Last one standing on your team, enemies still up: a clutch.
                let alive = |team: Team| c.scores.iter().filter(|s| s.team == team && s.alive && Some(s.id) != me).count();
                let enemy = if my_team == Team::T { Team::Ct } else { Team::T };
                if !c.practice && !brain.clutch_said && c.vitals.alive() && my_team != Team::Spectator && alive(my_team) == 0 && alive(enemy) > 0 {
                    brain.clutch_said = true;
                    brain.say("last_alive", Kind::Critical, now(), &settings, &[]);
                }
            }
            GameEvent::BombPlanted { .. } => brain.say("bomb_planted", Kind::Critical, now(), &settings, &[]),
            GameEvent::BombDefused { .. } => brain.say("bomb_defused", Kind::Critical, now(), &settings, &[]),
            GameEvent::SidesSwapped => brain.say("halftime", Kind::Personal, now(), &settings, &[]),
            GameEvent::MatchOver { winner } if my_team != Team::Spectator => {
                let t = if *winner == Some(my_team) { "match_won" } else { "match_lost" };
                brain.say(t, Kind::Personal, now(), &settings, &[]);
            }
            _ => {}
        }
    }
}
