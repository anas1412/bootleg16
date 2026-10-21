//! The buy menu (B), direction E: a full-screen armory with the categories on the left
//! and a card per item (picture, name, price). CS 1.6 keys still work (number picks,
//! 0 closes) and everything is clickable. No ammo to buy: guns come full and reserves
//! are topped up every round.
use crate::game::{Frame, Walker};
use crate::menu::{AppState, Overlay};
use crate::hud::{weapon_icon, Icons};
use crate::ui::{chip, chip_off, clickable_keep_border, digit_pressed, kit_text, nav, primary_button, primary_off, vh, Anim, Kit, NavItem, UiAction, UiActions, AMBER, BAD, BG1, CT_COLOR, LINE, OK, TX1, TX2, TX3, T_COLOR};
use bevy::prelude::*;
use net::protocol::{BuyItem, Team};

/// Teammates with less than this show red in the buy menu: they can't buy a rifle (an AK-47).
const RIFLE: i32 = 2700;
/// Two clicks on a card within this many seconds buy it.
const DOUBLE_CLICK: f32 = 0.4;

pub struct BuyPlugin;

impl Plugin for BuyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BuyMenu>()
            .add_systems(OnEnter(AppState::InGame), spawn)
            .add_systems(Update, (buy_menu, buy_clock).chain().in_set(Frame::Ui));
    }
}

/// Where you are in the shop: the category (None: the first) and the card. Whether it is
/// open is `Overlay::Buy` (overlay.rs opens and closes every menu over the game).
#[derive(Resource, Default)]
pub struct BuyMenu(Option<usize>, usize);

/// Why you can't open the shop now (None: you can).
pub fn why_not(w: &Walker) -> Option<&'static str> {
    let team = w.client.team();
    if w.client.me_info.can_buy {
        None
    } else if !w.client.vitals.alive() || team == Team::Spectator {
        Some("") // nothing to say: you're dead or watching
    } else {
        let side = if team == Team::Ct { sim::zones::Side::Ct } else { sim::zones::Side::T };
        Some(if w.client.world.zones.in_buy_zone(side, w.client.me().origin) { "Buy time is over" } else { "Not in a buy zone" })
    }
}

/// One buy menu category: its name and items.
#[derive(Clone)]
struct Entry(&'static str, Vec<(&'static str, BuyItem)>);

/// Terrorists and counter-terrorists see different guns.
fn layout(ct: bool) -> Vec<Entry> {
    use sim::weapons::Id::*;
    use BuyItem::*;
    let side = |t: (&'static str, sim::weapons::Id), c: (&'static str, sim::weapons::Id)| if ct { (c.0, Weapon(c.1)) } else { (t.0, Weapon(t.1)) };
    let mut equipment = vec![("Kevlar", Kevlar), ("Kevlar + Helmet", KevlarHelmet), ("Flashbang", Weapon(Flashbang)), ("HE Grenade", Weapon(HeGrenade)), ("Smoke Grenade", Weapon(Smoke))];
    if ct {
        equipment.push(("Defuse Kit", DefuseKit));
    }
    vec![
        Entry("Pistols", vec![("Glock-18", Weapon(Glock)), ("USP", Weapon(Usp)), ("P228", Weapon(P228)), ("Desert Eagle", Weapon(Deagle))]),
        Entry("SMGs", vec![side(("MAC-10", Mac10), ("TMP", Tmp)), ("MP5", Weapon(Mp5)), ("UMP45", Weapon(Ump45)), ("P90", Weapon(P90))]),
        Entry(
            "Rifles",
            if ct {
                vec![("FAMAS", Weapon(Famas)), ("M4A1", Weapon(M4a1)), ("Scout", Weapon(Scout)), ("AWP", Weapon(Awp))]
            } else {
                vec![("Galil", Weapon(Galil)), ("AK-47", Weapon(Ak47)), ("Scout", Weapon(Scout)), ("AWP", Weapon(Awp))]
            },
        ),
        Entry("Equipment", equipment),
    ]
}



#[derive(Component)]
struct BuyRoot;

/// The buy time left, by the side's name in the header.
#[derive(Component)]
struct BuyClock;

/// Buy time left under this many seconds shows amber.
const BUY_HURRY: f32 = 5.0;

