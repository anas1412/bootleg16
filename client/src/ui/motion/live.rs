//! Live values on the HUD (design/mockups/ui: hud.js, components.css "the HUD, alive"):
//! - `Edge`: the light on a HUD panel's top edge, in your side's colour (or a fixed one).
//!   Quiet at rest; `flare()` lights it full and it settles; `alert` turns it red; `beat`
//!   makes it pulse like a heartbeat.
//! - `Meter`: a bar whose fill drops at once while a light trail shows what was lost and
//!   drains after it (going up, the trail jumps with it).
//! - `Count`: a number that counts to its new value instead of jumping.
//! - `Tint`: HUD glass takes a faint tint of your side; your own rows a faint amber.
//! `Side` says which side you're on; the game sets it, everything here follows.
use super::ease;
use crate::ui::kit::vh;
use crate::ui::tokens::*;
use bevy::prelude::*;

/// Your side's colour (Bureau cyan, Strays pink), or None when you're on neither.
#[derive(Resource, Default)]
pub struct Side(pub Option<Color>);

/// The glass tint a HUD surface takes.
#[derive(Component, Clone, Copy, PartialEq)]
pub enum Tint {
    /// A faint tint of your side.
    Team,
    /// Yours (your kill, your voice): a faint amber.
    You,
}

