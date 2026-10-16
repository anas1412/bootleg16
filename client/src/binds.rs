//! Key bindings: every action the player can rebind, with CS's defaults, saved in
//! ~/.config/bootleg16/binds.txt by name ("forward=KeyW", "fire=MouseLeft"). Game code asks
//! `Binds` (`held`, `just`) instead of naming keys. Rebinding: `capture` waits for the next key
//! or mouse button (Esc cancels); a key already used moves over and the other action takes the
//! old one, so nothing is ever left unbound or doubled.
//! Keys are kept by position (Bevy's `KeyCode`), so the same keys sit in the same place on any
//! layout; labels show what they print on yours (`Layout`, learned from typing: AZERTY shows Z
//! Q S D for movement).
//! Fixed on purpose: Esc (menus), Enter, the buy menu's number keys, the mouse wheel.
use bevy::prelude::*;
use std::path::PathBuf;

/// A key or a mouse button, saved by name ("KeyK", "MouseBack").
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Bind {
    Key(KeyCode),
    Mouse(MouseButton),
}

/// Keys that can be bound (by their saved names).
pub const KEYS: [KeyCode; 62] = {
    use KeyCode::*;
    [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO, KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ, Digit0, Digit1,
        Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, Space, Tab, CapsLock, ShiftLeft, ShiftRight,
        ControlLeft, ControlRight, AltLeft, AltRight, Backquote, Comma, Period, Slash, Semicolon,
    ]
};
pub const MICE: [MouseButton; 5] = [MouseButton::Left, MouseButton::Right, MouseButton::Middle, MouseButton::Back, MouseButton::Forward];

impl Bind {
    pub fn name(self) -> String {
        match self {
            Bind::Key(k) => format!("{k:?}"),
            Bind::Mouse(m) => format!("Mouse{m:?}"),
        }
    }

    pub fn parse(s: &str) -> Option<Bind> {
        KEYS.iter().find(|k| format!("{k:?}") == s).map(|k| Bind::Key(*k)).or_else(|| MICE.iter().find(|m| format!("Mouse{m:?}") == s).map(|m| Bind::Mouse(*m)))
    }

    /// As shown to the player on this keyboard layout: "K", "5", "Space", "Mouse Back".
    pub fn label(self, layout: Layout) -> String {
        // keys are kept by position; on AZERTY some positions print another letter
        if let (Layout::Azerty, Bind::Key(k)) = (layout, self) {
            let swapped = match k {
                KeyCode::KeyA => Some("Q"),
                KeyCode::KeyQ => Some("A"),
                KeyCode::KeyW => Some("Z"),
                KeyCode::KeyZ => Some("W"),
                KeyCode::Semicolon => Some("M"),
                KeyCode::KeyM => Some(","),
                _ => None,
            };
            if let Some(s) = swapped {
                return s.into();
            }
        }
        let n = self.name();
        let n = n.strip_prefix("Key").or_else(|| n.strip_prefix("Digit")).map(str::to_string).unwrap_or(n);
        match n.as_str() {
            "ControlLeft" => "Ctrl".into(),
            "ControlRight" => "Right Ctrl".into(),
            "ShiftLeft" => "Shift".into(),
            "ShiftRight" => "Right Shift".into(),
            "AltLeft" => "Alt".into(),
            "AltRight" => "Right Alt".into(),
            "MouseLeft" => "Mouse 1".into(),
            "MouseRight" => "Mouse 2".into(),
            "MouseMiddle" => "Mouse 3".into(),
            _ => n.replace("Mouse", "Mouse "),
        }
    }

    pub fn held(self, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
        match self {
            Bind::Key(k) => keys.pressed(k),
            Bind::Mouse(m) => mouse.pressed(m),
        }
    }

    pub fn just(self, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
        match self {
            Bind::Key(k) => keys.just_pressed(k),
            Bind::Mouse(m) => mouse.just_pressed(m),
        }
    }
}

/// The keyboard's layout, learned from what keys type (see `learn_layout`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Layout {
    #[default]
    Qwerty,
    Azerty,
}

/// What a key does. The order is the order on the Keys page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    Forward,
    Back,
    Left,
    Right,
    Jump,
    Crouch,
    Walk,
    Use,
    Fire,
    AltFire,
    Reload,
    Drop,
    LastWeapon,
    Primary,
    Secondary,
    Knife,
    Grenade,
    Bomb,
    Buy,
    Scores,
    Teams,
    TeamVoice,
    NearVoice,
    VoteYes,
    VoteNo,
    Surrender,
}

use Action::*;

