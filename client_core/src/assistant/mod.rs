//! The AI assistant's brain: a companion that talks to you during matches, in one of four
//! voices. v1 picks hand-written placeholder lines by rules (`lines`, `speaker`); generated
//! lines from the server come later (plan: OpenCode, free models first). Same information for
//! all four assistants, and only what your team already knows (`Known`).
//!
//! Each frame the front end hands `Brain::update` the game's events and what you know, and
//! shows the lines it gives back.
mod lines;
mod speaker;

pub use lines::Mood;
use lines::{fill, Lines};
use net::protocol::{BombNet, GameEvent, Team};
use net::rounds::Phase;
use speaker::{Kind, Now, Speaker};

/// The four assistants, by `Prefs::who`.
pub const NAMES: [&str; 4] = [
    // Assertive and careful.
    "Claudette",
    // A child's voice, cold and polite (Red Queen archetype).
    "ChildGBT",
    // A tsundere sister and the calm brother who balances her.
    "Twins",
    // Roaster and troll.
    "Grumpk",
];

/// Every trigger the lines file must cover (checked by a test).
#[cfg(test)]
const TRIGGERS: [&str; 27] = [
    "greet_first", "greet_back", "hello", "practice_start", "match_start", "buy_low", "buy_rich", "round_won", "round_lost", "you_died", "streak",
    "bomb_planted", "bomb_defused", "last_alive", "halftime", "match_won", "match_lost", "teammate_down",
    "bomb_dropped", "bomb_yours", "enemy_spotted", "mvp", "flashed", "team_out", "clutch_won", "round_live",
    "body_grabbed",
];

/// Below this you can't buy a rifle and armor; at or above the other, you're rich.
const LOW_MONEY: i32 = 2000;
const RICH_MONEY: i32 = 5000;

/// A line to show.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// Index into `NAMES`.
    pub who: usize,
    pub mood: Mood,
    pub text: String,
    /// The Twins' brother answering the line before it (his sister's).
    pub second: bool,
}

/// The player's assistant settings.
#[derive(Clone, Debug)]
pub struct Prefs {
    /// Index into `NAMES` (anything else: no assistant).
    pub who: usize,
    /// Your name, for `{name}` in lines.
    pub name: String,
    /// Talk during live rounds at all.
    pub in_round: bool,
    /// 0 quiet, 1 normal, 2 chatty.
    pub chattiness: u8,
    /// Relay team news (enemies spotted, teammates down).
    pub relay: bool,
}

/// A player on the scoreboard.
#[derive(Clone, Debug)]
pub struct Player {
    pub id: u8,
    pub name: String,
    pub team: Team,
    pub alive: bool,
    pub mvps: u32,
}

/// What you and your team know right now: what the HUD, the radar and the scoreboard show.
#[derive(Clone, Debug)]
pub struct Known {
    pub me: Option<u8>,
    pub team: Team,
    pub practice: bool,
    pub phase: Phase,
    pub alive: bool,
    /// Carrying a body.
    pub carrying: bool,
    pub bomb: BombNet,
    pub money: i32,
    /// Enemies alive and in sight (the server only sends those).
    pub enemies_seen: Vec<u8>,
    pub players: Vec<Player>,
}

impl Known {
    pub fn from_client(c: &net::Client) -> Known {
        let team = c.team();
        Known {
            me: c.id(),
            team,
            practice: c.practice,
            phase: c.round.phase,
            alive: c.vitals.alive(),
            carrying: c.state.movement.carrying,
            bomb: c.bomb,
            money: c.me_info.money,
            enemies_seen: c.others().into_iter().filter(|r| r.alive && r.team != team && r.team != Team::Spectator).map(|r| r.id).collect(),
            players: c.scores.iter().map(|s| Player { id: s.id, name: s.name.clone(), team: s.team, alive: s.alive, mvps: s.mvps }).collect(),
        }
    }
}

pub struct Brain {
    lines: Lines,
    speaker: Speaker,
    practice_hello: bool,
    streak: u32,
    clutch_said: bool,
    /// Said this round that the whole team is down.
    team_out_said: bool,
    /// The round's phase last time we looked (freeze time ending: "round_live").
    last_phase: Option<Phase>,
    /// Carrying a body last time we looked ("body_grabbed" when you pick one up).
    carrying: bool,
    /// Round MVPs on the scoreboard last time we looked (None until the first scoreboard).
    mvps: Option<u32>,
    /// Last bomb state and visible enemies seen, to notice changes (same data as the HUD/radar).
    last_bomb: Option<BombNet>,
    seen_enemies: Vec<u8>,
}

