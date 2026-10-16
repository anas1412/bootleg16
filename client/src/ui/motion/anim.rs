//! Tuning in and out, bursts, slams and rolls: short keyframed animations on a node and its
//! whole subtree (alpha, offset, and the channel split on its text).
use crate::ui::kit::vh;
use crate::ui::tokens::*;
use super::Fade;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    TuneIn,
    TuneOut,
    Burst,
    /// A harder, longer burst (Grumpk's entrance).
    Slam,
    Roll { up: bool },
    /// A second ticking by: a small swell and split that settles.
    Tick,
}

/// A short animation on a node and everything inside it.
#[derive(Component)]
#[require(UiTransform)]
pub struct Anim {
    kind: Kind,
    delay: f32,
    start: Option<f32>,
}

impl Anim {
    pub fn tune_in(delay: f32) -> Anim {
        Anim { kind: Kind::TuneIn, delay, start: None }
    }
    /// The i-th part of a group tuning in, a beat after the one before.
    pub fn stagger(i: usize) -> Anim {
        Anim::tune_in(i as f32 * STAGGER)
    }
    /// Leave: tune out, then the node is despawned.
    pub fn tune_out() -> Anim {
        Anim { kind: Kind::TuneOut, delay: 0.0, start: None }
    }
    pub fn burst() -> Anim {
        Anim { kind: Kind::Burst, delay: 0.0, start: None }
    }
    pub fn slam() -> Anim {
        Anim { kind: Kind::Slam, delay: 0.0, start: None }
    }
    pub fn roll(up: bool) -> Anim {
        Anim { kind: Kind::Roll { up }, delay: 0.0, start: None }
    }
    pub fn tick() -> Anim {
        Anim { kind: Kind::Tick, delay: 0.0, start: None }
    }
}

// ---- the animation curves ---------------------------------------------------------------
fn keys(t: f32, k: &[(f32, f32)]) -> f32 {
    for w in k.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t <= t1 {
            return v0 + (v1 - v0) * ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
        }
    }
    k.last().unwrap().1
}

/// (x offset, y offset, alpha, split) at progress t (0..1), all offsets at 1080p.
fn curve(kind: Kind, t: f32) -> (f32, f32, f32, f32) {
    match kind {
        Kind::TuneIn => {
            let e = 1.0 - (1.0 - t).powf(3.5);
            (keys(e, &[(0.0, -10.0), (0.35, 3.0), (0.65, -1.0), (1.0, 0.0)]), 0.0, keys(e, &[(0.0, 0.0), (0.35, 1.0)]), keys(e, &[(0.0, SPLIT_IN), (0.35, 3.0), (0.65, 1.0), (1.0, 0.0)]))
        }
        Kind::TuneOut => {
            let e = t * t;
            (keys(e, &[(0.0, 0.0), (0.4, 2.0), (1.0, 10.0)]), 0.0, 1.0 - e, keys(e, &[(0.0, 0.0), (0.4, 3.0), (1.0, 6.0)]))
        }
        Kind::Burst => {
            let e = 1.0 - (1.0 - t).powf(2.5);
            (keys(e, &[(0.0, 2.0), (0.25, -2.0), (0.5, 1.0), (1.0, 0.0)]), 0.0, 1.0, keys(e, &[(0.0, 5.0), (0.25, 3.0), (0.5, 2.0), (1.0, 0.0)]))
        }
        Kind::Slam => {
            let e = 1.0 - (1.0 - t).powf(2.5);
            (keys(e, &[(0.0, -14.0), (0.15, 8.0), (0.3, -5.0), (0.5, 3.0), (1.0, 0.0)]), 0.0, 1.0, keys(e, &[(0.0, 12.0), (0.15, 8.0), (0.3, 6.0), (0.5, 3.0), (1.0, 0.0)]))
        }
        Kind::Roll { up } => {
            let e = 1.0 - (1.0 - t).powi(3);
            (0.0, (if up { 14.0 } else { -14.0 }) * (1.0 - e), e, 4.0 * (1.0 - e))
        }
        Kind::Tick => (0.0, 0.0, 1.0, 4.0 * (1.0 - t).powi(3)),
    }
}

fn duration(kind: Kind) -> f32 {
    match kind {
        Kind::TuneIn => T_TUNE,
        Kind::TuneOut => T_OUT,
        Kind::Burst => T_BURST,
        Kind::Slam => 0.52,
        Kind::Roll { .. } => T_ROLL,
        Kind::Tick => 0.42,
    }
}

/// The colours a part had before an animation touched it, restored when it ends.
#[derive(Component)]
pub(super) struct Base {
    text: Option<Color>,
    bg: Option<Color>,
    border: Option<BorderColor>,
    img: Option<Color>,
    had_shadow: bool,
}

type Parts<'a> = (Option<&'a mut TextColor>, Option<&'a mut BackgroundColor>, Option<&'a mut BorderColor>, Option<&'a mut ImageNode>, Option<&'a mut TextShadow>, Option<&'a Base>, Has<Fade>);

