//! The uplink: the assistant on screen in matches, bottom centre, built symmetric around the
//! centre line (design/mockups/ui: `ui.uplink`, ai.js, the AI assistant tab).
//! A HUD panel like the others; the assistant's live portrait sits centred in its top edge;
//! the edge is a light in the assistant's colour with a waveform running out to each side;
//! the name and the line are centred under the portrait.
//!
//! Timeline (seconds from the line):
//! - 0: a chirp; the edge light draws out from the centre; the panel tunes in, a scanline
//!   sweeps it; the portrait tunes in (Grumpk slams in) and four brackets lock onto it.
//! - 0.42: the line resolves from the centre outward, two scan edges running to its ends.
//!   While it talks its voice drives everything: the waveforms carry it out from the
//!   portrait, the edge light and the portrait's glow breathe with it, the mouth moves.
//! - Twins: a blip; the portrait glitches over to him (his own mood), her line dims, the name
//!   says him, his line opens under hers and resolves.
//! - After the last word a timer line on the bottom edge shrinks to the centre for 4 s; then
//!   a lower chirp, the light retracts and it all tunes out.
use super::view::{face_for, portrait, Faces, Rig, BROTHER_FACE};
use super::{lines::Mood, Brain, ASSISTANTS};
use crate::menu::AppState;
use crate::ui::{edge, hud_panel, kit_text, vh, Anim, Covered, Edge, EdgeColor, Kit, UiSound, SAFE, TX1, TX2};
use bevy::prelude::*;

/// When the line starts resolving, how long a reveal takes, the Twins' hand-over pause, and
/// how long a finished line stays.
const OPEN: f32 = 0.42;
const REVEAL: f32 = 0.56;
const HAND: f32 = 0.26;
const HOLD: f32 = 4.0;
/// Each assistant's chirp pitch (Hz).
const PITCH: [f32; 4] = [660.0, 990.0, 784.0, 440.0];
/// Portrait size and the ring around it (gap + line).
const FACE: f32 = 88.0;
const RING: f32 = 4.0;
/// Waveform bars per side.
const BARS: usize = 10;

/// About as long as reading the line aloud.
fn talk_time(s: &str) -> f32 {
    (0.6 + s.chars().count() as f32 * 0.038).min(3.2)
}

/// (her or the only line, his line)
fn parts(text: &str) -> (String, String) {
    let mut it = text.splitn(2, '\n');
    (it.next().unwrap_or("").to_string(), it.next().unwrap_or("").to_string())
}

/// The voice as loudness over time (one value per 20 ms): vowels peak, consonants are lower,
/// spaces and punctuation dip, eased so it moves like speech rather than flickers.
fn envelope(s: &str, secs: f32) -> Vec<f32> {
    let chars: Vec<char> = s.chars().collect();
    let n = ((secs / 0.02).round() as usize).max(1);
    let mut v = 0.0f32;
    (0..n)
        .map(|i| {
            let c = chars.get(i * chars.len() / n).copied().unwrap_or(' ');
            let target = if "aeiouyAEIOUY".contains(c) {
                0.85 + fastrand::f32() * 0.15
            } else if c.is_alphanumeric() {
                0.45 + fastrand::f32() * 0.25
            } else if ".,!?:".contains(c) {
                0.0
            } else {
                0.12
            };
            v += (target - v) * 0.35;
            v
        })
        .collect()
}

/// The schedule of one line.
struct Plan {
    /// When each row starts resolving and stops talking.
    rows: Vec<(f32, f32)>,
    /// The voice per row.
    voice: Vec<Vec<f32>>,
    /// Last word, and when it leaves.
    end: f32,
    close: f32,
}

