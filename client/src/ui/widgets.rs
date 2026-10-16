//! Widgets (design/mockups/ui/components.css). Buttons are framed, never filled solid. Every
//! pressable thing bursts when hovered and when pressed, and reports its click as a `UiAction`.
//! Lists and chip groups have one sliding selection.
use super::kit::{kit_text, t, vh, Kit};
use super::motion::{Anim, Fade, FadeBorder, Slide, UiMemory};
use super::tokens::*;
use super::{UiAction, UiActions};
use bevy::prelude::*;

/// Hover: ease to the hover wash and frame and burst once; leave: ease back.
pub fn hoverable(e: &mut EntityCommands, bg: (Color, Color), border: (Color, Color)) {
    e.insert((Fade(bg.0), FadeBorder(border.0)))
        .observe(move |ev: On<PointerOver>, mut q: Query<(&mut Fade, &mut FadeBorder)>, mut commands: Commands| {
            if let Ok((mut f, mut b)) = q.get_mut(ev.entity) {
                f.0 = bg.1;
                b.0 = border.1;
                commands.entity(ev.entity).insert(Anim::burst());
            }
        })
        .observe(move |ev: On<PointerOut>, mut q: Query<(&mut Fade, &mut FadeBorder)>| {
            if let Ok((mut f, mut b)) = q.get_mut(ev.entity) {
                f.0 = bg.0;
                b.0 = border.0;
            }
        });
}

/// Click → `UiAction`, with a burst.
pub fn pressable(e: &mut EntityCommands, action: UiAction) {
    e.observe(move |ev: On<PointerClick>, mut actions: ResMut<UiActions>, mut commands: Commands| {
        actions.0.push(action.clone());
        commands.entity(ev.entity).insert(Anim::burst());
    });
}

/// Hover and click for a card or row whose look is its own: only the hover wash changes.
pub fn clickable(e: &mut EntityCommands, action: UiAction, base: Color, _selected: bool) {
    let hover = if base == Color::NONE { HOVER } else { base.mix(&TX1, 0.04) };
    e.insert((Fade(base), BackgroundColor(base)))
        .observe(move |ev: On<PointerOver>, mut q: Query<&mut Fade>, mut commands: Commands| {
            if let Ok(mut f) = q.get_mut(ev.entity) {
                f.0 = hover;
                commands.entity(ev.entity).insert(Anim::burst());
            }
        })
        .observe(move |ev: On<PointerOut>, mut q: Query<&mut Fade>| {
            if let Ok(mut f) = q.get_mut(ev.entity) {
                f.0 = base;
            }
        });
    pressable(e, action);
}

/// Hover and click for a card that keeps its own frame colour.
pub fn clickable_keep_border(e: &mut EntityCommands, action: UiAction, base: Color) {
    clickable(e, action, base, false);
}

/// A hairline between list rows.
pub fn hairline(p: &mut ChildSpawnerCommands) {
    p.spawn((Node { height: px(1), ..default() }, BackgroundColor(LINE)));
}

/// One row of a numbered list.
pub struct NavItem<'a> {
    pub num: &'a str,
    pub label: &'a str,
    pub jp: &'a str,
    pub action: UiAction,
    pub selected: bool,
}

/// A numbered list (main menu, buy categories, Esc menu) with one sliding selection: an amber
/// wash with the amber bar. `key` names it so a rebuilt list slides on from where it was.
pub fn nav(p: &mut ChildSpawnerCommands, kit: &Kit, key: &str, items: Vec<NavItem>, compact: bool) {
    p.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|col| {
        let ind = col.spawn((Node { position_type: PositionType::Absolute, border: UiRect::left(vh(BAR)), border_radius: BorderRadius::all(vh(RADIUS_S)), ..default() }, BackgroundColor(AMBER_WASH), BorderColor::all(AMBER), Pickable::IGNORE)).id();
        let mut chosen = None;
        for it in items {
            hairline(col);
            let color = if it.selected { AMBER } else { TX1 };
            let node = Node { height: vh(if compact { 64.0 } else { 76.0 }), align_items: AlignItems::Center, column_gap: vh(PAD), padding: UiRect::left(vh(20.0)), border: UiRect::left(vh(BAR)), ..default() };
            let mut e = col.spawn((node, BackgroundColor(Color::NONE), BorderColor::all(Color::NONE)));
            e.with_children(|r| {
                r.spawn((t(kit, Type::Label, it.num, if it.selected { AMBER } else { TX2 }), Node { width: vh(PAD), ..default() }, Pickable::IGNORE));
                r.spawn((kit_text(it.label.to_uppercase(), if compact { 36.0 } else { 52.0 }, color, &kit.display), Pickable::IGNORE));
                if !it.jp.is_empty() {
                    r.spawn((t(kit, Type::Jp, it.jp, TX3), Pickable::IGNORE));
                }
            });
            if it.selected {
                chosen = Some(e.id());
            } else {
                hoverable(&mut e, (Color::NONE, HOVER), (Color::NONE, LINE_3));
            }
            pressable(&mut e, it.action);
        }
        hairline(col);
        match chosen {
            Some(target) => {
                col.commands().entity(ind).insert(Slide { key: key.to_string(), target });
            }
            None => {
                col.commands().entity(ind).insert(Visibility::Hidden);
            }
        }
    });
}

