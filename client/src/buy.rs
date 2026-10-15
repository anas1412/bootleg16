//! The CS 1.6 buy menu (B): categories, then items; number keys pick, 0 cancels.
//! `,` / `.` buy primary / secondary ammo without the menu.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use bevy::prelude::*;
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

impl Entry {
    fn name(&self) -> &'static str {
        match self {
            Entry::Direct(n, _) | Entry::Category(n, _) => n,
        }
    }
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

#[derive(Component)]
struct MenuText;

fn spawn(mut commands: Commands, mut menu: ResMut<BuyMenu>) {
    menu.0 = None;
    commands.spawn((
        MenuText,
        Text::new(""),
        TextFont { font_size: bevy::text::FontSize::Px(20.0), ..default() },
        TextColor(Color::srgb(1.0, 0.85, 0.3)),
        Node { position_type: PositionType::Absolute, left: px(24), top: percent(30), ..default() },
        DespawnOnExit(AppState::InGame),
    ));
}

fn buy_menu(keys: Res<ButtonInput<KeyCode>>, mut w: ResMut<Walker>, mut menu: ResMut<BuyMenu>, mut text: Single<&mut Text, With<MenuText>>) {
    let auto = std::env::var_os("BOOTLEG16_SCREENSHOT").is_some();
    if auto && std::env::var_os("BOOTLEG16_MENU").is_none() {
        return;
    }
    let team = w.client.team();
    if team == Team::Spectator {
        menu.0 = None;
    }
    let ct = team == Team::Ct;
    let layout = layout(ct);
    let digits = [KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];
    let pressed = digits.iter().position(|k| keys.just_pressed(*k));
    if keys.just_pressed(KeyCode::Comma) {
        w.client.buy(BuyItem::PrimaryAmmo);
    }
    if keys.just_pressed(KeyCode::Period) {
        w.client.buy(BuyItem::SecondaryAmmo);
    }
    if keys.just_pressed(KeyCode::KeyB) && team != Team::Spectator {
        menu.0 = if menu.0.is_some() { None } else { Some(None) };
    } else if let (Some(state), Some(n)) = (menu.0, pressed) {
        if n == 0 {
            menu.0 = None; // 0. Cancel
        } else {
            let i = n - 1;
            match state {
                None => match layout.get(i) {
                    Some(Entry::Category(..)) => menu.0 = Some(Some(i)),
                    Some(Entry::Direct(_, item)) => {
                        w.client.buy(*item);
                        menu.0 = None;
                    }
                    None => {}
                },
                Some(cat) => {
                    if let Some(Entry::Category(_, items)) = layout.get(cat) {
                        if let Some((_, item)) = items.get(i) {
                            w.client.buy(*item);
                            menu.0 = None;
                        }
                    }
                }
            }
        }
    }
    if auto && menu.0.is_none() {
        menu.0 = Some(Some(3)); // screenshot check: show the rifles page
    }
    text.0 = match menu.0 {
        None => String::new(),
        Some(None) => {
            let mut t = format!("Buy Menu ({})\n", if ct { "CT" } else { "T" });
            for (i, e) in layout.iter().enumerate() {
                t += &format!("\n{}. {}", i + 1, e.name());
            }
            t + "\n\n0. Cancel"
        }
        Some(Some(cat)) => match layout.get(cat) {
            Some(Entry::Category(name, items)) => {
                let mut t = format!("{name}\n");
                for (i, (item, _)) in items.iter().enumerate() {
                    t += &format!("\n{}. {item}", i + 1);
                }
                t + "\n\n0. Cancel"
            }
            _ => String::new(),
        },
    };
}