fn tint(side: Res<Side>, mut q: Query<(&Tint, &mut BackgroundColor)>) {
    for (t, mut bg) in &mut q {
        let want = match (t, side.0) {
            (Tint::You, _) => HUD_BG.mix(&AMBER, 0.10),
            (Tint::Team, Some(c)) => HUD_BG.mix(&c, 0.07),
            (Tint::Team, None) => HUD_BG,
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

// ---- the edge light -------------------------------------------------------------------------
/// What colour an edge light is.
#[derive(Clone, Copy, PartialEq)]
pub enum EdgeColor {
    Team,
    Fixed(Color),
}

/// How bright an edge rests (0..1).
pub const EDGE_REST: f32 = 0.35;
/// How long a flare takes to settle.
const T_FLARE: f32 = 0.6;

#[derive(Component)]
pub struct Edge {
    pub color: EdgeColor,
    pub rest: f32,
    /// Red: something needs you (a hit, low health, the last seconds).
    pub alert: bool,
    /// Pulse like a heartbeat (low health).
    pub beat: bool,
    level: f32,
    drawn: Option<(Color, u8)>,
}

impl Edge {
    pub fn flare(&mut self) {
        self.level = 1.0;
    }
}

/// The edge light, as a child of the panel it lights.
pub fn edge(color: EdgeColor, rest: f32) -> impl Bundle {
    (
        Edge { color, rest, alert: false, beat: false, level: rest, drawn: None },
        Node { position_type: PositionType::Absolute, left: vh(12.0), right: vh(12.0), top: px(0), height: vh(2.0), border_radius: BorderRadius::all(px(1)), ..default() },
        BackgroundGradient::default(),
        ZIndex(1),
        Pickable::IGNORE,
    )
}

/// The light fading out at both ends, at `alpha`.
fn edge_gradient(c: Color, alpha: f32) -> BackgroundGradient {
    let on = c.with_alpha(alpha);
    let off = c.with_alpha(0.0);
    BackgroundGradient::from(LinearGradient::to_right(vec![ColorStop::percent(off, 0.0), ColorStop::percent(on, 25.0), ColorStop::percent(on, 75.0), ColorStop::percent(off, 100.0)]))
}

/// A heartbeat: two quick pulses, then rest (1.1 s).
fn heartbeat(t: f32) -> f32 {
    let p = (t / 1.1).fract();
    let keys = [(0.0, 0.0), (0.08, 1.0), (0.18, 0.15), (0.28, 0.85), (0.6, 0.0), (1.0, 0.0)];
    for w in keys.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if p <= t1 {
            return v0 + (v1 - v0) * (p - t0) / (t1 - t0);
        }
    }
    0.0
}

fn edges(time: Res<Time>, side: Res<Side>, mut q: Query<(&mut Edge, &mut BackgroundGradient)>) {
    let k = ease(time.delta_secs(), T_FLARE / 3.0);
    let now = time.elapsed_secs();
    for (mut e, mut g) in &mut q {
        let rest = e.rest;
        e.level += (rest - e.level) * k;
        let level = if e.beat { e.level.max(rest + (1.0 - rest) * heartbeat(now)) } else { e.level };
        let c = if e.alert {
            BAD
        } else {
            match e.color {
                EdgeColor::Fixed(c) => c,
                EdgeColor::Team => side.0.unwrap_or(LINE_3),
            }
        };
        // only rebuild the gradient when it visibly changed
        let key = (c, (level * 64.0) as u8);
        if e.drawn != Some(key) {
            e.drawn = Some(key);
            *g = edge_gradient(c, level);
        }
    }
}

// ---- meters ---------------------------------------------------------------------------------
/// How long the trail waits before it drains, and how fast it drains.
const TRAIL_HOLD: f32 = 0.28;
const T_TRAIL: f32 = 0.42;

#[derive(Component)]
pub struct Meter {
    /// 0..1
    pub value: f32,
    pub color: Color,
    shown: f32,
    trail: f32,
    drain_at: f32,
    fill: Entity,
    trail_node: Entity,
}

/// A meter `width` wide (1080p px) in `color`, with its trail; returns the meter.
pub fn meter(p: &mut ChildSpawnerCommands, width: f32, color: Color, value: f32) -> Entity {
    let bar = |w: f32| Node { position_type: PositionType::Absolute, left: px(0), top: px(0), bottom: px(0), width: percent(w * 100.0), border_radius: BorderRadius::all(px(2)), ..default() };
    let mut track = p.spawn((Node { width: vh(width), height: vh(4.0), margin: UiRect::top(vh(10.0)), border_radius: BorderRadius::all(px(2)), overflow: Overflow::clip(), ..default() }, BackgroundColor(LINE_2)));
    let (mut fill, mut trail) = (Entity::PLACEHOLDER, Entity::PLACEHOLDER);
    track.with_children(|t| {
        trail = t.spawn((bar(value), BackgroundColor(TX1.with_alpha(0.5)))).id();
        fill = t.spawn((bar(value), BackgroundColor(color))).id();
    });
    track.insert(Meter { value, color, shown: value, trail: value, drain_at: 0.0, fill, trail_node: trail });
    track.id()
}

fn meters(time: Res<Time>, mut q: Query<&mut Meter>, mut nodes: Query<(&mut Node, &mut BackgroundColor)>) {
    let now = time.elapsed_secs();
    let dt = time.delta_secs();
    for mut m in &mut q {
        let v = m.value.clamp(0.0, 1.0);
        m.shown += (v - m.shown) * ease(dt, 0.04);
        if v >= m.trail {
            m.trail = v;
            m.drain_at = now + TRAIL_HOLD;
        } else if now >= m.drain_at {
            m.trail += (v - m.trail) * ease(dt, T_TRAIL / 3.0);
        }
        // settle exactly, so a meter at rest stops touching its nodes (a changed Node is laid out again)
        if (v - m.shown).abs() < 1e-3 {
            m.shown = v;
        }
        if (v - m.trail).abs() < 1e-3 {
            m.trail = v;
        }
        let (shown, trail, color) = (m.shown, m.trail, m.color);
        if let Ok((mut n, mut bg)) = nodes.get_mut(m.fill) {
            let w = percent(shown * 100.0);
            if n.width != w {
                n.width = w;
            }
            bg.set_if_neq(BackgroundColor(color));
        }
        if let Ok((mut n, _)) = nodes.get_mut(m.trail_node) {
            let w = percent(trail * 100.0);
            if n.width != w {
                n.width = w;
            }
        }
    }
}

// ---- numbers ----------------------------------------------------------------------------------
/// A whole number that counts to `target` (about `tau` seconds per third of the way).
#[derive(Component)]
pub struct Count {
    pub target: f32,
    pub tau: f32,
    shown: f32,
}

impl Count {
    pub fn new(value: f32, tau: f32) -> Count {
        Count { target: value, tau, shown: value }
    }
}

fn counts(time: Res<Time>, mut q: Query<(&mut Count, &mut Text)>) {
    let dt = time.delta_secs();
    for (mut c, mut t) in &mut q {
        let target = c.target;
        c.shown += (target - c.shown) * ease(dt, c.tau);
        if (target - c.shown).abs() < 0.5 {
            c.shown = target;
        }
        let s = format!("{:.0}", c.shown);
        if t.0 != s {
            t.0 = s;
        }
    }
}

// ---- opening in place ---------------------------------------------------------------------
/// A part that opens and closes in place (status icons): its width eases between 0 and
/// `size` (1080p px) and it clips what's inside, so its neighbours slide over smoothly.
#[derive(Component)]
pub struct Opens {
    pub open: bool,
    pub size: f32,
    shown: f32,
}

impl Opens {
    pub fn new(open: bool, size: f32) -> Opens {
        Opens { open, size, shown: if open { 1.0 } else { 0.0 } }
    }
}

fn opens(time: Res<Time>, mut q: Query<(&mut Opens, &mut Node)>) {
    let k = ease(time.delta_secs(), T_SLIDE / 3.0);
    for (mut o, mut n) in &mut q {
        let target = if o.open { 1.0 } else { 0.0 };
        o.shown += (target - o.shown) * k;
        if (target - o.shown).abs() < 0.01 {
            o.shown = target;
        }
        let w = vh(o.size * o.shown);
        if n.width != w {
            n.width = w;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Side>().add_systems(Update, (tint, edges, meters, counts, opens));
}