/// One option: framed; hover brightens the frame. `framed`: a lone chip draws its own amber
/// selection; in a group the sliding selection draws it.
fn chip_node(p: &mut ChildSpawnerCommands, kit: &Kit, label: &str, action: UiAction, selected: bool, framed: bool, lead: impl FnOnce(&mut ChildSpawnerCommands)) -> Entity {
    let frame = if selected && framed { AMBER } else if selected { Color::NONE } else { LINE_2 };
    let bg = if selected && framed { AMBER_WASH } else { Color::NONE };
    let node = Node { height: vh(44.0), padding: UiRect::horizontal(vh(18.0)), column_gap: vh(10.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, border: UiRect::all(px(1)), border_radius: BorderRadius::all(vh(RADIUS_S)), ..default() };
    let mut e = p.spawn((node, BackgroundColor(bg), BorderColor::all(frame), super::panels::lit()));
    e.with_children(|c| {
        lead(c);
        c.spawn((kit_text(label.to_uppercase(), 13.0, if selected { AMBER } else { TX2 }, &kit.text), Pickable::IGNORE));
    });
    if !selected {
        hoverable(&mut e, (Color::NONE, HOVER), (LINE_2, LINE_3));
    }
    pressable(&mut e, action);
    e.id()
}

/// A single framed choice (toast buttons, Invite). Selected: amber frame and text.
pub fn chip(p: &mut ChildSpawnerCommands, kit: &Kit, label: &str, action: UiAction, selected: bool) {
    chip_node(p, kit, label, action, selected, true, |_| {});
}

/// A lone chip with a fixed width (a column of actions that must line up).
pub fn chip_w(p: &mut ChildSpawnerCommands, kit: &Kit, label: &str, action: UiAction, width: f32) {
    let id = chip_node(p, kit, label, action, false, true, |_| {});
    p.commands().entity(id).entry::<Node>().and_modify(move |mut n| n.width = vh(width));
}

/// A key binding: a fixed-width chip with the key; amber while it waits for a new one.
pub fn key_chip(p: &mut ChildSpawnerCommands, kit: &Kit, label: &str, action: UiAction, waiting: bool) {
    let id = chip_node(p, kit, label, action, waiting, true, |_| {});
    p.commands().entity(id).entry::<Node>().and_modify(|mut n| n.width = vh(136.0));
}

/// A choice in a group; `lead` draws something before the label (an assistant portrait).
pub struct Choice<'a> {
    pub label: &'a str,
    pub action: UiAction,
    pub selected: bool,
    pub lead: Option<Box<dyn FnOnce(&mut ChildSpawnerCommands) + 'a>>,
}

