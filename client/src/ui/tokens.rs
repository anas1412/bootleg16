//! Design tokens: Interference (docs/UI.md, design/mockups/ui/tokens.css name for name). The UI
//! is a signal: clean at rest; when something changes it splits into its two channels (Bureau
//! cyan, Strays pink) and tunes back in. Amber is you: your selection, your main action, your
//! row. The only place colours, sizes and timings are defined.
use bevy::prelude::*;

// ---- colour ----------------------------------------------------------------------------
/// Surfaces, darkest to lightest: page, panel, raised, selected.
pub const BG0: Color = Color::srgb(0.027, 0.031, 0.043);
pub const BG1: Color = Color::srgb(0.055, 0.067, 0.086);
pub const BG2: Color = Color::srgb(0.082, 0.102, 0.129);
pub const BG3: Color = Color::srgb(0.114, 0.141, 0.176);
/// Panels drawn over the game.
pub const HUD_BG: Color = Color::srgba(0.027, 0.035, 0.047, 0.76);
/// Text: primary, secondary, quiet. Over the game use TX1/TX2 only.
pub const TX1: Color = Color::srgb(0.918, 0.949, 0.973);
pub const TX2: Color = Color::srgb(0.557, 0.608, 0.659);
pub const TX3: Color = Color::srgb(0.337, 0.380, 0.427);
/// You: the selection, the main action, your row.
pub const AMBER: Color = Color::srgb(1.0, 0.627, 0.0);
pub const AMBER_INK: Color = Color::srgb(0.090, 0.055, 0.0);
/// The amber washes under a selection and a hovered main action.
pub const AMBER_WASH: Color = Color::srgba(1.0, 0.627, 0.0, 0.08);
pub const AMBER_WASH_2: Color = Color::srgba(1.0, 0.627, 0.0, 0.16);
/// The two channels, which are the two sides: Bureau (Team::Ct) cyan, Strays (Team::T) pink.
pub const CH_A: Color = Color::srgb(0.286, 0.776, 1.0);
pub const CH_B: Color = Color::srgb(1.0, 0.31, 0.545);
pub const CT_COLOR: Color = CH_A;
pub const T_COLOR: Color = CH_B;
/// States: good (money, ready, online), bad (low health, can't afford).
pub const OK: Color = Color::srgb(0.357, 0.890, 0.627);
pub const BAD: Color = Color::srgb(1.0, 0.251, 0.251);
/// Lines: hairlines, quiet frames, hovered frames.
pub const LINE: Color = Color::srgba(0.784, 0.902, 1.0, 0.10);
pub const LINE_2: Color = Color::srgba(0.784, 0.902, 1.0, 0.24);
pub const LINE_3: Color = Color::srgba(0.784, 0.902, 1.0, 0.45);
/// The faint wash under a hovered row or choice.
pub const HOVER: Color = Color::srgba(0.784, 0.902, 1.0, 0.04);
/// The glass plate a menu over the game sits on (tokens.css .plate: .55 in the browser; Bevy
/// blends in linear light, where the same dark reads lighter, so a little more here).
pub const PLATE_BG: Color = Color::srgba(0.055, 0.067, 0.086, 0.72);
/// The dim behind menus and popups drawn over the game (the blur does the rest).
pub const BACKDROP: Color = Color::srgba(0.027, 0.031, 0.043, 0.72);

// ---- space (1080p px; `vh()` scales them with the window) ------------------------------
pub const GAP_S: f32 = 8.0;
pub const GAP: f32 = 16.0;
pub const PAD: f32 = 24.0;
/// HUD distance from every screen edge.
pub const SAFE: f32 = 32.0;
/// The accent bar: you, or the main action, on the outer edge.
pub const BAR: f32 = 4.0;
/// Corner radius: panels and cards, and the smaller one for chips, tags and buttons.
pub const RADIUS: f32 = 6.0;
pub const RADIUS_S: f32 = 4.0;

// ---- type scale (1080p px) -------------------------------------------------------------
/// One step of the type scale: which font, what size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Type {
    /// Wordmark, hero names (Big Shoulders 150).
    Display,
    /// Page titles (Big Shoulders 52).
    H2,
    /// Section and card titles (Chakra Petch 28).
    H3,
    /// Running text (Chakra Petch 18).
    Body,
    /// Small caps labels (Chakra Petch 13, uppercase).
    Label,
    /// Japanese subtitle (Noto Sans JP 13).
    Jp,
    /// Numbers, large / medium / small (JetBrains Mono 56 / 32 / 18).
    NumL,
    NumM,
    NumS,
}

impl Type {
    pub fn size(self) -> f32 {
        match self {
            Type::Display => 150.0,
            Type::H2 => 52.0,
            Type::H3 => 28.0,
            Type::Body => 18.0,
            Type::Label | Type::Jp => 13.0,
            Type::NumL => 56.0,
            Type::NumM => 32.0,
            Type::NumS => 18.0,
        }
    }
}

// ---- motion (seconds) ------------------------------------------------------------------
/// Frames and fills on hover.
pub const T_HOVER: f32 = 0.14;
/// Tuning in: the channel split settles clean.
pub const T_TUNE: f32 = 0.32;
/// Tuning out.
pub const T_OUT: f32 = 0.18;
/// A hover or a choice: the burst.
pub const T_BURST: f32 = 0.24;
/// The selection moving between rows or chips.
pub const T_SLIDE: f32 = 0.28;
/// A stepped value rolling in.
pub const T_ROLL: f32 = 0.22;
/// Between staggered siblings.
pub const STAGGER: f32 = 0.04;
/// The blur and dim behind menus over the game.
pub const T_BLUR: f32 = 0.22;
/// Channel offset when something tunes in, at 1080p.
pub const SPLIT_IN: f32 = 8.0;
/// The scrim behind menus over the game: above the HUD, under the menus (7 and up).
pub const Z_SCRIM: i32 = 5;