fn plan(text: &str) -> Plan {
    let (a, b) = parts(text);
    let mut rows = vec![(OPEN, OPEN + talk_time(&a))];
    let mut voice = vec![envelope(&a, talk_time(&a))];
    if !b.is_empty() {
        let start = rows[0].1 + HAND;
        rows.push((start, start + talk_time(&b)));
        voice.push(envelope(&b, talk_time(&b)));
    }
    let end = rows.last().unwrap().1;
    Plan { rows, voice, end, close: end + HOLD }
}

/// The uplink on screen and its line's schedule.
#[derive(Component)]
pub struct Uplink {
    who: usize,
    mood2: Mood,
    at: f32,
    plan: Plan,
    /// Hand-over done, closing chirp sent.
    handed: bool,
    closed: bool,
    /// Seconds since the line, the talking row and its voice sample, the smoothed loudness.
    t: f32,
    talking: Option<(usize, usize)>,
    level: f32,
}

#[derive(Component)]
pub(super) struct UpEdge;
#[derive(Component)]
pub(super) struct UpSweep;
#[derive(Component)]
pub(super) struct UpTimer;
#[derive(Component)]
pub(super) struct UpRing;
/// Which face a portrait is (the Twins: 0 her, 1 him); only the one talking moves.
#[derive(Component)]
pub(super) struct Speaker(usize);
/// Space between the Twins' two portraits.
const TWIN_GAP: f32 = 12.0;
#[derive(Component)]
pub(super) struct UpName;
/// A waveform bar: how far from the portrait.
#[derive(Component)]
pub(super) struct Wave(usize);
/// A corner bracket: which corner (x, y as -1 / 1).
#[derive(Component)]
pub(super) struct Lock(f32, f32);
/// A line row: which, the box that holds it (sized by an invisible copy), the window that
/// reveals it, the shown copy, the two scan edges.
#[derive(Component)]
pub(super) struct LineRow(usize);
#[derive(Component)]
pub(super) struct LineWindow(usize);
#[derive(Component)]
pub(super) struct LineText(usize);
#[derive(Component)]
pub(super) struct Scan(usize, bool);

fn full() -> Node {
    Node { position_type: PositionType::Absolute, ..default() }
}