impl Default for Brain {
    fn default() -> Self {
        Brain::new()
    }
}

impl Brain {
    pub fn new() -> Brain {
        Brain {
            lines: Lines::parse(include_str!("lines.txt")),
            speaker: Speaker::default(),
            practice_hello: false,
            streak: 0,
            clutch_said: false,
            team_out_said: false,
            last_phase: None,
            carrying: false,
            mvps: None,
            last_bomb: None,
            seen_enemies: Vec::new(),
        }
    }

    /// A new match starts clean: no gap left over from the menus, no streak, practice hello due.
    pub fn new_match(&mut self) {
        self.speaker = Speaker::default();
        self.practice_hello = false;
        self.streak = 0;
        self.clutch_said = false;
        self.team_out_said = false;
        self.mvps = None;
        self.last_bomb = None;
        self.seen_enemies.clear();
    }

    /// Say a line for `trigger` if the assistant is on and the rules allow it now. Twins: her
    /// line, then his (" / " in the lines file).
    fn say(&mut self, out: &mut Vec<Line>, trigger: &str, kind: Kind, now: Now, p: &Prefs, slots: &[(&str, String)]) {
        let who = p.who;
        if who >= NAMES.len() || !self.speaker.allow(kind, now) {
            return;
        }
        let Some((mood, his, text)) = self.lines.pick(trigger, who) else { return };
        let mut all = vec![("name", p.name.clone())];
        all.extend(slots.iter().cloned());
        for (i, text) in fill(&text, &all).split(" / ").enumerate() {
            let mood = if i == 0 { mood } else { his.unwrap_or(mood.brother_to()) };
            out.push(Line { who, mood, text: text.to_string(), second: i > 0 });
        }
    }