/// Every action with its saved name, its label and CS's default.
pub const ACTIONS: [(Action, &str, &str, Bind); 26] = [
    (Forward, "forward", "Forward", Bind::Key(KeyCode::KeyW)),
    (Back, "back", "Back", Bind::Key(KeyCode::KeyS)),
    (Left, "left", "Left", Bind::Key(KeyCode::KeyA)),
    (Right, "right", "Right", Bind::Key(KeyCode::KeyD)),
    (Jump, "jump", "Jump", Bind::Key(KeyCode::Space)),
    (Crouch, "crouch", "Crouch", Bind::Key(KeyCode::ControlLeft)),
    (Walk, "walk", "Walk", Bind::Key(KeyCode::ShiftLeft)),
    (Use, "use", "Use", Bind::Key(KeyCode::KeyE)),
    (Fire, "fire", "Fire", Bind::Mouse(MouseButton::Left)),
    (AltFire, "alt_fire", "Alt fire, scope", Bind::Mouse(MouseButton::Right)),
    (Reload, "reload", "Reload", Bind::Key(KeyCode::KeyR)),
    (Drop, "drop", "Drop", Bind::Key(KeyCode::KeyG)),
    (LastWeapon, "last_weapon", "Last weapon", Bind::Key(KeyCode::KeyQ)),
    (Primary, "primary", "Primary", Bind::Key(KeyCode::Digit1)),
    (Secondary, "secondary", "Pistol", Bind::Key(KeyCode::Digit2)),
    (Knife, "knife", "Knife", Bind::Key(KeyCode::Digit3)),
    (Grenade, "grenade", "Grenades", Bind::Key(KeyCode::Digit4)),
    (Bomb, "bomb", "Bomb", Bind::Key(KeyCode::Digit5)),
    (Buy, "buy", "Buy menu", Bind::Key(KeyCode::KeyB)),
    (Scores, "scores", "Scoreboard", Bind::Key(KeyCode::Tab)),
    (Teams, "teams", "Choose team", Bind::Key(KeyCode::KeyM)),
    // V rather than CS's K: next to the movement keys
    (TeamVoice, "team_voice", "Team voice", Bind::Key(KeyCode::KeyV)),
    (NearVoice, "near_voice", "Proximity voice", Bind::Key(KeyCode::KeyX)),
    (VoteYes, "vote_yes", "Vote yes", Bind::Key(KeyCode::F1)),
    (VoteNo, "vote_no", "Vote no", Bind::Key(KeyCode::F2)),
    (Surrender, "surrender", "Call surrender", Bind::Key(KeyCode::F3)),
];

/// The Keys page's three columns: (title, first, last) into `ACTIONS`.
pub const GROUPS: [(&str, usize, usize); 3] = [("Move", 0, 8), ("Weapons", 8, 18), ("Game · voice", 18, 26)];

/// The bindings, and the action waiting for a key on the Keys page.
#[derive(Resource, Clone)]
pub struct Binds {
    keys: [Bind; ACTIONS.len()],
    pub capture: Option<Action>,
    pub layout: Layout,
}

fn index(a: Action) -> usize {
    ACTIONS.iter().position(|e| e.0 == a).expect("every action is listed")
}

impl Default for Binds {
    fn default() -> Self {
        Binds { keys: ACTIONS.map(|e| e.3), capture: None, layout: Layout::Qwerty }
    }
}

fn path() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config/bootleg16")
}

impl Binds {
    pub fn get(&self, a: Action) -> Bind {
        self.keys[index(a)]
    }

    pub fn held(&self, a: Action, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
        self.get(a).held(keys, mouse)
    }

    pub fn just(&self, a: Action, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
        self.get(a).just(keys, mouse)
    }

    /// Bind `a` to `b`; an action that had `b` takes `a`'s old key.
    pub fn set(&mut self, a: Action, b: Bind) {
        let i = index(a);
        let old = self.keys[i];
        if let Some(j) = self.keys.iter().position(|k| *k == b) {
            self.keys[j] = old;
        }
        self.keys[i] = b;
    }

    /// From the text of binds.txt (unknown names and keys are skipped).
    fn parse(text: &str) -> Binds {
        let mut b = Binds::default();
        for line in text.lines() {
            let Some((name, v)) = line.split_once('=') else { continue };
            if name.trim() == "layout" {
                b.layout = if v.trim() == "azerty" { Layout::Azerty } else { Layout::Qwerty };
                continue;
            }
            let (Some(e), Some(key)) = (ACTIONS.iter().find(|e| e.1 == name.trim()), Bind::parse(v.trim())) else { continue };
            b.set(e.0, key);
        }
        b
    }

    fn text(&self) -> String {
        let layout = if self.layout == Layout::Azerty { "azerty" } else { "qwerty" };
        format!("layout={layout}\n") + &ACTIONS.iter().zip(self.keys).map(|(e, k)| format!("{}={}\n", e.1, k.name())).collect::<String>()
    }