fn spawn(commands: &mut Commands, faces: &Faces, kit: &Kit, who: usize, mood: Mood, mood2: Mood, text: &str, now: f32) {
    let a = &ASSISTANTS[who];
    let c = a.color;
    let (her, his) = parts(text);
    // the Twins always show both faces, side by side
    let twins = who == 2;
    let name = if twins { format!("{} · HER", a.name.to_uppercase()) } else { a.name.to_uppercase() };
    let line_font = 20.0;
    commands
        .spawn((
            Uplink { who, mood2, at: now, plan: plan(text), handed: false, closed: false, t: 0.0, talking: None, level: 0.0 },
            DespawnOnExit(AppState::InGame),
            GlobalZIndex(6),
            Node { position_type: PositionType::Absolute, left: px(0), right: px(0), bottom: vh(SAFE), justify_content: JustifyContent::Center, ..default() },
            Pickable::IGNORE,
        ))
        .with_children(|root| {
            root.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, padding: UiRect::top(vh(FACE / 2.0 + RING)), ..default() }).with_children(|up| {
                // the panel
                up.spawn((Anim::tune_in(0.1), hud_panel(Node { min_width: vh(560.0), max_width: vh(820.0), padding: UiRect { left: vh(48.0), right: vh(48.0), top: vh(FACE / 2.0 + RING + 16.0), bottom: vh(20.0) }, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() })))
                    .with_children(|band| {
                        // the assistant's colour glowing down from the edge
                        band.spawn((Node { position_type: PositionType::Absolute, left: px(0), right: px(0), top: px(0), bottom: px(0), border_radius: BorderRadius::all(vh(crate::ui::RADIUS)), ..default() }, BackgroundGradient::from(RadialGradient::new(UiPosition::TOP, RadialGradientShape::FarthestSide, vec![ColorStop::percent(c.with_alpha(0.12), 0.0), ColorStop::percent(c.with_alpha(0.0), 100.0)])), Pickable::IGNORE));
                        band.spawn((UpEdge, edge(EdgeColor::Fixed(c), 0.7), UiTransform { scale: Vec2::new(0.0, 1.0), ..default() }));
                        band.spawn((UpSweep, Node { left: px(0), right: px(0), top: percent(0), height: vh(2.0), ..full() }, BackgroundGradient::from(LinearGradient::to_right(vec![ColorStop::percent(TX1.with_alpha(0.0), 0.0), ColorStop::percent(TX1.with_alpha(0.4), 50.0), ColorStop::percent(TX1.with_alpha(0.0), 100.0)])), Visibility::Hidden));
                        band.spawn((UpTimer, Node { left: vh(24.0), right: vh(24.0), bottom: px(0), height: vh(2.0), border_radius: BorderRadius::all(px(1)), ..full() }, BackgroundColor(c.with_alpha(0.7)), UiTransform { scale: Vec2::new(0.0, 1.0), ..default() }));
                        // the waveforms, out from the portrait along the edge
                        for right in [false, true] {
                            let side = Node { top: vh(-8.0), height: vh(16.0), align_items: AlignItems::Center, column_gap: vh(4.0), flex_direction: if right { FlexDirection::Row } else { FlexDirection::RowReverse }, ..full() };
                            let reach = if twins { FACE + 2.0 * RING + TWIN_GAP / 2.0 + 16.0 } else { FACE / 2.0 + RING + 16.0 };
                            let side = if right { Node { left: percent(50), margin: UiRect::left(vh(reach)), ..side } } else { Node { right: percent(50), margin: UiRect::right(vh(reach)), ..side } };
                            band.spawn(side).with_children(|w| {
                                for k in 0..BARS {
                                    w.spawn((Wave(k), Node { width: vh(3.0), height: vh(3.0), border_radius: BorderRadius::all(px(2)), ..default() }, BackgroundColor(c.with_alpha(rest_alpha(k)))));
                                }
                            });
                        }
                        band.spawn((UpName, kit_text(name, 13.0, c, &kit.text)));
                        for (i, line) in [her, his].into_iter().enumerate().filter(|(_, l)| !l.is_empty()) {
                            let layout = TextLayout::justify(Justify::Center);
                            band.spawn((LineRow(i), Node { margin: UiRect::top(vh(8.0)), display: if i == 0 { Display::Flex } else { Display::None }, ..default() })).with_children(|row| {
                                // the invisible copy gives the row its size
                                row.spawn((kit_text(line.clone(), line_font, TX1, &kit.text), layout, Visibility::Hidden));
                                row.spawn((LineWindow(i), Node { top: px(0), bottom: px(0), left: percent(50), right: percent(50), overflow: Overflow::clip(), ..full() })).with_children(|win| {
                                    win.spawn((LineText(i), Node { top: px(0), ..full() }, kit_text(line, line_font, TX1, &kit.text), layout));
                                });
                                for right in [false, true] {
                                    row.spawn((Scan(i, right), Node { top: px(0), bottom: px(0), width: vh(2.0), left: percent(50), border_radius: BorderRadius::all(px(1)), ..full() }, BackgroundColor(c), BoxShadow(vec![ShadowStyle { color: c.with_alpha(0.8), x_offset: px(0), y_offset: px(0), spread_radius: px(0), blur_radius: vh(10.0) }]), Visibility::Hidden));
                                }
                            });
                        }
                    });
                // the portrait in its ring, centred on the edge (the Twins: her left, him right)
                let faces_shown: Vec<(usize, Mood, f32)> = if twins {
                    vec![(face_for(who), mood, -(FACE + 2.0 * RING + TWIN_GAP / 2.0)), (BROTHER_FACE, mood2, TWIN_GAP / 2.0)]
                } else {
                    vec![(face_for(who), mood, -(FACE / 2.0 + RING))]
                };
                for (k, (face, m, offset)) in faces_shown.into_iter().enumerate() {
                    up.spawn((
                        UpRing,
                        Speaker(k),
                        if who == 3 { Anim::slam() } else { Anim::tune_in(0.16 + 0.06 * k as f32) },
                        Node { position_type: PositionType::Absolute, top: px(0), left: percent(50), margin: UiRect::left(vh(offset)), padding: UiRect::all(vh(RING - 1.0)), border: UiRect::all(px(1)), border_radius: BorderRadius::all(vh(8.0 + RING)), ..default() },
                        BackgroundColor(Color::srgba(0.027, 0.035, 0.047, 0.9)),
                        BorderColor::all(c.with_alpha(0.7)),
                        BoxShadow(vec![glow(c, 0.0)]),
                    ))
                    .with_children(|ring| {
                        let tile = portrait(ring, faces, face, m, c, FACE);
                        ring.commands().entity(tile).insert(Speaker(k));
                        for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                            let b = px(2);
                            let border = UiRect { left: if x < 0.0 { b } else { px(0) }, right: if x > 0.0 { b } else { px(0) }, top: if y < 0.0 { b } else { px(0) }, bottom: if y > 0.0 { b } else { px(0) } };
                            ring.spawn((Lock(x, y), Node { width: vh(12.0), height: vh(12.0), border, ..full() }, BorderColor::all(c), Visibility::Hidden));
                        }
                    });
                }
            });
        });
}