/// "· 0:14 LEFT" while buy time runs (none in practice, deathmatch and warmup). At 0 the server
/// stops buying and overlay.rs closes the shop.
fn buy_clock(w: Res<Walker>, mut clock: Query<(&mut Text, &mut TextColor), With<BuyClock>>) {
    let left = w.client.me_info.buy_left;
    for (mut t, mut c) in &mut clock {
        let s = left.map(|l| {
            let n = l.max(0.0).ceil() as u32;
            format!("· {}:{:02} LEFT", n / 60, n % 60)
        });
        let s = s.unwrap_or_default();
        if t.0 != s {
            t.0 = s;
        }
        c.set_if_neq(TextColor(if left.is_some_and(|l| l < BUY_HURRY) { AMBER } else { TX2 }));
    }
}

fn spawn(mut menu: ResMut<BuyMenu>) {
    menu.0 = None;
    menu.1 = 0;
}

/// The shop while it's open: number keys, Enter and clicks pick and buy (as in CS), double-click
/// buys, and it stays open for more. Opening, closing and the mouse are overlay.rs's.
#[allow(clippy::too_many_arguments)]
fn buy_menu(
    mut commands: Commands,
    (kit, icons): (Res<Kit>, Res<Icons>),
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: ResMut<UiActions>,
    mut w: ResMut<Walker>,
    mut menu: ResMut<BuyMenu>,
    mut shown: Local<Option<(Option<Option<usize>>, usize, Team, i32, Vec<(String, i32, bool)>, Vec<Option<u8>>, bool)>>,
    roots: Query<Entity, With<BuyRoot>>,
    mut overlay: ResMut<Overlay>,
    (time, mut last_click): (Res<Time>, Local<Option<(usize, f32)>>),
    // bought anything this round (the round it was): Undo needs something to undo
    mut bought: Local<(u32, bool)>,
) {
    let auto = std::env::var_os("BOOTLEG16_SCREENSHOT").is_some();
    if auto && std::env::var_os("BOOTLEG16_MENU").is_none() {
        return;
    }
    let team = w.client.team();
    let ct = team == Team::Ct;
    let layout = layout(ct);
    // open: on which category (None: the first)
    let mut state: Option<Option<usize>> = (*overlay == Overlay::Buy).then_some(menu.0);
    // Number keys (top row or numpad) become the same actions as clicks.
    // Enter buys the selected card.
    if keys.just_pressed(KeyCode::Enter) {
        if let Some(Some(cat)) = state {
            if let Some(Entry(_, items)) = layout.get(cat) {
                if let Some((_, item)) = items.get(menu.1) {
                    actions.0.push(UiAction::Buy(*item));
                }
            }
        }
    }
    if let (Some(state), Some(n)) = (state, digit_pressed(&keys)) {
        let i = n.wrapping_sub(1);
        actions.0.push(match (n, state) {
            (0, _) => UiAction::CloseBuy,
            (_, None) => UiAction::BuyCategory(i),
            (_, Some(cat)) => match layout.get(cat) {
                Some(Entry(_, items)) if i < items.len() => UiAction::Buy(items[i].1),
                _ => UiAction::BuyCategory(cat),
            },
        });
    }
    let mut buy_now = false;
    actions.0.retain(|a| match a {
        UiAction::CloseBuy => {
            state = None;
            false
        }
        UiAction::UndoBuys => {
            w.client.undo_buys();
            bought.1 = false;
            false
        }
        UiAction::BuyCategory(i) => {
            if *i < layout.len() {
                state = Some(Some(*i));
                menu.1 = 0;
            }
            false
        }
        UiAction::SelectBuy(i) => {
            menu.1 = *i;
            // a second click on the same card soon after buys it
            let now = time.elapsed_secs();
            if last_click.is_some_and(|(j, at)| j == *i && now - at < DOUBLE_CLICK) {
                *last_click = None;
                buy_now = true;
            } else {
                *last_click = Some((*i, now));
            }
            false
        }
        // the menu stays open: buy several things, undo, close with B, Esc or 0
        UiAction::Buy(item) => {
            w.client.buy(*item);
            bought.1 = true;
            false
        }
        _ => true,
    });
    // Buy time over, left the buy zone, died: the menu goes away.
    if buy_now {
        if let Some(Some(cat)) = state {
            if let Some((_, item)) = layout.get(cat).and_then(|Entry(_, items)| items.get(menu.1)) {
                w.client.buy(*item);
                bought.1 = true;
            }
        }
    }
    // a new round, or dying, empties the list (the server's too)
    if bought.0 != w.client.round.round || !w.client.vitals.alive() {
        *bought = (w.client.round.round, false);
    }
    if auto && state.is_none() {
        // Screenshot check: `BOOTLEG16_MENU=<page>` (0 pistols .. 3 equipment; default rifles).
        state = Some(Some(std::env::var("BOOTLEG16_MENU").ok().and_then(|v| v.parse().ok()).unwrap_or(2)));
        *overlay = Overlay::Buy;
    }
    match state {
        Some(cat) => menu.0 = cat,
        None if *overlay == Overlay::Buy => *overlay = Overlay::None,
        None => {}
    }

    let money = w.client.me_info.money;
    // teammates' money (CS2): who can buy, who needs a drop. The server only sends your team's.
    let mates: Vec<(String, i32, bool)> = w.client.scores.iter().filter(|s| Some(s.id) != w.client.id()).filter_map(|s| s.money.map(|m| (s.name.clone(), m, s.alive))).collect();
    // what you already have, per item (the cards say OWNED, grenades how many)
    let have: Vec<Option<u8>> = layout.iter().flat_map(|Entry(_, items)| items.iter().map(|(_, item)| owned(&w, *item))).collect();
    if *shown == Some((state, menu.1, team, money, mates.clone(), have.clone(), bought.1)) {
        return;
    }
    // What changed decides what moves: opening tunes everything in; a new category tunes the
    // cards in (staggered) and the details; a new card just the details; closing tunes out.
    let prev = shown.replace((state, menu.1, team, money, mates.clone(), have, bought.1));
    let was = prev.as_ref().and_then(|p| p.0);
    let opening = was.is_none();
    let new_cat = opening || was != Some(state.unwrap_or(None)) || prev.as_ref().map(|p| p.2) != Some(team);
    let new_item = new_cat || prev.as_ref().map(|p| p.1) != Some(menu.1);
    for e in &roots {
        if state.is_none() {
            commands.entity(e).remove::<(BuyRoot, crate::ui::Blurs)>().insert(Anim::tune_out());
        } else {
            commands.entity(e).despawn();
        }
    }
    let Some(state) = state else { return };
    let tune = |on: bool, i: usize| on.then(|| Anim::stagger(i));
    let free = w.client.practice || w.client.round.phase == net::rounds::Phase::Warmup;
    // What each item costs you now, by the server's own rule (None: you have it, or carry
    // the most allowed; full armor makes the helmet the only cost).
    let now = |item: BuyItem| net::server::buy::cost_now(item, &w.client.state.loadout, w.client.vitals.armor, w.client.vitals.helmet, w.client.me_info.defuse_kit);
    let cost = |item: BuyItem| match now(item) {
        None => String::new(),
        _ if free => "FREE".to_string(),
        Some(p) => format!("${p}"),
    };
    // Prices you can't pay show red, the card dims.
    let afford = |item: BuyItem| now(item).is_some_and(|p| free || p <= money);
    let team_color = if ct { CT_COLOR } else { T_COLOR };
    let cat = state.unwrap_or(0);
    let Some(Entry(_, items)) = layout.get(cat).cloned() else { return };
    let sel = menu.1.min(items.len().saturating_sub(1));
    let abs = |node: Node| Node { position_type: PositionType::Absolute, ..node };
    // Layout at 1080p (from the mockup): 96 side margins; header at 120 under the HUD's
    // top band; categories 96..456, cards 496..1316 (2 columns), details 1356..1824.
    let top = 240.0;
    commands
        .spawn((
            BuyRoot,
            DespawnOnExit(AppState::InGame),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
            crate::ui::Blurs(0.9),
            GlobalZIndex(7),
        ))
        .with_children(|root| {
            let mut head = root.spawn((abs(Node { left: vh(96.0), right: vh(96.0), top: vh(120.0), height: vh(96.0), align_items: AlignItems::FlexEnd, padding: UiRect::bottom(vh(24.0)), border: UiRect::bottom(px(1)), ..default() }), BorderColor::all(LINE)));
            if let Some(a) = tune(opening, 0) {
                head.insert(a);
            }
            head.with_children(|h| {
                    h.spawn(Node { flex_direction: FlexDirection::Column, row_gap: vh(8.0), ..default() }).with_children(|t| {
                        t.spawn(Node { column_gap: vh(8.0), ..default() }).with_children(|l| {
                            l.spawn(kit_text(if ct { "BUY · BUREAU" } else { "BUY · STRAYS" }, 13.0, team_color, &kit.text));
                            l.spawn((BuyClock, kit_text("", 13.0, TX2, &kit.text)));
                        });
                        t.spawn(Node { align_items: AlignItems::Baseline, column_gap: vh(16.0), ..default() }).with_children(|r| {
                            r.spawn(kit_text("ARMORY", 52.0, TX1, &kit.display));
                            r.spawn(kit_text("武器庫", 13.0, TX3, &kit.jp));
                        });
                    });
                    if !free && !mates.is_empty() {
                        h.spawn(Node { margin: UiRect::left(Val::Auto), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: vh(8.0), ..default() }).with_children(|t| {
                            t.spawn(kit_text("TEAM", 13.0, TX2, &kit.text));
                            t.spawn(Node { column_gap: vh(24.0), ..default() }).with_children(|r| {
                                for (name, m, alive) in &mates {
                                    let dim = if *alive { 1.0 } else { 0.45 };
                                    r.spawn(Node { align_items: AlignItems::Baseline, column_gap: vh(8.0), ..default() }).with_children(|b| {
                                        b.spawn(kit_text(name.clone(), 16.0, TX2.with_alpha(dim), &kit.text));
                                        b.spawn(kit_text(format!("${m}"), 20.0, if *m < RIFLE { BAD } else { TX1 }.with_alpha(dim), &kit.num));
                                    });
                                }
                            });
                        });
                    }
                    let lead = if !free && !mates.is_empty() { vh(48.0) } else { Val::Auto };
                    h.spawn(Node { margin: UiRect::left(lead), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: vh(8.0), ..default() }).with_children(|f| {
                        f.spawn(kit_text("FUNDS", 13.0, TX2, &kit.text));
                        f.spawn(Node { align_items: AlignItems::Baseline, column_gap: vh(4.0), ..default() }).with_children(|r| {
                            if free {
                                r.spawn(kit_text("FREE", 44.0, OK, &kit.num));
                            } else {
                                r.spawn(kit_text("$", 44.0, OK, &kit.num));
                                r.spawn(kit_text(money.to_string(), 44.0, TX1, &kit.num));
                            }
                        });
                    });
                });
            // Categories.
            // The three columns share one row between the side margins, so a narrower screen
            // (16:10, a small window) squeezes the cards, not the details.
            root.spawn(abs(Node { left: vh(96.0), right: vh(96.0), top: vh(top), bottom: vh(96.0), column_gap: vh(40.0), ..default() })).with_children(|body| {
            let mut cats = body.spawn(Node { width: vh(360.0), flex_shrink: 0.0, flex_direction: FlexDirection::Column, ..default() });
            if let Some(a) = tune(opening, 1) {
                cats.insert(a);
            }
            cats.with_children(|col| {
                let nums: Vec<String> = (1..=layout.len()).map(|i| format!("{i:02}")).collect();
                nav(col, &kit, "buy.cat", layout.iter().enumerate().map(|(i, e)| NavItem { num: &nums[i], label: e.0, jp: "", action: UiAction::BuyCategory(i), selected: i == cat }).collect(), true);
                col.spawn((kit_text("1-9 · BUY    ENTER · BUY SELECTED    ESC · CLOSE", 13.0, TX3, &kit.text), Node { margin: UiRect::top(vh(24.0)), padding: UiRect::left(vh(16.0)), ..default() }));
                // sell back this round's buys (the server refunds only what's unused; free buys just go)
                col.spawn(Node { margin: UiRect { top: vh(24.0), left: vh(16.0), ..default() }, ..default() }).with_children(|r| {
                    if bought.1 {
                        chip(r, &kit, "Undo buys", UiAction::UndoBuys, false);
                    } else {
                        chip_off(r, &kit, "Undo buys");
                    }
                });
            });
            // Cards, two columns.
            body.spawn(Node { flex_grow: 1.0, flex_basis: px(0), min_width: px(0), flex_wrap: FlexWrap::Wrap, justify_content: JustifyContent::SpaceBetween, row_gap: vh(16.0), align_content: AlignContent::FlexStart, ..default() }).with_children(|grid| {
                for (i, (name, item)) in items.iter().enumerate() {
                    let ok = afford(*item);
                    let selected = i == sel;
                    let base = if selected { BG1.mix(&AMBER, 0.08) } else { BG1 };
                    let mut card = grid.spawn(crate::ui::card(Node { width: percent(48.5), height: vh(200.0), flex_direction: FlexDirection::Column, padding: UiRect::axes(vh(24.0), vh(20.0)), ..default() }, selected));
                    if let Some(a) = tune(new_cat, i + 2) {
                        card.insert(a);
                    }
                    card.with_children(|c| {
                        c.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|top| {
                            top.spawn((Node { column_gap: vh(10.0), align_items: AlignItems::Center, ..default() }, Pickable::IGNORE)).with_children(|n| {
                                n.spawn((kit_text(format!("{}", i + 1), 13.0, if selected { AMBER } else { TX2 }, &kit.text), Pickable::IGNORE));
                                match owned(&w, *item) {
                                    Some(c) if c > 1 => n.spawn((kit_text(format!("OWNED x{c}"), 13.0, AMBER, &kit.text), Pickable::IGNORE)),
                                    Some(_) => n.spawn((kit_text("OWNED", 13.0, AMBER, &kit.text), Pickable::IGNORE)),
                                    None => n.spawn(Node::default()),
                                };
                            });
                            top.spawn((kit_text(cost(*item), 20.0, if ok { TX1 } else { BAD }, &kit.num), Pickable::IGNORE));
                        });
                        c.spawn((Node { flex_grow: 1.0, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, Pickable::IGNORE)).with_children(|pic| {
                            item_art(pic, &icons, *item, 76.0, if ok { TX1 } else { TX3 });
                        });
                        c.spawn((kit_text(name.to_uppercase(), 26.0, if ok { TX1 } else { TX3 }, &kit.text), Pickable::IGNORE));
                    });
                    clickable_keep_border(&mut card, UiAction::SelectBuy(i), base);
                }
            });
            // Details of the selected card.
            let (name, item) = items[sel];
            body.spawn((Node { width: vh(468.0), flex_shrink: 0.0, ..default() }, BackgroundColor(BG1)))
                .with_children(|panel| {
                    let mut d = panel.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, padding: UiRect::all(vh(32.0)), ..default() });
                    if let Some(a) = tune(new_item, if opening { 3 } else { 0 }) {
                        d.insert(a);
                    }
                    d.with_children(|d| {
                    d.spawn(kit_text(format!("{} · SELECTED", layout[cat].0.to_uppercase()), 13.0, AMBER, &kit.text));
                    // the name fills the panel width at most
                    let size = (404.0 / (name.len() as f32 * 0.43)).min(96.0);
                    d.spawn((kit_text(name.to_uppercase(), size, TX1, &kit.display), Node { margin: UiRect::top(vh(8.0)), ..default() }));
                    d.spawn(Node { height: vh(150.0), margin: UiRect::vertical(vh(16.0)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }).with_children(|pic| {
                        item_art(pic, &icons, item, 120.0, TX1);
                    });
                    for (label, value, frac) in stats(item) {
                        d.spawn((Node { height: vh(44.0), align_items: AlignItems::Center, column_gap: vh(16.0), border: UiRect::top(px(1)), ..default() }, BorderColor::all(LINE))).with_children(|r| {
                            r.spawn((kit_text(label, 13.0, TX2, &kit.text), Node { width: vh(110.0), ..default() }));
                            r.spawn((Node { flex_grow: 1.0, height: vh(4.0), ..default() }, BackgroundColor(LINE))).with_children(|b| {
                                b.spawn((Node { width: percent(frac.clamp(0.0, 1.0) * 100.0), height: percent(100), ..default() }, BackgroundColor(TX1)));
                            });
                            r.spawn((kit_text(value, 16.0, TX1, &kit.num), Node { width: vh(92.0), justify_content: JustifyContent::FlexEnd, ..default() }));
                        });
                    }
                    d.spawn(Node { flex_grow: 1.0, ..default() });
                    if afford(item) {
                        primary_button(d, &kit, &buy_label(&cost(item)), "Enter", UiAction::Buy(item));
                    } else {
                        let why = match now(item) {
                            Some(p) => format!("Need ${}", p - money),
                            None => "Owned".to_string(),
                        };
                        primary_off(d, &kit, &buy_label(&cost(item)), &why);
                    }
                    });
                });
            });
        });
}

