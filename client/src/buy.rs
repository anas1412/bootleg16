//! The buy menu (B): CS 1.6 categories and keys (number picks, 0 closes), also clickable.
//! `,` / `.` buy primary / secondary ammo without the menu.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::ui::{button, panel, text, UiAction, UiActions, CT_COLOR, MUTED, T_COLOR};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use net::protocol::{BuyItem, Team};

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
pub struct BuyMenu(Option<Option<usize>>);

impl BuyMenu {
    pub fn is_open(&self) -> bool {
        self.0.is_some()
    }
}

/// One buy menu entry.
#[derive(Clone)]
enum Entry {
    /// Buys straight away from the category list (6 and 7: ammo).
    Direct(&'static str, BuyItem),
    Category(&'static str, Vec<(&'static str, BuyItem)>),
}

/// Terrorists and counter-terrorists see different guns.
fn layout(ct: bool) -> Vec<Entry> {
    use sim::weapons::Id::*;
    use BuyItem::*;
    let side = |t: (&'static str, sim::weapons::Id), c: (&'static str, sim::weapons::Id)| if ct { (c.0, Weapon(c.1)) } else { (t.0, Weapon(t.1)) };
    let mut equipment = vec![("Kevlar", Kevlar), ("Kevlar + Helmet", KevlarHelmet), ("Flashbang", Weapon(Flashbang)), ("HE Grenade", Weapon(HeGrenade)), ("Smoke Grenade", Weapon(Smoke))];
    if ct {
        equipment.push(("Defuse Kit", DefuseKit));
    }
    equipment.push(("NightVision", NightVision));
    vec![
        Entry::Category(
            "Pistols",
            vec![("Glock-18", Weapon(Glock)), ("USP", Weapon(Usp)), ("P228", Weapon(P228)), ("Desert Eagle", Weapon(Deagle)), side(("Dual Elites", Elite), ("Five-SeveN", FiveSeven))],
        ),
        Entry::Category("Shotguns", vec![("M3", Weapon(M3)), ("XM1014", Weapon(Xm1014))]),
        Entry::Category("Sub-Machine Guns", vec![side(("MAC-10", Mac10), ("TMP", Tmp)), ("MP5", Weapon(Mp5)), ("UMP45", Weapon(Ump45)), ("P90", Weapon(P90))]),
        Entry::Category(
            "Rifles",
            if ct {
                vec![("FAMAS", Weapon(Famas)), ("Scout", Weapon(Scout)), ("M4A1", Weapon(M4a1)), ("AUG", Weapon(Aug)), ("SG550", Weapon(Sg550)), ("AWP", Weapon(Awp))]
            } else {
                vec![("Galil", Weapon(Galil)), ("AK-47", Weapon(Ak47)), ("Scout", Weapon(Scout)), ("SG552", Weapon(Sg552)), ("AWP", Weapon(Awp)), ("G3SG1", Weapon(G3sg1))]
            },
        ),
        Entry::Category("Machine Gun", vec![("M249", Weapon(M249))]),
        Entry::Direct("Primary Ammo", PrimaryAmmo),
        Entry::Direct("Secondary Ammo", SecondaryAmmo),
        Entry::Category("Equipment", equipment),
    ]
}

/// What an item costs (shown in the menu; the server charges).
fn price(item: BuyItem) -> Option<u32> {
    match item {
        BuyItem::Weapon(id) => Some(sim::weapons::def(id).price),
        BuyItem::Kevlar => Some(650),
        BuyItem::KevlarHelmet => Some(1000),
        BuyItem::DefuseKit => Some(200),
        BuyItem::NightVision => Some(1250),
        BuyItem::PrimaryAmmo | BuyItem::SecondaryAmmo => None,
    }
}

#[derive(Component)]
struct BuyRoot;

fn spawn(mut menu: ResMut<BuyMenu>) {
    menu.0 = None;
}