/// Waveform bars fade out toward the ends at rest.
fn rest_alpha(k: usize) -> f32 {
    match k {
        0..4 => 0.5,
        4..7 => 0.3,
        _ => 0.15,
    }
}

/// The portrait's glow, brighter with the voice.
fn glow(c: Color, level: f32) -> ShadowStyle {
    ShadowStyle { color: c.with_alpha(0.35 + 0.35 * level), x_offset: px(0), y_offset: px(0), spread_radius: px(0), blur_radius: vh(20.0 + 22.0 * level) }
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(3)
}

fn ease_in_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

/// A new line: the old uplink tunes out, the new one opens.
pub(super) fn draw(mut commands: Commands, brain: Res<Brain>, faces: Res<Faces>, kit: Res<Kit>, time: Res<Time>, ups: Query<Entity, With<Uplink>>, mut shown: Local<u32>, mut sounds: MessageWriter<UiSound>) {
    let Some(said) = brain.said.as_ref() else { return };
    if *shown == brain.version {
        return;
    }
    *shown = brain.version;
    for e in &ups {
        commands.entity(e).remove::<Uplink>().insert(Anim::tune_out());
    }
    spawn(&mut commands, &faces, &kit, said.who, said.mood, said.mood2, &said.text, time.elapsed_secs());
    sounds.write(UiSound::Open(PITCH[said.who.min(3)]));
}

