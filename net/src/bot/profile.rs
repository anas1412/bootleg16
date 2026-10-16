//! How good a bot is, after CS 1.6 / Condition Zero bot profiles (Skill, Aggression,
//! ReactionTime, Teamwork, difficulty Easy / Normal / Hard / Expert). CS 1.6 ships the bot
//! code but not its profile file, so these numbers are ours, tuned to feel like the names.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
    Expert,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub difficulty: Difficulty,
    /// 0..1: aim and how well it handles recoil and range.
    pub skill: f32,
    /// 0..1: pushes and chases (high) or holds and retreats (low).
    pub aggression: f32,
    /// Seconds an enemy must be in view before the bot reacts.
    pub reaction: f32,
    /// Aim error (degrees) when it first sees someone; shrinks while it tracks them.
    pub aim_error: f32,
}

impl Profile {
    pub fn of(d: Difficulty) -> Profile {
        let (skill, aggression, reaction, aim_error) = match d {
            Difficulty::Easy => (0.25, 0.3, 0.60, 9.0),
            Difficulty::Normal => (0.5, 0.5, 0.40, 6.0),
            Difficulty::Hard => (0.75, 0.6, 0.28, 3.5),
            Difficulty::Expert => (1.0, 0.7, 0.20, 1.5),
        };
        Profile { difficulty: d, skill, aggression, reaction, aim_error }
    }

    pub fn parse(s: &str) -> Option<Difficulty> {
        Some(match s {
            "easy" => Difficulty::Easy,
            "normal" => Difficulty::Normal,
            "hard" => Difficulty::Hard,
            "expert" => Difficulty::Expert,
            _ => return None,
        })
    }
}
