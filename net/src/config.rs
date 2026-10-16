//! Match settings, read from a server config file (`key = value` lines, `#` comments).
//! Defaults are the ones in docs/RULES.md: CS2 Competitive format, CS 1.6 economy.

/// How the losing team's bonus grows (docs/RULES.md 3.3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LossBonus {
    /// Each team has its own ladder: $1400, +$500 per loss in a row, up to $3400; a win resets it.
    PerTeam,
    /// CS 1.6 exactly: one shared bonus for both teams.
    Cs16,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchConfig {
    /// Rounds in regulation (24 = MR12: first to 13, sides swap after 12).
    pub max_rounds: u32,
    /// Rounds per overtime at a tie (6 = MR3); 0 = a tie ends the match as a draw.
    pub overtime_rounds: u32,
    pub overtime_money: i32,
    /// Seconds.
    pub freeze_time: f32,
    /// Seconds of buying after freeze time ends.
    pub buy_time: f32,
    pub round_time: f32,
    pub c4_timer: f32,
    pub defuse_time: f32,
    pub defuse_kit_time: f32,
    /// Seconds between a round ending and the next one starting.
    pub round_end_delay: f32,
    pub start_money: i32,
    pub max_money: i32,
    pub loss_bonus: LossBonus,
    /// Bullets and grenades hurt teammates.
    pub friendly_fire: bool,
    /// How far proximity voice carries (units); it fades out towards this distance.
    pub voice_range: f32,
}

impl Default for MatchConfig {
    fn default() -> Self {
        MatchConfig {
            max_rounds: 24,
            overtime_rounds: 6,
            overtime_money: 10_000,
            freeze_time: 15.0,
            buy_time: 20.0,
            round_time: 115.0,
            c4_timer: 40.0,
            defuse_time: 10.0,
            defuse_kit_time: 5.0,
            round_end_delay: 5.0,
            start_money: 800,
            max_money: 16_000,
            loss_bonus: LossBonus::PerTeam,
            friendly_fire: false,
            voice_range: 1500.0,
        }
    }
}

impl MatchConfig {
    /// Settings from config text; keys not given keep their defaults. Unknown keys and bad
    /// values are errors, so a typo never silently changes the rules.
    pub fn parse(text: &str) -> Result<MatchConfig, String> {
        let mut c = MatchConfig::default();
        for (n, line) in text.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let (k, v) = line.split_once('=').ok_or_else(|| format!("line {}: expected key = value", n + 1))?;
            let (k, v) = (k.trim(), v.trim().trim_matches('"'));
            let bad = || format!("line {}: bad value for {k}: {v}", n + 1);
            let num = |v: &str| v.parse::<f32>().map_err(|_| bad());
            match k {
                "max_rounds" => c.max_rounds = v.parse().map_err(|_| bad())?,
                "overtime_rounds" => c.overtime_rounds = v.parse().map_err(|_| bad())?,
                "overtime_money" => c.overtime_money = v.parse().map_err(|_| bad())?,
                "freeze_time" => c.freeze_time = num(v)?,
                "buy_time" => c.buy_time = num(v)?,
                "round_time" => c.round_time = num(v)?,
                "c4_timer" => c.c4_timer = num(v)?,
                "defuse_time" => c.defuse_time = num(v)?,
                "defuse_kit_time" => c.defuse_kit_time = num(v)?,
                "round_end_delay" => c.round_end_delay = num(v)?,
                "start_money" => c.start_money = v.parse().map_err(|_| bad())?,
                "max_money" => c.max_money = v.parse().map_err(|_| bad())?,
                "loss_bonus" => {
                    c.loss_bonus = match v {
                        "per_team" => LossBonus::PerTeam,
                        "cs16" => LossBonus::Cs16,
                        _ => return Err(bad()),
                    }
                }
                "friendly_fire" => c.friendly_fire = v.parse().map_err(|_| bad())?,
                "voice_range" => c.voice_range = num(v)?,
                _ => return Err(format!("line {}: unknown setting {k}", n + 1)),
            }
        }
        if c.max_rounds % 2 != 0 || c.overtime_rounds % 2 != 0 {
            return Err("max_rounds and overtime_rounds must be even (two halves)".into());
        }
        Ok(c)
    }

    pub fn load(path: &std::path::Path) -> Result<MatchConfig, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        MatchConfig::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_rejects_typos() {
        let c = MatchConfig::parse("# comment\nmax_rounds = 30\nloss_bonus = \"cs16\"\nfreeze_time=20 # pro\n").unwrap();
        assert_eq!((c.max_rounds, c.loss_bonus, c.freeze_time, c.start_money), (30, LossBonus::Cs16, 20.0, 800));
        assert!(MatchConfig::parse("freezetime = 3").is_err());
        assert!(MatchConfig::parse("max_rounds = 25").is_err());
        assert_eq!(MatchConfig::parse("").unwrap(), MatchConfig::default());
    }

    #[test]
    fn shipped_config_matches_the_defaults() {
        assert_eq!(MatchConfig::parse(include_str!("../../config/server.cfg")).unwrap(), MatchConfig::default());
    }
}