/// Per frame, the timeline above: who talks and how loud, the Twins' hand-over, the chirps,
/// leaving. `light` and `reveal` draw from what this works out.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn animate(
    time: Res<Time>,
    covered: Res<Covered>,
    overlay: Option<Res<crate::menu::Overlay>>,
    mut ups: Query<(Entity, &mut Uplink, &mut Visibility)>,
    mut rigs: Query<(&mut Rig, &Speaker)>,
    mut name: Query<&mut Text, With<UpName>>,
    mut rows: Query<(&LineRow, &mut Node)>,
    mut texts: Query<(&LineText, &mut TextColor)>,
    mut sounds: MessageWriter<UiSound>,
    mut commands: Commands,
) {
    // under the scoreboard and the Esc menu it hides; over the buy menu it stays
    let hide = covered.0 && !overlay.is_some_and(|o| *o == crate::menu::Overlay::Buy);
    // ponytail: one uplink at a time (a new line replaces it), so its parts are queried flat
    let Some((root, up, mut vis)) = ups.iter_mut().next() else { return };
    let up = up.into_inner();
    vis.set_if_neq(if hide { Visibility::Hidden } else { Visibility::Inherited });
    let t = time.elapsed_secs() - up.at;
    up.t = t;
    // which row talks, and how loud
    up.talking = up.plan.rows.iter().position(|(s, e)| t >= *s && t < *e).map(|i| (i, ((t - up.plan.rows[i].0) / 0.02) as usize));
    let loud = up.talking.map(|(i, at)| up.plan.voice[i].get(at).copied().unwrap_or(0.0)).unwrap_or(0.0);
    up.level += (loud - up.level) * 0.25;
    // only the one talking moves (one face: row 0 always)
    for (mut r, Speaker(k)) in &mut rigs {
        r.talking = up.talking.is_some_and(|(i, _)| i == *k || up.who != 2);
    }
    // the Twins hand over
    if up.plan.rows.len() > 1 && !up.handed && t >= up.plan.rows[0].1 {
        up.handed = true;
        sounds.write(UiSound::Hand(PITCH[2]));
        for mut n in &mut name {
            n.0 = format!("{} · HIM", ASSISTANTS[2].name.to_uppercase());
        }
        for (LineRow(i), mut n) in &mut rows {
            if *i == 1 {
                n.display = Display::Flex;
            }
        }
        for (LineText(i), mut tc) in &mut texts {
            if *i == 0 {
                tc.0 = TX2;
            }
        }
    }
    if t >= up.plan.close && !up.closed {
        up.closed = true;
        sounds.write(UiSound::Close(PITCH[up.who.min(3)]));
    }
    if t >= up.plan.close + 0.24 {
        commands.entity(root).remove::<Uplink>().insert(Anim::tune_out());
    }
}

/// The light: the edge draws out, breathes with the voice and retracts; the portrait's glow;
/// the sweep; the brackets locking on; the waveforms; the hold timer.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn light(
    ups: Query<&Uplink>,
    mut edge: Query<(&mut Edge, &mut UiTransform), (With<UpEdge>, Without<UpTimer>)>,
    mut timer: Query<&mut UiTransform, (With<UpTimer>, Without<UpEdge>)>,
    mut sweep: Query<(&mut Node, &mut Visibility), (With<UpSweep>, Without<Lock>, Without<Wave>)>,
    mut locks: Query<(&Lock, &mut Node, &mut Visibility, &mut BorderColor), (Without<UpSweep>, Without<Wave>)>,
    mut ring: Query<&mut BoxShadow, With<UpRing>>,
    mut waves: Query<(&Wave, &mut Node, &mut BackgroundColor), (Without<UpSweep>, Without<Lock>)>,
) {
    let Some(up) = ups.iter().next() else { return };
    let (t, level) = (up.t, up.level);
    let c = ASSISTANTS[up.who.min(3)].color;
    let closing = t >= up.plan.close;
    for (mut e, mut tr) in &mut edge {
        e.rest = 0.7 + 0.3 * level;
        tr.scale.x = if closing { 1.0 - ease_out((t - up.plan.close) / 0.24) } else { ease_out(t / 0.34) };
    }
    for mut s in &mut ring {
        s.0 = vec![glow(c, level)];
    }
    for (mut n, mut v) in &mut sweep {
        let p = (t - 0.14) / 0.42;
        v.set_if_neq(if (0.0..1.0).contains(&p) { Visibility::Inherited } else { Visibility::Hidden });
        n.top = percent(100.0 * ease_out(p));
    }
    // the brackets: in from 22 out to 8 out, then fade
    for (Lock(x, y), mut n, mut v, mut b) in &mut locks {
        let p = (t - 0.18) / 0.64;
        v.set_if_neq(if (0.0..1.0).contains(&p) { Visibility::Inherited } else { Visibility::Hidden });
        let out = 22.0 - 14.0 * ease_out(p / 0.55);
        let alpha = if p < 0.3 { p / 0.3 } else if p < 0.55 { 1.0 } else { 1.0 - (p - 0.55) / 0.45 };
        let at = vh(-RING - out);
        if *x < 0.0 {
            n.left = at
        } else {
            n.right = at
        }
        if *y < 0.0 {
            n.top = at
        } else {
            n.bottom = at
        }
        *b = BorderColor::all(c.with_alpha(alpha.clamp(0.0, 1.0)));
    }
    // each bar is the voice a little earlier, quieter toward the ends
    for (Wave(k), mut n, mut bg) in &mut waves {
        let h = match up.talking {
            Some((i, at)) => 3.0 + up.plan.voice[i].get(at.saturating_sub(*k * 2)).copied().unwrap_or(0.0) * 15.0 * (1.0 - *k as f32 / (BARS as f32 + 2.0)),
            None => 3.0,
        };
        n.height = vh(h);
        bg.0 = c.with_alpha(if up.talking.is_some() { 1.0 } else { rest_alpha(*k) });
    }
    for mut tr in &mut timer {
        tr.scale.x = if t >= up.plan.end && !closing { 1.0 - (t - up.plan.end) / HOLD } else { 0.0 };
    }
}