/// B opens / closes; with the menu open the mouse is free to click, and number keys work as in CS.
#[allow(clippy::too_many_arguments)]
fn buy_menu(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut actions: ResMut<UiActions>,
    mut w: ResMut<Walker>,
    mut menu: ResMut<BuyMenu>,
    mut shown: Local<Option<(Option<Option<usize>>, Team)>>,
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
    if keys.just_pressed(KeyCode::Comma) {
        w.client.buy(BuyItem::PrimaryAmmo);
    }
    if keys.just_pressed(KeyCode::Period) {
        w.client.buy(BuyItem::SecondaryAmmo);
    }
    // Number keys become the same actions as clicks.
    let digits = [KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];
    if keys.just_pressed(KeyCode::KeyB) && team != Team::Spectator {
        menu.0 = if menu.0.is_some() { None } else { Some(None) };
    } else if let (Some(state), Some(n)) = (menu.0, digits.iter().position(|k| keys.just_pressed(*k))) {
        let i = n.wrapping_sub(1);
        actions.0.push(match (n, state) {
            (0, _) => UiAction::CloseBuy,
            (_, None) => match layout.get(i) {
                Some(Entry::Direct(_, item)) => UiAction::Buy(*item),
                _ => UiAction::BuyCategory(i),
            },
            (_, Some(cat)) => match layout.get(cat) {
                Some(Entry::Category(_, items)) if i < items.len() => UiAction::Buy(items[i].1),
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
            match layout.get(*i) {
                Some(Entry::Category(..)) => menu.0 = Some(Some(*i)),
                Some(Entry::Direct(_, item)) => {
                    w.client.buy(*item);
                    menu.0 = None;
                }
                None => {}
            }
            false
        }
        UiAction::Buy(item) => {
            w.client.buy(*item);
            menu.0 = None;
            false
        }
        _ => true,
    });
    if team == Team::Spectator {
        menu.0 = None;
    }
    if auto && menu.0.is_none() {
        menu.0 = Some(Some(3)); // screenshot check: show the rifles page
    }
    // Free the mouse while buying; take it back when done.
    if menu.0.is_some() && !was_open {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    } else if menu.0.is_none() && was_open && !auto {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }

    if *shown == Some((menu.0, team)) {
        return;
    }
    *shown = Some((menu.0, team));
    for e in &roots {
        commands.entity(e).despawn();
    }
    let Some(state) = menu.0 else { return };
    let free = w.client.practice;
    let cost = |item: BuyItem| match price(item) {
        _ if free => "free".to_string(),
        Some(p) => format!("${p}"),
        None => String::new(),
    };
    commands
        .spawn((
            BuyRoot,
            DespawnOnExit(AppState::InGame),
            Node { position_type: PositionType::Absolute, left: px(40), top: percent(16), ..default() },
            GlobalZIndex(5),
        ))
        .with_children(|root| {
            root.spawn(panel(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(px(20)), ..default() })).with_children(|p| {
                p.spawn((text(format!("Buy  -  {}", if ct { "Counter-Terrorists" } else { "Terrorists" }), 24.0, if ct { CT_COLOR } else { T_COLOR }), Node { margin: UiRect::bottom(px(10)), ..default() }));
                p.spawn(Node { flex_direction: FlexDirection::Row, column_gap: px(16), ..default() }).with_children(|row| {
                    row.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|col| {
                        for (i, e) in layout.iter().enumerate() {
                            let label = match e {
                                Entry::Direct(n, item) => format!("{}  {n}   {}", i + 1, cost(*item)),
                                Entry::Category(n, _) => format!("{}  {n}", i + 1),
                            };
                            button(col, &label, UiAction::BuyCategory(i), state == Some(i));
                        }
                        button(col, "0  Close", UiAction::CloseBuy, false);
                    });
                    if let Some(Some(Entry::Category(_, items))) = state.map(|c| layout.get(c)) {
                        row.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|col| {
                            for (i, (name, item)) in items.iter().enumerate() {
                                col.spawn(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, ..default() }).with_children(|r| {
                                    button(r, &format!("{}  {name}", i + 1), UiAction::Buy(*item), false);
                                    r.spawn((text(cost(*item), 18.0, MUTED), Node { margin: UiRect::left(px(12)), width: px(64), ..default() }));
                                });
                            }
                        });
                    }
                });
                p.spawn((text(", / .  buy primary / secondary ammo", 14.0, MUTED), Node { margin: UiRect::top(px(10)), ..default() }));
            });
        });
}