pub(super) fn anim(
    time: Res<Time>,
    mut commands: Commands,
    window: Query<&Window, With<PrimaryWindow>>,
    mut roots: Query<(Entity, &mut Anim, &mut UiTransform)>,
    children: Query<&Children>,
    mut parts: Query<Parts>,
) {
    let now = time.elapsed_secs();
    let px = window.single().map(|w| w.height() / 1080.0).unwrap_or(1.0);
    // Animations nest (a page tunes in while each of its rows does): they share one Base per
    // node, so only the last one still covering a node restores it and lets the Base go.
    let subtree = |root: Entity| {
        let mut all = Vec::new();
        let mut stack = vec![root];
        while let Some(e) = stack.pop() {
            if let Ok(kids) = children.get(e) {
                stack.extend(kids.iter());
            }
            all.push(e);
        }
        all
    };
    // Bases made this frame (the insert lands at the end of the frame): a second animation
    // reaching the same node this frame must use the colours from before the first one
    let mut fresh: HashMap<Entity, Base> = HashMap::new();
    let mut cover: HashMap<Entity, u32> = HashMap::new();
    for (root, ..) in &roots {
        for e in subtree(root) {
            *cover.entry(e).or_default() += 1;
        }
    }
    for (root, mut a, mut tr) in &mut roots {
        let delay = a.delay;
        let start = *a.start.get_or_insert(now + delay);
        let t = ((now - start) / duration(a.kind)).clamp(0.0, 1.0);
        let done = t >= 1.0 && now >= start;
        let (x, y, alpha, split) = if now < start { curve(a.kind, 0.0) } else { curve(a.kind, t) };
        tr.translation = if done { Val2::ZERO } else { Val2::new(vh(x), vh(y)) };
        if a.kind == Kind::Tick {
            tr.scale = Vec2::splat(if done { 1.0 } else { 1.0 + 0.14 * (1.0 - t).powi(3) });
        }
        let touches_alpha = !matches!(a.kind, Kind::Burst | Kind::Slam | Kind::Tick);
        // walk the subtree
        let mut stack = vec![root];
        while let Some(e) = stack.pop() {
            if let Ok(kids) = children.get(e) {
                stack.extend(kids.iter());
            }
            let Ok((text, bg, border, img, shadow, base, faded)) = parts.get_mut(e) else { continue };
            let b = match (base, fresh.get(&e)) {
                (Some(b), _) | (None, Some(b)) => Base { text: b.text, bg: b.bg, border: b.border, img: b.img, had_shadow: b.had_shadow },
                (None, None) => {
                    let b = Base { text: text.as_ref().map(|c| c.0), bg: bg.as_ref().map(|c| c.0), border: border.as_deref().copied(), img: img.as_ref().map(|i| i.color), had_shadow: shadow.is_some() };
                    commands.entity(e).insert(Base { ..b });
                    fresh.insert(e, Base { ..b });
                    b
                }
            };
            if done && cover.get(&e).copied().unwrap_or(1) > 1 {
                // another animation still runs here: it restores the node when it ends
                continue;
            }
            if done {
                if let (Some(mut c), Some(v)) = (text, b.text) {
                    c.0 = v;
                }
                if touches_alpha {
                    if let (Some(mut c), Some(v), false) = (bg, b.bg, faded) {
                        c.0 = v;
                    }
                    if let (Some(mut c), Some(v)) = (border, b.border) {
                        *c = v;
                    }
                    if let (Some(mut c), Some(v)) = (img, b.img) {
                        c.color = v;
                    }
                }
                if !b.had_shadow {
                    commands.entity(e).remove::<TextShadow>();
                }
                commands.entity(e).remove::<Base>();
                continue;
            }
            let k = (split / SPLIT_IN).clamp(0.0, 1.0);
            if let (Some(mut c), Some(v)) = (text, b.text) {
                let tinted = v.mix(&CH_A, k * 0.6);
                c.0 = if touches_alpha { tinted.with_alpha(v.alpha() * alpha) } else { tinted };
                if !b.had_shadow {
                    let s = TextShadow { offset: Vec2::new(split * px, 0.0), color: CH_B.with_alpha(0.9 * alpha * k.min(1.0).max(if split > 0.3 { 0.4 } else { 0.0 })) };
                    match shadow {
                        Some(mut sh) => *sh = s,
                        None => {
                            commands.entity(e).insert(s);
                        }
                    }
                }
            }
            if touches_alpha {
                if let (Some(mut c), Some(v), false) = (bg, b.bg, faded) {
                    c.0 = v.with_alpha(v.alpha() * alpha);
                }
                if let (Some(mut c), Some(v)) = (border, b.border) {
                    *c = BorderColor { top: v.top.with_alpha(v.top.alpha() * alpha), right: v.right.with_alpha(v.right.alpha() * alpha), bottom: v.bottom.with_alpha(v.bottom.alpha() * alpha), left: v.left.with_alpha(v.left.alpha() * alpha) };
                }
                if let (Some(mut c), Some(v)) = (img, b.img) {
                    c.color = v.with_alpha(v.alpha() * alpha);
                }
            }
        }
        if done {
            for e in subtree(root) {
                if let Some(n) = cover.get_mut(&e) {
                    *n -= 1;
                }
            }
            if a.kind == Kind::TuneOut {
                commands.entity(root).despawn();
            } else {
                commands.entity(root).remove::<Anim>();
            }
        }
    }
}