/// The buy button's words: the price after it, when there is one.
fn buy_label(cost: &str) -> String {
    if cost.is_empty() { "Buy".into() } else { format!("Buy · {cost}") }
}

/// How many of an item you already have (None: none).
fn owned(w: &Walker, item: BuyItem) -> Option<u8> {
    use sim::weapons::Id;
    let l = &w.client.state.loadout;
    let v = &w.client.vitals;
    let n = match item {
        BuyItem::Weapon(Id::HeGrenade) => l.grenades[0],
        BuyItem::Weapon(Id::Flashbang) => l.grenades[1],
        BuyItem::Weapon(Id::Smoke) => l.grenades[2],
        BuyItem::Weapon(id) => (l.primary.as_ref().is_some_and(|g| g.id == id) || l.secondary.as_ref().is_some_and(|g| g.id == id)) as u8,
        BuyItem::Kevlar => (v.armor >= 100.0) as u8,
        BuyItem::KevlarHelmet => (v.armor >= 100.0 && v.helmet) as u8,
        BuyItem::DefuseKit => w.client.me_info.defuse_kit as u8,
    };
    (n > 0).then_some(n)
}

/// The picture for an item: gun art, or the HUD icon for armor and the kit.
fn item_art(p: &mut ChildSpawnerCommands, icons: &Icons, item: BuyItem, height: f32, color: Color) {
    let name = match item {
        BuyItem::Weapon(id) => Some(weapon_icon(id)),
        BuyItem::Kevlar => Some("suit_full"),
        BuyItem::KevlarHelmet => Some("suithelmet_full"),
        BuyItem::DefuseKit => Some("defuser"),
    };
    let h = if matches!(item, BuyItem::Weapon(_)) { height } else { height * 0.8 };
    if let Some(n) = name {
        // no wider than its card (narrow screens squeeze the cards): the width leads, the
        // height follows the picture's shape and stops at `h`
        p.spawn((icons.node_in(n, h, 3.8, color), Pickable::IGNORE)).entry::<Node>().and_modify(move |mut node| {
            if let (Val::Vh(tall), Some(aspect)) = (node.height, node.aspect_ratio) {
                node.width = Val::Vh(tall * aspect);
                node.height = Val::Auto;
                node.max_width = percent(90.0);
            }
        });
    }
}

/// Real numbers from the weapon data: label, value, bar fill (0..1).
fn stats(item: BuyItem) -> Vec<(&'static str, String, f32)> {
    use sim::weapons::data::Kind;
    let BuyItem::Weapon(id) = item else {
        return match item {
            BuyItem::Kevlar => vec![("ARMOR", "100".into(), 1.0)],
            BuyItem::KevlarHelmet => vec![("ARMOR", "100".into(), 1.0), ("HELMET", "YES".into(), 1.0)],
            _ => vec![("DEFUSE", "5 S".into(), 0.5)],
        };
    };
    let def = sim::weapons::def(id);
    let Kind::Gun(g) = def.kind else { return vec![] };
    let m = &g.modes[0];
    let rpm = (60.0 / m.cycle).round();
    vec![
        ("DAMAGE", m.damage.to_string(), m.damage as f32 / 120.0),
        ("FIRE RATE", format!("{rpm}"), rpm / 1000.0),
        ("MAGAZINE", format!("{}/{}", g.clip, g.reserve), g.clip as f32 / 100.0),
        ("SPEED", format!("{:.0}", g.speed), g.speed / 260.0),
        ("PENETRATE", g.penetration.to_string(), g.penetration as f32 / 3.0),
    ]
}