/// Each line resolves from the centre outward: a window opens on the shown copy (which stays
/// put), two scan edges running out to its ends.
#[allow(clippy::type_complexity)]
pub(super) fn reveal(
    ups: Query<&Uplink>,
    rows: Query<(&LineRow, &ComputedNode)>,
    mut windows: Query<(&LineWindow, &mut Node), (Without<LineText>, Without<Scan>)>,
    mut texts: Query<(&LineText, &mut Node), (Without<LineWindow>, Without<Scan>)>,
    mut scans: Query<(&Scan, &mut Node, &mut Visibility), (Without<LineWindow>, Without<LineText>)>,
) {
    let Some(up) = ups.iter().next() else { return };
    let start = |i: usize| up.plan.rows[i.min(up.plan.rows.len() - 1)].0;
    let mut widths = [0.0f32; 2];
    for (LineRow(i), cn) in &rows {
        widths[(*i).min(1)] = cn.size.x * cn.inverse_scale_factor;
    }
    for (LineWindow(i), mut n) in &mut windows {
        let p = ease_in_out((up.t - start(*i)) / REVEAL);
        let side = percent(50.0 * (1.0 - p));
        n.left = side;
        n.right = side;
        let w = widths[(*i).min(1)];
        for (LineText(j), mut tn) in &mut texts {
            if j == i {
                tn.left = px(-w * 0.5 * (1.0 - p));
                tn.width = px(w);
            }
        }
    }
    for (Scan(i, right), mut n, mut v) in &mut scans {
        let p = (up.t - start(*i)) / REVEAL;
        v.set_if_neq(if (0.0..0.95).contains(&p) { Visibility::Inherited } else { Visibility::Hidden });
        n.left = percent(50.0 + if *right { 50.0 } else { -50.0 } * ease_in_out(p));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_holds_after_the_last_word() {
        let p = plan("Two down at B.");
        assert_eq!(p.rows.len(), 1);
        assert!((p.close - p.end - HOLD).abs() < 1e-5);
        assert_eq!(p.voice[0].len(), (talk_time("Two down at B.") / 0.02).round() as usize);
    }

    #[test]
    fn twins_talk_one_after_the_other() {
        let p = plan("Ugh, fine.\nShe means save.");
        assert_eq!(p.rows.len(), 2);
        assert!(p.rows[1].0 >= p.rows[0].1 + HAND - 1e-5);
        assert!((p.end - p.rows[1].1).abs() < 1e-5);
    }

    #[test]
    fn the_voice_dips_at_punctuation() {
        let v = envelope("a. a. a.", 1.0);
        assert!(v.iter().all(|x| (0.0..=1.0).contains(x)));
        assert!(v.iter().cloned().fold(1.0, f32::min) < 0.3);
    }
}
