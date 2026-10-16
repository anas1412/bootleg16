//! Motion (design/mockups/ui/motion.css). Calm at rest; every change tunes in. Everything is
//! eased per frame from the real clock, so it runs the same at any frame rate.
//! - `Anim::tune_in` / `tune_out`: a part enters split into its two channels and settles clean
//!   (320 ms), or leaves the same way faster (180 ms, then it is despawned). The whole subtree
//!   fades and its text splits: a pink copy beside it, the text itself tinted cyan.
//! - `Anim::burst`: a hover or a choice: a 240 ms jitter and split that settles fully clean.
//! - `Anim::roll`: a stepped value rolls in from the side it moved toward.
//! - `Slide`: one selection per list or chip group, sliding (280 ms) to the chosen item.
//! - `Fade` / `FadeBorder`: a background or frame eases to its target colour (hovers).
//! - `Pulse`: live markers breathe.
//! - `Blurs`: a menu over the game. The game blurs, the shared scrim dims it, a scanline
//!   sweeps once, and the HUD (`HudLayer`) hides; closing reverses it.
mod anim;
mod blur;
mod live;

pub use anim::Anim;
pub use blur::{Blurs, Covered, HudLayer};
pub use live::*;

use super::tokens::*;
use bevy::prelude::*;
use std::collections::HashMap;

/// The background colour a node eases toward.
#[derive(Component)]
pub struct Fade(pub Color);

/// The frame colour a node eases toward.
#[derive(Component)]
pub struct FadeBorder(pub Color);

/// Breathe: the background's alpha swings between `low` and full.
#[derive(Component)]
pub struct Pulse {
    pub base: Color,
    pub low: f32,
}

impl Pulse {
    pub fn new(base: Color) -> Pulse {
        Pulse { base, low: 0.35 }
    }
}

/// A sliding selection: this node moves to cover `target` (a sibling inside the same parent).
/// Its last place is remembered under `key`, so a rebuilt list slides on from where it was.
#[derive(Component)]
pub struct Slide {
    pub key: String,
    pub target: Entity,
}

/// What the UI remembers across rebuilds: where each selection was, each shown value.
#[derive(Resource, Default)]
pub struct UiMemory {
    rects: HashMap<String, (Vec2, Vec2)>,
    values: HashMap<String, String>,
}

impl UiMemory {
    /// Remember `value` under `key`; true if it differs from last time (and there was one).
    pub fn changed(&mut self, key: &str, value: &str) -> bool {
        match self.values.insert(key.to_string(), value.to_string()) {
            Some(old) => old != value,
            None => false,
        }
    }
    /// The previous value under `key`, if any.
    pub fn previous(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(|s| s.as_str())
    }
}

/// Ease factor for a time constant: about 95% of the way in 3 τ, at any frame rate.
pub fn ease(dt: f32, tau: f32) -> f32 {
    1.0 - (-dt / tau).exp()
}

pub(crate) fn lerp_color(a: Color, b: Color, k: f32) -> Color {
    let (a, b) = (a.to_srgba(), b.to_srgba());
    let m = |x: f32, y: f32| if (y - x).abs() < 0.002 { y } else { x + (y - x) * k };
    Color::srgba(m(a.red, b.red), m(a.green, b.green), m(a.blue, b.blue), m(a.alpha, b.alpha))
}

fn fade(time: Res<Time>, mut q: Query<(Option<&Fade>, Option<&FadeBorder>, Option<&mut BackgroundColor>, Option<&mut BorderColor>), Or<(With<Fade>, With<FadeBorder>)>>) {
    let k = ease(time.delta_secs(), T_HOVER / 3.0);
    for (f, fb, bg, border) in &mut q {
        if let (Some(f), Some(mut bg)) = (f, bg) {
            if bg.0 != f.0 {
                bg.0 = lerp_color(bg.0, f.0, k);
            }
        }
        if let (Some(fb), Some(mut b)) = (fb, border) {
            if b.top != fb.0 {
                *b = BorderColor::all(lerp_color(b.top, fb.0, k));
            }
        }
    }
}

fn slide(time: Res<Time>, mut memory: ResMut<UiMemory>, mut inds: Query<(&Slide, &mut Node, &ChildOf)>, nodes: Query<(&ComputedNode, &UiGlobalTransform)>) {
    let k = ease(time.delta_secs(), T_SLIDE / 3.0);
    for (s, mut node, parent) in &mut inds {
        let (Ok((tn, tt)), Ok((pn, pt))) = (nodes.get(s.target), nodes.get(parent.parent())) else { continue };
        if tn.size.x <= 0.0 {
            continue;
        }
        let inv = tn.inverse_scale_factor;
        let corner = |n: &ComputedNode, t: &UiGlobalTransform| t.affine().translation - n.size / 2.0;
        let pos = (corner(tn, tt) - corner(pn, pt)) * inv;
        let size = tn.size * inv;
        let cur = memory.rects.entry(s.key.clone()).or_insert((pos, size));
        cur.0 += (pos - cur.0) * k;
        cur.1 += (size - cur.1) * k;
        let (p, z) = *cur;
        node.left = px(p.x);
        node.top = px(p.y);
        node.width = px(z.x);
        node.height = px(z.y);
    }
}

fn pulse(time: Res<Time>, mut q: Query<(&Pulse, &mut BackgroundColor)>) {
    let s = (time.elapsed_secs() * std::f32::consts::TAU / 1.6).sin() * 0.5 + 0.5;
    for (p, mut bg) in &mut q {
        bg.0 = p.base.with_alpha(p.base.alpha() * (p.low + (1.0 - p.low) * s));
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<UiMemory>().add_systems(Update, (fade, anim::anim, slide, pulse));
    blur::plugin(app);
    live::plugin(app);
}