    pub fn load() -> Binds {
        let dir = path();
        if let Ok(text) = std::fs::read_to_string(dir.join("binds.txt")) {
            return Binds::parse(&text);
        }
        // before binds.txt, the voice keys were in voice.txt (keys changed from the old
        // defaults K and X carry over)
        let mut b = Binds::default();
        for line in std::fs::read_to_string(dir.join("voice.txt")).unwrap_or_default().lines() {
            match line.split_once('=') {
                Some(("team_key", v)) if v.trim() != "KeyK" => _ = Bind::parse(v.trim()).map(|k| b.set(TeamVoice, k)),
                Some(("near_key", v)) if v.trim() != "KeyX" => _ = Bind::parse(v.trim()).map(|k| b.set(NearVoice, k)),
                _ => {}
            }
        }
        b
    }

    pub fn save(&self) {
        let dir = path();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("binds.txt"), self.text());
    }
}

/// Waiting for a key: the next key or mouse button takes it (Esc cancels).
fn capture(mut b: ResMut<Binds>, keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>, mut dirty: ResMut<crate::menu::MenuDirty>, mut armed: Local<bool>) {
    let Some(a) = b.capture else {
        *armed = false;
        return;
    };
    // the click that asked for a key isn't the key
    if !*armed {
        *armed = true;
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        b.capture = None;
        dirty.0 = true;
        return;
    }
    let pressed = keys.get_just_pressed().find_map(|k| KEYS.contains(k).then_some(Bind::Key(*k))).or_else(|| mouse.get_just_pressed().find_map(|m| MICE.contains(m).then_some(Bind::Mouse(*m))));
    let Some(k) = pressed else { return };
    b.set(a, k);
    b.capture = None;
    b.save();
    dirty.0 = true;
}

/// The layout from what keys type: the key at QWERTY's Q typing "a" (or W typing "z") is
/// AZERTY. Learned from any key press, saved with the bindings, labels follow.
fn learn_layout(mut events: MessageReader<bevy::input::keyboard::KeyboardInput>, mut b: ResMut<Binds>, mut dirty: ResMut<crate::menu::MenuDirty>) {
    use bevy::input::keyboard::Key;
    for ev in events.read() {
        let Key::Character(c) = &ev.logical_key else { continue };
        let c = c.to_lowercase();
        let layout = match (ev.key_code, c.as_str()) {
            (KeyCode::KeyQ, "a") | (KeyCode::KeyW, "z") => Layout::Azerty,
            (KeyCode::KeyQ, "q") | (KeyCode::KeyW, "w") => Layout::Qwerty,
            _ => continue,
        };
        if b.layout != layout {
            b.layout = layout;
            b.save();
            dirty.0 = true;
        }
    }
}

pub struct BindsPlugin;

impl Plugin for BindsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Binds::load()).add_systems(Update, (capture.run_if(in_state(crate::menu::AppState::Menu)), learn_layout));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_taken_key_swaps_instead_of_doubling() {
        let mut b = Binds::default();
        b.set(Jump, Bind::Key(KeyCode::KeyW));
        assert_eq!(b.get(Jump), Bind::Key(KeyCode::KeyW));
        assert_eq!(b.get(Forward), Bind::Key(KeyCode::Space));
        let mut all: Vec<String> = b.keys.iter().map(|k| k.name()).collect();
        all.sort();
        all.dedup();
        assert_eq!(all.len(), ACTIONS.len());
    }

    #[test]
    fn saved_and_loaded_by_name() {
        let mut b = Binds::default();
        b.set(Fire, Bind::Mouse(MouseButton::Back));
        b.set(TeamVoice, Bind::Key(KeyCode::KeyC));
        let back = Binds::parse(&b.text());
        assert_eq!(back.get(Fire), Bind::Mouse(MouseButton::Back));
        assert_eq!(back.get(TeamVoice), Bind::Key(KeyCode::KeyC));
        assert_eq!(Binds::default().get(TeamVoice), Bind::Key(KeyCode::KeyV));
        assert_eq!(Binds::parse("nonsense\nforward=NoSuchKey\n").get(Forward), Bind::Key(KeyCode::KeyW));
    }

    #[test]
    fn azerty_labels_follow_the_layout() {
        let b = Binds::default();
        assert_eq!(b.get(Forward).label(Layout::Qwerty), "W");
        assert_eq!(b.get(Forward).label(Layout::Azerty), "Z");
        assert_eq!(b.get(Left).label(Layout::Azerty), "Q");
        assert_eq!(b.get(LastWeapon).label(Layout::Azerty), "A");
        assert_eq!(b.get(Jump).label(Layout::Azerty), "Space");
        let mut az = Binds::default();
        az.layout = Layout::Azerty;
        assert_eq!(Binds::parse(&az.text()).layout, Layout::Azerty);
    }

    #[test]
    fn groups_cover_every_action_once() {
        assert_eq!(GROUPS[0].1, 0);
        assert!(GROUPS.windows(2).all(|w| w[0].2 == w[1].1));
        assert_eq!(GROUPS[2].2, ACTIONS.len());
    }
}