    /// One frame: this frame's game events and what you know into lines (oldest first; the
    /// newest is the one to show). `t` is seconds on any steady clock.
    pub fn update(&mut self, events: &[GameEvent], k: &Known, p: &Prefs, t: f32) -> Vec<Line> {
        let mut out = Vec::new();
        let out = &mut out;
        let me = k.me;
        let my_team = k.team;
        let now = Now { t, live: k.phase == Phase::Live, alive: k.alive, in_round: p.in_round, chattiness: p.chattiness };
        let name_of = |id: u8| k.players.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_default();
        let team_of = |id: u8| k.players.iter().find(|s| s.id == id).map(|s| s.team);
        if k.practice && !self.practice_hello && me.is_some() {
            self.practice_hello = true;
            self.say(out, "practice_start", Kind::Personal, now, p, &[]);
        }
        // You picked up a body (practice too: it's new to you).
        if k.carrying && !self.carrying {
            self.say(out, "body_grabbed", Kind::Personal, now, p, &[]);
        }
        self.carrying = k.carrying;
        // What the radar and scoreboard show, the assistant knows too.
        if !k.practice && my_team != Team::Spectator {
            // Freeze time over (CS plays its radio "Go go go" here; ours is the assistant, silent).
            if self.last_phase == Some(Phase::Freeze) && k.phase == Phase::Live && k.alive {
                self.say(out, "round_live", Kind::Relay, now, p, &[]);
            }
            self.last_phase = Some(k.phase);
            let bomb = k.bomb;
            if self.last_bomb != Some(bomb) {
                match (self.last_bomb, bomb) {
                    (Some(BombNet::Carried(_)), BombNet::Dropped(_)) => self.say(out, "bomb_dropped", Kind::Relay, now, p, &[]),
                    (_, BombNet::Carried(id)) if Some(id) == me && !matches!(self.last_bomb, Some(BombNet::Carried(_))) => self.say(out, "bomb_yours", Kind::Relay, now, p, &[]),
                    _ => {}
                }
                self.last_bomb = Some(bomb);
            }
            // Enemies the server shows you (only ones in sight): a line when new ones appear.
            let new = k.enemies_seen.iter().filter(|id| !self.seen_enemies.contains(id)).count();
            if new > 0 && p.relay {
                let n = k.enemies_seen.len().to_string();
                self.say(out, "enemy_spotted", Kind::Relay, now, p, &[("count", n)]);
            }
            self.seen_enemies = k.enemies_seen.clone();
            // The scoreboard gave you a round MVP star.
            if let Some(mine) = k.players.iter().find(|s| Some(s.id) == me).map(|s| s.mvps) {
                if self.mvps.is_some_and(|before| mine > before) {
                    self.say(out, "mvp", Kind::Personal, now, p, &[]);
                }
                self.mvps = Some(mine);
            }
        }
        for e in events {
            match e {
                GameEvent::Shot { shooter, .. } if Some(*shooter) == me => self.speaker.combat(t),
                GameEvent::Damage { attacker, victim, .. } if Some(*attacker) == me || Some(*victim) == me => self.speaker.combat(t),
                GameEvent::RoundStart { round } => {
                    self.speaker.new_round();
                    self.streak = 0;
                    self.clutch_said = false;
                    self.team_out_said = false;
                    if k.practice || my_team == Team::Spectator {
                        continue;
                    }
                    if *round == 1 {
                        self.say(out, "match_start", Kind::Personal, now, p, &[]);
                    }
                    let slots = [("money", k.money.to_string())];
                    if k.money < LOW_MONEY {
                        self.say(out, "buy_low", Kind::Personal, now, p, &slots);
                    } else if k.money >= RICH_MONEY {
                        self.say(out, "buy_rich", Kind::Personal, now, p, &slots);
                    }
                }
                GameEvent::RoundEnd { winner, .. } if my_team != Team::Spectator => {
                    // Won it as the last one standing: a clutch.
                    let trigger = match *winner == my_team {
                        true if self.clutch_said && k.alive => "clutch_won",
                        true => "round_won",
                        false => "round_lost",
                    };
                    self.say(out, trigger, Kind::Personal, now, p, &[]);
                }
                GameEvent::Kill { killer, victim, .. } => {
                    if Some(*victim) == me {
                        self.streak = 0;
                        self.say(out, "you_died", Kind::Personal, now, p, &[]);
                    } else if Some(*killer) == me {
                        self.streak += 1;
                        if self.streak == 3 || self.streak == 5 {
                            let s = self.streak.to_string();
                            self.say(out, "streak", Kind::Relay, now, p, &[("streak", s)]);
                        }
                    } else if p.relay && team_of(*victim) == Some(my_team) {
                        self.say(out, "teammate_down", Kind::Relay, now, p, &[("teammate", name_of(*victim))]);
                    }
                    // Last one standing on your team, enemies still up: a clutch.
                    let alive = |team: Team| k.players.iter().filter(|s| s.team == team && s.alive && Some(s.id) != me).count();
                    let enemy = if my_team == Team::T { Team::Ct } else { Team::T };
                    if !k.practice && !self.clutch_said && k.alive && my_team != Team::Spectator && alive(my_team) == 0 && alive(enemy) > 0 {
                        self.clutch_said = true;
                        self.say(out, "last_alive", Kind::Critical, now, p, &[]);
                    }
                    // The whole team is down (you're watching where the last of you fell).
                    if !k.practice && !self.team_out_said && my_team != Team::Spectator && !k.alive && team_of(*victim) == Some(my_team)
                        && !k.players.iter().any(|s| s.team == my_team && s.alive && Some(s.id) != me && s.id != *victim)
                    {
                        self.team_out_said = true;
                        self.say(out, "team_out", Kind::Personal, now, p, &[]);
                    }
                }
                // A real white-out (not a glance from behind).
                GameEvent::Flash { blinded, .. } if blinded.iter().any(|(id, b)| Some(*id) == me && b.hold > 0.5) => self.say(out, "flashed", Kind::Personal, now, p, &[]),
                GameEvent::BombPlanted { .. } => self.say(out, "bomb_planted", Kind::Critical, now, p, &[]),
                GameEvent::BombDefused { .. } => self.say(out, "bomb_defused", Kind::Critical, now, p, &[]),
                GameEvent::SidesSwapped => self.say(out, "halftime", Kind::Personal, now, p, &[]),
                GameEvent::MatchOver { winner } if my_team != Team::Spectator => {
                    let trigger = if *winner == Some(my_team) { "match_won" } else { "match_lost" };
                    self.say(out, trigger, Kind::Personal, now, p, &[]);
                }
                _ => {}
            }
        }
        std::mem::take(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefs(who: usize) -> Prefs {
        Prefs { who, name: "anas".into(), in_round: true, chattiness: 1, relay: true }
    }

    /// You (id 1, CT) alive in a live round, a teammate and two enemies up.
    fn known() -> Known {
        let player = |id: u8, team: Team| Player { id, name: format!("p{id}"), team, alive: true, mvps: 0 };
        Known {
            me: Some(1),
            team: Team::Ct,
            practice: false,
            phase: Phase::Live,
            alive: true,
            carrying: false,
            bomb: BombNet::None,
            money: 3000,
            enemies_seen: Vec::new(),
            players: vec![player(1, Team::Ct), player(2, Team::Ct), player(3, Team::T), player(4, Team::T)],
        }
    }

    fn kill(killer: u8, victim: u8) -> GameEvent {
        GameEvent::Kill { killer, victim, weapon: None, headshot: false }
    }

    #[test]
    fn the_plant_is_called_and_twins_answer_each_other() {
        let mut b = Brain::new();
        let said = b.update(&[GameEvent::BombPlanted { by: 3, at: [0.0; 3] }], &known(), &prefs(0), 10.0);
        assert_eq!(said.len(), 1);
        assert_eq!(said[0].who, 0);
        assert!(!said[0].second);
        let mut b = Brain::new();
        let said = b.update(&[GameEvent::BombDefused { by: 1 }], &known(), &prefs(2), 10.0);
        assert_eq!(said.iter().map(|l| l.second).collect::<Vec<_>>(), [false, true]);
    }

    #[test]
    fn no_assistant_says_nothing() {
        let mut b = Brain::new();
        assert!(b.update(&[GameEvent::BombPlanted { by: 3, at: [0.0; 3] }], &known(), &prefs(NAMES.len()), 10.0).is_empty());
    }

    #[test]
    fn personal_lines_wait_while_you_play() {
        let mut b = Brain::new();
        let mut k = known();
        assert!(b.update(&[GameEvent::SidesSwapped], &k, &prefs(0), 10.0).is_empty());
        k.phase = Phase::Freeze;
        assert_eq!(b.update(&[GameEvent::SidesSwapped], &k, &prefs(0), 20.0).len(), 1);
    }

    #[test]
    fn teammate_down_then_the_clutch() {
        let mut b = Brain::new();
        let mut k = known();
        k.players[1].alive = false;
        let said = b.update(&[kill(3, 2)], &k, &prefs(0), 10.0);
        // teammate down (relay), then last alive (critical, past the gap): show the newest
        assert_eq!(said.len(), 2);
        let mut l = Lines::parse(include_str!("lines.txt"));
        let pool: Vec<String> = (0..64).filter_map(|_| l.pick("last_alive", 0)).map(|x| x.2).collect();
        assert!(pool.contains(&said[1].text), "{}", said[1].text);
        // won as the last one standing
        k.phase = Phase::Over;
        let said = b.update(&[GameEvent::RoundEnd { winner: Team::Ct, reason: net::rounds::EndReason::Eliminated }], &k, &prefs(0), 20.0);
        let mut l = Lines::parse(include_str!("lines.txt"));
        let pool: Vec<String> = (0..64).filter_map(|_| l.pick("clutch_won", 0)).map(|x| x.2).collect();
        assert!(pool.contains(&said[0].text), "{}", said[0].text);
    }

    #[test]
    fn new_enemies_in_sight_are_relayed_once() {
        let mut b = Brain::new();
        let mut k = known();
        k.enemies_seen = vec![3];
        let said = b.update(&[], &k, &prefs(0), 10.0);
        assert_eq!(said.len(), 1);
        assert!(b.update(&[], &k, &prefs(0), 30.0).is_empty(), "same enemy, no news");
        let p = Prefs { relay: false, ..prefs(0) };
        k.enemies_seen = vec![3, 4];
        assert!(b.update(&[], &k, &p, 50.0).is_empty(), "relay off");
    }
}
