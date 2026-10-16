//! Orders for test bots, as text: `<who> <order> [args]`. Who: `all`, `t`, `ct`, or a bot
//! number (`3` or `bot3`). Several orders: separate them with `;`.
//!
//! Orders: `normal` (back to playing), `afk`, `hold` (stand here, shoot what comes),
//! `goto a|b|tspawn|ctspawn|x y z`, `rush a|b`, `plant a|b` (the site to take the bomb to /
//! guard, kept alongside the mode; `plant any` undoes it),
//! `peaceful` (never shoots), `spray` (fires nonstop), `aimbot` (instant perfect aim, for
//! anti-cheat tests), `grief` (shoots teammates), `bhop`, `knife`, `leave`, `rejoin`,
//! `difficulty easy|normal|hard|expert`, `buy <weapon>`.
use super::profile::{Difficulty, Profile};

#[derive(Clone, Debug, PartialEq)]
pub enum Who {
    All,
    Team(crate::protocol::Team),
    Bot(usize),
}

/// Where to go.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    Site(usize),
    TSpawn,
    CtSpawn,
    At([f32; 3]),
}

/// A standing test behaviour (replaces normal play until `normal`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    Normal,
    Afk,
    Hold,
    Goto(Place),
    /// Straight there, no retreating, shooting on the way.
    Rush(Place),
    Peaceful,
    Spray,
    Aimbot,
    Grief,
    Bhop,
    Knife,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Order {
    Mode(Mode),
    /// The site this bot plays (None: its own pick each round).
    Site(Option<usize>),
    Difficulty(Difficulty),
    Buy(sim::weapons::Id),
    Leave,
    Rejoin,
}

fn place(words: &[&str]) -> Option<Place> {
    Some(match words {
        ["a"] => Place::Site(0),
        ["b"] => Place::Site(1),
        ["tspawn"] => Place::TSpawn,
        ["ctspawn"] => Place::CtSpawn,
        [x, y, z] => Place::At([x.parse().ok()?, y.parse().ok()?, z.parse().ok()?]),
        _ => return None,
    })
}

/// One `<who> <order> [args]`; Err explains what was wrong.
pub fn parse(line: &str) -> Result<(Who, Order), String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let [who, order, args @ ..] = words.as_slice() else { return Err("usage: <all|t|ct|N> <order> [args]".into()) };
    let who = match *who {
        "all" => Who::All,
        "t" => Who::Team(crate::protocol::Team::T),
        "ct" => Who::Team(crate::protocol::Team::Ct),
        n => Who::Bot(n.trim_start_matches("bot").parse().map_err(|_| format!("who? {n}"))?),
    };
    let site = |args: &[&str]| match args {
        ["a"] => Ok(0),
        ["b"] => Ok(1),
        _ => Err("site a or b".to_string()),
    };
    let order = match (*order, args) {
        ("normal", []) => Order::Mode(Mode::Normal),
        ("afk", []) => Order::Mode(Mode::Afk),
        ("hold", []) => Order::Mode(Mode::Hold),
        ("peaceful", []) => Order::Mode(Mode::Peaceful),
        ("spray", []) => Order::Mode(Mode::Spray),
        ("aimbot", []) => Order::Mode(Mode::Aimbot),
        ("grief", []) => Order::Mode(Mode::Grief),
        ("bhop", []) => Order::Mode(Mode::Bhop),
        ("knife", []) => Order::Mode(Mode::Knife),
        ("goto", a) => Order::Mode(Mode::Goto(place(a).ok_or("goto a|b|tspawn|ctspawn|x y z")?)),
        ("rush", a) => Order::Mode(Mode::Rush(place(a).ok_or("rush a|b|tspawn|ctspawn|x y z")?)),
        ("plant", ["any"]) => Order::Site(None),
        ("plant", a) => Order::Site(Some(site(a)?)),
        ("difficulty", [d]) => Order::Difficulty(Profile::parse(d).ok_or("easy|normal|hard|expert")?),
        ("buy", [w]) => Order::Buy(sim::weapons::data::by_name(w).ok_or(format!("no weapon {w}"))?),
        ("leave", []) => Order::Leave,
        ("rejoin", []) => Order::Rejoin,
        (o, _) => return Err(format!("unknown order {o} (see net/src/bot/control.rs)")),
    };
    Ok((who, order))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Team;

    #[test]
    fn orders_parse() {
        assert_eq!(parse("t plant b"), Ok((Who::Team(Team::T), Order::Site(Some(1)))));
        assert_eq!(parse("bot3 goto 1 2 3"), Ok((Who::Bot(3), Order::Mode(Mode::Goto(Place::At([1.0, 2.0, 3.0]))))));
        assert_eq!(parse("all difficulty expert"), Ok((Who::All, Order::Difficulty(Difficulty::Expert))));
        assert_eq!(parse("2 buy ak47"), Ok((Who::Bot(2), Order::Buy(sim::weapons::Id::Ak47))));
        assert!(parse("all dance").is_err());
        assert!(parse("x afk").is_err());
    }
}
