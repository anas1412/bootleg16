//! The buy menu (B), direction E: a full-screen armory with the categories on the left
//! and a card per item (picture, name, price). CS 1.6 keys still work (number picks,
//! 0 closes) and everything is clickable. No ammo to buy: guns come full and reserves
//! are topped up every round.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::hud::{gun_art, Icons};
use crate::ui::{clickable_keep_border, kit_text, nav, primary_button, primary_off, vh, Anim, Kit, NavItem, UiAction, UiActions, AMBER, BAD, BG1, CT_COLOR, LINE, OK, TX1, TX2, TX3, T_COLOR};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use net::protocol::{BuyItem, Team};
use net::server::buy::price;

pub struct BuyPlugin;

impl Plugin for BuyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BuyMenu>()
            .add_systems(OnEnter(AppState::InGame), spawn)
            .add_systems(Update, buy_menu.in_set(Frame::Ui));
    }
}

/// Closed (None), open on the categories (Some(None)) or on one category.
#[derive(Resource, Default)]
pub struct BuyMenu(Option<Option<usize>>, usize);

impl BuyMenu {
    pub fn is_open(&self) -> bool {
        self.0.is_some()
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
        Entry("Pistols", vec![("Glock-18", Weapon(Glock)), ("USP", Weapon(Usp)), ("Desert Eagle", Weapon(Deagle))]),
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

fn spawn(mut menu: ResMut<BuyMenu>) {
    menu.0 = None;
    menu.1 = 0;
}

/// B opens / closes; with the menu open the mouse is free to click, and number keys work as in CS.
#[allow(clippy::too_many_arguments)]
fn buy_menu(
    mut commands: Commands,
    (kit, icons): (Res<Kit>, Res<Icons>),
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: ResMut<UiActions>,
    mut w: ResMut<Walker>,
    mut menu: ResMut<BuyMenu>,
    mut shown: Local<Option<(Option<Option<usize>>, usize, Team, i32)>>,
    mut cursor: Single<&mut CursorOptions>,
    roots: Query<Entity, With<BuyRoot>>,
) {
    let auto = std::env::var_os("BOOTLEG16_SCREENSHOT").is_some();
    if auto && std::env::var_os("BOOTLEG16_MENU").is_none() {
        return;
    }
    let team = w.client.team();
    let ct = team == Team::Ct;
    let layout = layout(ct);
    let was_open = menu.0.is_some();
    // Number keys become the same actions as clicks.
    let digits = [KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];
    let can_buy = w.client.me_info.can_buy;
    // Enter buys the selected card.
    if keys.just_pressed(KeyCode::Enter) {
        if let Some(Some(cat)) = menu.0 {
            if let Some(Entry(_, items)) = layout.get(cat) {
                if let Some((_, item)) = items.get(menu.1) {
                    actions.0.push(UiAction::Buy(*item));
                }
            }
        }
    }
    if keys.just_pressed(KeyCode::KeyB) && can_buy {
        menu.0 = if menu.0.is_some() { None } else { Some(None) };
    } else if let (Some(state), Some(n)) = (menu.0, digits.iter().position(|k| keys.just_pressed(*k))) {
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
    actions.0.retain(|a| match a {
        UiAction::CloseBuy => {
            menu.0 = None;
            false
        }
        UiAction::BuyCategory(i) => {
            if *i < layout.len() {
                menu.0 = Some(Some(*i));
                menu.1 = 0;
            }
            false
        }
        UiAction::SelectBuy(i) => {
            menu.1 = *i;
            false
        }
        UiAction::Buy(item) => {
            w.client.buy(*item);
            menu.0 = None;
            false
        }
        _ => true,
    });
    // Buy time over, left the buy zone, died: the menu goes away.
    if !can_buy && !auto {
        menu.0 = None;
    }
    if auto && menu.0.is_none() {
        menu.0 = Some(Some(2)); // screenshot check: show the rifles page
    }
    // Free the mouse while buying; take it back when done.
    if menu.0.is_some() && !was_open {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else if menu.0.is_none() && was_open && !auto {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }

    let money = w.client.me_info.money;
    if *shown == Some((menu.0, menu.1, team, money)) {
        return;
    }
    // What changed decides what moves: opening tunes everything in; a new category tunes the
    // cards in (staggered) and the details; a new card just the details; closing tunes out.
    let prev = shown.replace((menu.0, menu.1, team, money));
    let was = prev.and_then(|p| p.0);
    let opening = was.is_none();
    let new_cat = opening || was != Some(menu.0.unwrap_or(None)) || prev.map(|p| p.2) != Some(team);
    let new_item = new_cat || prev.map(|p| p.1) != Some(menu.1);
    for e in &roots {
        if menu.0.is_none() {
            commands.entity(e).remove::<(BuyRoot, crate::ui::Blurs)>().insert(Anim::tune_out());
        } else {
            commands.entity(e).despawn();
        }
    }
    let Some(state) = menu.0 else { return };
    let tune = |on: bool, i: usize| on.then(|| Anim::stagger(i));
    let free = w.client.practice || w.client.round.phase == net::rounds::Phase::Warmup;
    let cost = |item: BuyItem| match price(item) {
        _ if free => "FREE".to_string(),
        Some(p) => format!("${p}"),
        None => String::new(),
    };
    // Prices you can't pay show red, the card dims.
    let afford = |item: BuyItem| free || price(item).is_none_or(|p| p <= money);
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
                        t.spawn(kit_text(if ct { "BUY · BUREAU" } else { "BUY · STRAYS" }, 13.0, team_color, &kit.text));
                        t.spawn(Node { align_items: AlignItems::Baseline, column_gap: vh(16.0), ..default() }).with_children(|r| {
                            r.spawn(kit_text("ARMORY", 52.0, TX1, &kit.display));
                            r.spawn(kit_text("武器庫", 13.0, TX3, &kit.jp));
                        });
                    });
                    h.spawn(Node { margin: UiRect::left(Val::Auto), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: vh(8.0), ..default() }).with_children(|f| {
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
            let mut cats = root.spawn(abs(Node { left: vh(96.0), top: vh(top), width: vh(360.0), flex_direction: FlexDirection::Column, ..default() }));
            if let Some(a) = tune(opening, 1) {
                cats.insert(a);
            }
            cats.with_children(|col| {
                let nums: Vec<String> = (1..=layout.len()).map(|i| format!("{i:02}")).collect();
                nav(col, &kit, "buy.cat", layout.iter().enumerate().map(|(i, e)| NavItem { num: &nums[i], label: e.0, jp: "", action: UiAction::BuyCategory(i), selected: i == cat }).collect(), true);
                col.spawn((kit_text("1-9 · BUY    ENTER · BUY SELECTED    0 · CLOSE", 13.0, TX3, &kit.text), Node { margin: UiRect::top(vh(24.0)), padding: UiRect::left(vh(16.0)), ..default() }));
            });
            // Cards, two columns.
            root.spawn(abs(Node { left: vh(496.0), width: vh(820.0), top: vh(top), flex_wrap: FlexWrap::Wrap, column_gap: vh(16.0), row_gap: vh(16.0), align_content: AlignContent::FlexStart, ..default() })).with_children(|grid| {
                for (i, (name, item)) in items.iter().enumerate() {
                    let ok = afford(*item);
                    let selected = i == sel;
                    let base = if selected { BG1.mix(&AMBER, 0.08) } else { BG1 };
                    let mut card = grid.spawn(crate::ui::card(Node { width: vh(402.0), height: vh(200.0), flex_direction: FlexDirection::Column, padding: UiRect::axes(vh(24.0), vh(20.0)), ..default() }, selected));
                    if let Some(a) = tune(new_cat, i + 2) {
                        card.insert(a);
                    }
                    card.with_children(|c| {
                        c.spawn(Node { justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|top| {
                            top.spawn((kit_text(format!("{}", i + 1), 13.0, if selected { AMBER } else { TX2 }, &kit.text), Pickable::IGNORE));
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
            root.spawn((abs(Node { left: vh(1356.0), right: vh(96.0), top: vh(top), bottom: vh(96.0), ..default() }), BackgroundColor(BG1)))
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
                        primary_button(d, &kit, &format!("Buy · {}", cost(item)), "Enter", UiAction::Buy(item));
                    } else {
                        primary_off(d, &kit, &format!("Buy · {}", cost(item)), &format!("Need ${}", price(item).unwrap_or(0) - money));
                    }
                    });
                });
        });
}

/// The picture for an item: gun art, or the HUD icon for armor and the kit.
fn item_art(p: &mut ChildSpawnerCommands, icons: &Icons, item: BuyItem, height: f32, color: Color) {
    let name = match item {
        BuyItem::Weapon(id) => gun_art(id),
        BuyItem::Kevlar => Some("suit_full"),
        BuyItem::KevlarHelmet => Some("suithelmet_full"),
        BuyItem::DefuseKit => Some("defuser"),
    };
    let h = if matches!(item, BuyItem::Weapon(_)) { height } else { height * 0.8 };
    if let Some(n) = name {
        p.spawn((icons.node(n, h, color), Pickable::IGNORE));
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
