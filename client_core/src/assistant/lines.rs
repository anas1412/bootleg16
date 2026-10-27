//! The assistant's lines: `lines.txt` next to this file, picked without repeats.
use std::collections::HashMap;

/// The 9 reactions, in face-sheet column order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mood {
    Neutral,
    Happy,
    Laughing,
    Smug,
    Shocked,
    Thinking,
    Worried,
    Sad,
    Annoyed,
}

impl Mood {
    pub fn parse(s: &str) -> Mood {
        match s {
            "happy" => Mood::Happy,
            "laughing" => Mood::Laughing,
            "smug" => Mood::Smug,
            "shocked" => Mood::Shocked,
            "thinking" => Mood::Thinking,
            "worried" => Mood::Worried,
            "sad" => Mood::Sad,
            "annoyed" => Mood::Annoyed,
            _ => Mood::Neutral,
        }
    }
    /// The calm twin's answer to his sister's mood, when the line doesn't give his.
    pub fn brother_to(self) -> Mood {
        match self {
            Mood::Annoyed | Mood::Smug | Mood::Laughing | Mood::Neutral => Mood::Neutral,
            Mood::Worried | Mood::Shocked => Mood::Thinking,
            Mood::Happy => Mood::Happy,
            Mood::Sad => Mood::Sad,
            Mood::Thinking => Mood::Thinking,
        }
    }
}

/// Lines by (trigger, assistant), each pool drawn like a shuffled deck: every line once
/// before any repeats, and never the same line twice in a row.
pub struct Lines {
    pools: HashMap<(String, usize), Vec<(Mood, Option<Mood>, String)>>,
    decks: HashMap<(String, usize), Vec<usize>>,
    last: HashMap<(String, usize), usize>,
}

/// Assistant names as written in the lines file, in `NAMES` order.
const KEYS: [&str; 4] = ["claudette", "childgbt", "twins", "grumpk"];

impl Lines {
    /// `trigger | assistant | mood | text` rows; `#` comments and blank rows skipped. Twins
    /// may give each twin a mood: `annoyed/neutral` (hers, then his).
    pub fn parse(src: &str) -> Lines {
        let mut pools: HashMap<(String, usize), Vec<(Mood, Option<Mood>, String)>> = HashMap::new();
        for row in src.lines().map(str::trim).filter(|r| !r.is_empty() && !r.starts_with('#')) {
            let f: Vec<&str> = row.splitn(4, '|').map(str::trim).collect();
            let [trigger, who, mood, text] = f[..] else { continue };
            let Some(who) = KEYS.iter().position(|k| *k == who) else { continue };
            let (first, second) = match mood.split_once('/') {
                Some((a, b)) => (a.trim(), Some(Mood::parse(b.trim()))),
                None => (mood, None),
            };
            pools.entry((trigger.to_string(), who)).or_default().push((Mood::parse(first), second, text.to_string()));
        }
        Lines { pools, decks: HashMap::new(), last: HashMap::new() }
    }

    /// The next line for this trigger and assistant, or None if it has none.
    pub fn pick(&mut self, trigger: &str, who: usize) -> Option<(Mood, Option<Mood>, String)> {
        let key = (trigger.to_string(), who);
        let pool = self.pools.get(&key)?;
        let deck = self.decks.entry(key.clone()).or_default();
        if deck.is_empty() {
            *deck = (0..pool.len()).collect();
            fastrand::shuffle(deck);
            // Don't start the new round with the line that ended the last one.
            if let (Some(&last), true) = (self.last.get(&key), deck.len() > 1) {
                if deck.last() == Some(&last) {
                    deck.swap(0, pool.len() - 1);
                }
            }
        }
        let i = deck.pop()?;
        self.last.insert(key, i);
        Some(pool[i].clone())
    }
}

/// Replace `{slot}`s with values.
pub fn fill(text: &str, slots: &[(&str, String)]) -> String {
    slots.iter().fold(text.to_string(), |t, (k, v)| t.replace(&format!("{{{k}}}"), v))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "# c\nhello | claudette | happy | Hi {name}.\nhello | claudette | neutral | Yo.\nhello | grumpk | annoyed | Ugh.\nbad row\n";

    #[test]
    fn parses_rows_and_moods() {
        let mut l = Lines::parse(SRC);
        assert_eq!(l.pick("hello", 3), Some((Mood::Annoyed, None, "Ugh.".into())));
        assert_eq!(l.pick("hello", 1), None);
        assert_eq!(l.pick("missing", 0), None);
    }

    #[test]
    fn no_repeats_until_pool_used() {
        let mut l = Lines::parse(SRC);
        for _ in 0..50 {
            let a = l.pick("hello", 0).unwrap().2;
            let b = l.pick("hello", 0).unwrap().2;
            assert_ne!(a, b, "two-line pool must alternate");
        }
    }

    #[test]
    fn twins_moods_split_hers_and_his() {
        let mut l = Lines::parse("hello | twins | annoyed/happy | Ugh. / Hi.");
        assert_eq!(l.pick("hello", 2), Some((Mood::Annoyed, Some(Mood::Happy), "Ugh. / Hi.".into())));
        assert_eq!(Mood::Annoyed.brother_to(), Mood::Neutral);
    }

    #[test]
    fn fills_slots() {
        assert_eq!(fill("Hi {name}, ${money}.", &[("name", "anas".into()), ("money", "800".into())]), "Hi anas, $800.");
    }

    #[test]
    fn shipped_file_has_every_trigger_for_every_assistant() {
        let mut l = Lines::parse(include_str!("lines.txt"));
        for t in super::super::TRIGGERS {
            for who in 0..KEYS.len() {
                let line = l.pick(t, who).unwrap_or_else(|| panic!("no {t} line for {}", KEYS[who]));
                assert!(!line.2.contains('—'), "em dash in {t}/{}", KEYS[who]);
            }
        }
    }
}