impl<'a> Choice<'a> {
    pub fn new(label: &'a str, action: UiAction, selected: bool) -> Choice<'a> {
        Choice { label, action, selected, lead: None }
    }
}

/// A row of choices with one sliding selection (amber frame on an amber wash).
pub fn chips(p: &mut ChildSpawnerCommands, kit: &Kit, key: &str, options: Vec<Choice>) {
    // one line, always: groups are sized to fit their row
    p.spawn(Node { column_gap: vh(GAP_S), align_items: AlignItems::Center, flex_shrink: 0.0, ..default() }).with_children(|g| {
        let ind = g.spawn((Node { position_type: PositionType::Absolute, border: UiRect::all(px(1)), border_radius: BorderRadius::all(vh(RADIUS_S)), ..default() }, BackgroundColor(AMBER_WASH), BorderColor::all(AMBER), Pickable::IGNORE)).id();
        let mut chosen = None;
        for o in options {
            let lead = o.lead;
            let id = chip_node(g, kit, o.label, o.action, o.selected, false, |c| {
                if let Some(f) = lead {
                    f(c);
                }
            });
            if o.selected {
                chosen = Some(id);
            }
        }
        match chosen {
            Some(target) => {
                g.commands().entity(ind).insert(Slide { key: key.to_string(), target });
            }
            None => {
                g.commands().entity(ind).insert(Visibility::Hidden);
            }
        }
    });
}

/// "−  value  +". When the value changed since last shown, it rolls in from the side it moved
/// toward.
pub fn stepper(p: &mut ChildSpawnerCommands, kit: &Kit, memory: &mut UiMemory, key: &str, value: &str, minus: UiAction, plus: UiAction) {
    let num = |s: &str| s.trim_end_matches(['%', ' ']).parse::<f32>().ok();
    let up = memory.previous(key).and_then(num).zip(num(value)).map(|(a, b)| b > a);
    let changed = memory.changed(key, value);
    p.spawn(Node { align_items: AlignItems::Center, column_gap: vh(GAP_S), ..default() }).with_children(|r| {
        chip_node(r, kit, "−", minus, false, true, |_| {});
        let bx = Node { min_width: vh(96.0), height: vh(44.0), border_radius: BorderRadius::all(vh(RADIUS_S)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, border: UiRect::bottom(px(1)), overflow: Overflow::clip(), ..default() };
        r.spawn((bx, BorderColor::all(LINE_2))).with_children(|v| {
            let mut e = v.spawn(kit_text(value, 20.0, TX1, &kit.num));
            if changed {
                e.insert(Anim::roll(up.unwrap_or(true)));
            }
        });
        chip_node(r, kit, "+", plus, false, true, |_| {});
    });
}

/// One settings row: name on the left, controls on the right, hairline above.
pub fn setting_row(p: &mut ChildSpawnerCommands, kit: &Kit, name: &str, controls: impl FnOnce(&mut ChildSpawnerCommands)) -> Entity {
    p.spawn((Node { min_height: vh(60.0), align_items: AlignItems::Center, padding: UiRect::vertical(vh(GAP_S)), border: UiRect::top(px(1)), ..default() }, BorderColor::all(LINE))).with_children(|r| {
        r.spawn((kit_text(name.to_uppercase(), 22.0, TX1, &kit.text), Node { width: vh(360.0), flex_shrink: 0.0, ..default() }));
        r.spawn(Node { align_items: AlignItems::Center, column_gap: vh(GAP_S), ..default() }).with_children(controls);
    })
    .id()
}

fn primary_node() -> Node {
    Node { height: vh(64.0), min_width: vh(420.0), padding: UiRect::horizontal(vh(PAD)), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, column_gap: vh(32.0), border_radius: BorderRadius::all(vh(RADIUS_S)), ..default() }
}

/// The one main action on a page: an amber frame, the amber bar on the reading edge, the key
/// on the far edge.
pub fn primary_button(p: &mut ChildSpawnerCommands, kit: &Kit, label: &str, hint: &str, action: UiAction) -> Entity {
    let node = Node { border: UiRect { left: vh(6.0), right: vh(2.0), top: vh(2.0), bottom: vh(2.0) }, ..primary_node() };
    let mut e = p.spawn((node, BackgroundColor(AMBER_WASH), BorderColor::all(AMBER), super::panels::lit(), super::panels::elevation(1)));
    e.with_children(|r| {
        r.spawn((kit_text(label.to_uppercase(), 18.0, TX1, &kit.text), Pickable::IGNORE));
        r.spawn((t(kit, Type::Label, hint, AMBER), Pickable::IGNORE));
    });
    hoverable(&mut e, (AMBER_WASH, AMBER_WASH_2), (AMBER, AMBER));
    pressable(&mut e, action);
    e.id()
}

/// The main action when it can't be taken: the same shape, quiet, the reason in red.
pub fn primary_off(p: &mut ChildSpawnerCommands, kit: &Kit, label: &str, reason: &str) {
    let node = Node { border: UiRect { left: vh(6.0), right: px(1), top: px(1), bottom: px(1) }, ..primary_node() };
    p.spawn((node, BorderColor::all(LINE_2))).with_children(|r| {
        r.spawn(kit_text(label.to_uppercase(), 18.0, TX3, &kit.text));
        r.spawn(t(kit, Type::Label, reason, BAD));
    });
}

/// Page heading: small label, big title, Japanese subtitle.
pub fn page_title(p: &mut ChildSpawnerCommands, kit: &Kit, eyebrow: &str, title: &str, jp: &str) {
    p.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|c| {
        c.spawn(t(kit, Type::Label, eyebrow, TX2));
        c.spawn(Node { align_items: AlignItems::Baseline, column_gap: vh(GAP), margin: UiRect::top(vh(GAP_S)), ..default() }).with_children(|r| {
            r.spawn(t(kit, Type::H2, title, TX1));
            r.spawn(t(kit, Type::Jp, jp, TX3));
        });
    });
}

/// A small status word in a frame of its own colour (READY, C4, DEAD...).
pub fn status_label(p: &mut ChildSpawnerCommands, kit: &Kit, s: &str, color: Color) {
    p.spawn((Node { padding: UiRect::axes(vh(10.0), vh(6.0)), border: UiRect::all(px(1)), border_radius: BorderRadius::all(vh(RADIUS_S)), ..default() }, BorderColor::all(color.with_alpha(color.alpha() * 0.4)))).with_children(|c| {
        c.spawn(t(kit, Type::Label, s, color));
    });
}
