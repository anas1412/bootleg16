//! The in-game HUD (Interference, design/mockups/ui "In game"): vitals bottom left, weapon
//! and ammo bottom right, frame rate and ping top right. Every panel is HUD glass with an edge
//! light in your side's colour; `motion` brings them to life (hits, shots, reloads, weapon
//! switches, sway). `crosshair` is the crosshair and the sniper scope, `icons` the install's
//! HUD sprites, `hint` short hints in the low middle. Round state and money are `round_hud`, the kill feed `killfeed`.
mod crosshair;
mod hint;
mod icons;
mod motion;
mod visor;

pub use crate::ui::SAFE;
pub use crosshair::{draw as draw_crosshair, CROSSHAIR_COLORS, STATIC_GAP};
pub use hint::Hint;
pub use visor::Visor;
pub use icons::{gun_art, weapon_icon, Icons};

use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::ui::{edge, hud_panel, kit_text, meter, vh, Count, EdgeColor, HudLayer, Kit, Meter, BAD, EDGE_REST, LINE, TX1, TX2};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use net::protocol::Team;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .init_resource::<Kit>()
            .init_resource::<Icons>()
            .add_systems(OnEnter(AppState::InGame), (spawn, crosshair::spawn))
            .add_systems(Update, (status, crosshair::crosshair, net_stats).in_set(Frame::Ui));
        motion::plugin(app);
        hint::plugin(app);
        visor::plugin(app);
    }
}

/// Health at or under this shows in red.
const LOW_HP: f32 = 25.0;

/// A text the status system keeps up to date.
#[derive(Component, Clone, Copy, PartialEq)]
enum Field {
    Clip,
    Reserve,
    Weapon,
    Grenades,
    Notice,
}
/// Shown only while you're alive in a team.
#[derive(Component)]
struct AliveOnly;
/// The active weapon's silhouette in the ammo panel.
#[derive(Component)]
struct WeaponIcon;
#[derive(Component)]
struct NetStats;
/// The clip and reserve half of the weapon panel (hidden when there's no gun in hand).
#[derive(Component)]
struct AmmoPart;

// The parts `motion` brings to life.
#[derive(Component)]
struct VitalsPanel;
#[derive(Component)]
struct VitalsWash;
#[derive(Component)]
struct VitalsEdge;
#[derive(Component)]
struct HealthNumber;
#[derive(Component)]
struct ArmorNumber;
#[derive(Component)]
struct HealthMeter;
#[derive(Component)]
struct ArmorMeter;
#[derive(Component)]
struct AmmoEdge;
#[derive(Component)]
struct ClipNumber;
/// The weapon's name and silhouette: tunes in on a switch.
#[derive(Component)]
struct WeaponBlock;
/// The reload line on the ammo panel's bottom edge, filling out from the centre.
#[derive(Component)]
struct ReloadLine;
/// HUD groups that sway a little as you turn.
#[derive(Component)]
struct Sway;

fn pos(node: Node) -> Node {
    Node { position_type: PositionType::Absolute, ..node }
}

fn divider() -> impl Bundle {
    (Node { width: px(1), align_self: AlignSelf::Stretch, ..default() }, BackgroundColor(LINE))
}

fn shadowed() -> TextShadow {
    TextShadow { offset: Vec2::splat(1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.9) }
}

fn spawn(mut commands: Commands, kit: Res<Kit>, icons: Res<Icons>) {
    let game = DespawnOnExit(AppState::InGame);
    let section = || Node { padding: UiRect::axes(vh(24.0), vh(16.0)), align_items: AlignItems::Center, column_gap: vh(16.0), ..default() };

    // Bottom left: health | armor.
    commands.spawn((game.clone(), AliveOnly, VitalsPanel, Sway, Visor, hud_panel(pos(Node { left: vh(SAFE), bottom: vh(SAFE), ..default() })))).with_children(|p| {
        p.spawn((VitalsEdge, edge(EdgeColor::Team, EDGE_REST)));
        // a red wash when you're hit, fading out
        p.spawn((VitalsWash, Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), border_radius: BorderRadius::all(vh(crate::ui::RADIUS)), ..default() }, BackgroundColor(Color::NONE)));
        p.spawn(section()).with_children(|s| {
            s.spawn(icons.node("cross", 36.0, TX1));
            s.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|c| {
                c.spawn((HealthNumber, Count::new(100.0, 0.08), kit_text("100", 56.0, TX1, &kit.num)));
                let m = meter(c, 132.0, TX1, 1.0);
                c.commands().entity(m).insert(HealthMeter);
            });
        });
        p.spawn(divider());
        p.spawn(section()).with_children(|s| {
            s.spawn(icons.node("suithelmet_full", 32.0, TX2));
            s.spawn(Node { flex_direction: FlexDirection::Column, ..default() }).with_children(|c| {
                c.spawn((ArmorNumber, Count::new(0.0, 0.08), kit_text("0", 32.0, TX2, &kit.num)));
                let m = meter(c, 72.0, TX2, 0.0);
                c.commands().entity(m).insert(ArmorMeter);
            });
        });
    });

    // Bottom right: weapon | ammo, grenades above.
    commands
        .spawn((game.clone(), AliveOnly, Sway, Visor, pos(Node { right: vh(SAFE), bottom: vh(SAFE), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: vh(8.0), ..default() })))
        .with_children(|col| {
            col.spawn((Field::Grenades, kit_text("", 16.0, TX2, &kit.text), shadowed()));
            col.spawn(hud_panel(Node::default())).with_children(|p| {
                p.spawn((AmmoEdge, edge(EdgeColor::Team, EDGE_REST)));
                p.spawn((ReloadLine, Node { position_type: PositionType::Absolute, left: vh(12.0), right: vh(12.0), bottom: px(0), height: vh(2.0), border_radius: BorderRadius::all(px(1)), ..default() }, BackgroundColor(TX1), UiTransform { scale: Vec2::new(0.0, 1.0), ..default() }, Visibility::Hidden));
                p.spawn((WeaponBlock, Node { width: vh(184.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::Center, row_gap: vh(10.0), padding: UiRect::axes(vh(24.0), vh(16.0)), ..default() })).with_children(|s| {
                    s.spawn((Field::Weapon, kit_text("", 13.0, TX1, &kit.text), TextLayout::no_wrap()));
                    s.spawn((WeaponIcon, icons.node("d_knife", 36.0, TX2)));
                });
                p.spawn((AmmoPart, divider()));
                p.spawn((AmmoPart, Node { align_items: AlignItems::Baseline, column_gap: vh(10.0), padding: UiRect::axes(vh(24.0), vh(16.0)), ..default() })).with_children(|s| {
                    s.spawn((Field::Clip, ClipNumber, kit_text("", 56.0, TX1, &kit.num)));
                    s.spawn((Field::Reserve, kit_text("", 18.0, TX2, &kit.num)));
                });
            });
        });

    // Top right, over the kill feed: frame rate and ping.
    commands.spawn((game.clone(), HudLayer, NetStats, kit_text("", 13.0, TX2, &kit.num), shadowed(), pos(Node { right: vh(SAFE), top: vh(SAFE), ..default() })));

    // Bottom centre, above the assistant: what's going on (spectating, dead).
    commands.spawn((game, HudLayer, pos(Node { width: percent(100), bottom: vh(260.0), justify_content: JustifyContent::Center, ..default() }))).with_children(|row| {
        row.spawn((Field::Notice, kit_text("", 20.0, TX1, &kit.text), shadowed()));
    });
}

/// The kill feed starts under the frame rate and ping (top right).
pub const FEED_TOP: f32 = SAFE + 24.0;
/// The voice list starts 16 under the money panel (top left).
pub const VOICES_TOP: f32 = SAFE + crate::radar::SIZE + 8.0 + 48.0 + 16.0;

#[allow(clippy::type_complexity)]
fn status(
    w: Res<Walker>,
    icons: Res<Icons>,
    mut fields: Query<(&Field, &mut Text)>,
    mut health: Single<(&mut Count, &mut TextColor), (With<HealthNumber>, Without<ArmorNumber>)>,
    mut armor: Single<&mut Count, (With<ArmorNumber>, Without<HealthNumber>)>,
    mut hm: Single<&mut Meter, (With<HealthMeter>, Without<ArmorMeter>)>,
    mut am: Single<&mut Meter, (With<ArmorMeter>, Without<HealthMeter>)>,
    mut icon: Single<&mut ImageNode, With<WeaponIcon>>,
    mut alive_only: Query<&mut Visibility, With<AliveOnly>>,
    covered: Res<crate::ui::Covered>,
    bursting: Query<(), (With<VitalsPanel>, With<crate::ui::Anim>)>,
    watch: Res<crate::spectate::Watch>,
    mut ammo_parts: Query<&mut Node, With<AmmoPart>>,
    reconnecting: Option<Res<crate::game::Reconnecting>>,
) {
    let mut v = w.client.vitals;
    let l = &w.client.state.loadout;
    // spectating: the HUD shows the player you watch (health, armor, weapon and ammo)
    let watched = crate::spectate::spectating(&w).then(|| crate::spectate::target(&w, &watch)).flatten().filter(|t| t.mate.is_some());
    if let Some(m) = watched.as_ref().and_then(|t| t.mate) {
        v.health = m.health as f32;
        v.armor = m.armor as f32;
    }
    let weapon = watched.as_ref().map(|t| t.weapon).unwrap_or(l.active_id());
    let alive = (w.client.vitals.alive() && w.client.id().is_some() && w.client.team() != Team::Spectator) || watched.is_some();
    for mut vis in &mut alive_only {
        vis.set_if_neq(if alive && !covered.0 { Visibility::Inherited } else { Visibility::Hidden });
    }
    let low = v.health <= LOW_HP;
    health.0.target = v.health;
    // while the panel bursts, the animation owns its colours
    if bursting.is_empty() {
        health.1.set_if_neq(TextColor(if low { BAD } else { TX1 }));
    }
    hm.value = v.health / 100.0;
    hm.color = if low { BAD } else { TX1 };
    armor.target = v.armor;
    am.value = v.armor / 100.0;
    // (magazine, reserve) of the gun in hand: yours, or the watched teammate's.
    let ammo = match &watched {
        Some(t) => t.mate.and_then(|m| m.ammo).map(|(c, r)| (c as u32, r as u32)),
        None => l.active_gun().map(|g| (g.clip, g.reserve)),
    };
    for mut n in &mut ammo_parts {
        let d = if watched.is_some() && ammo.is_none() { Display::None } else { Display::Flex };
        if n.display != d {
            n.display = d;
        }
    }
    let [he, fl, sm] = if watched.is_some() { [0; 3] } else { l.grenades };
    for (field, mut t) in &mut fields {
        let s = match field {
            Field::Clip => ammo.map(|(c, _)| c.to_string()).unwrap_or_default(),
            Field::Reserve => ammo.map(|(_, r)| format!("/ {r}")).unwrap_or_default(),
            Field::Weapon => sim::weapons::def(weapon).name.to_uppercase(),
            Field::Grenades => [("HE", he), ("FLASH", fl), ("SMOKE", sm)].iter().filter(|(_, n)| *n > 0).map(|(name, n)| if *n > 1 { format!("{name} x{n}") } else { name.to_string() }).collect::<Vec<_>>().join("   "),
            Field::Notice => match w.client.team() {
                _ if reconnecting.is_some() => String::new(), // the reconnecting panel says it
                _ if w.client.id().is_none() => "Connecting...".into(),
                _ if watched.is_some() => String::new(), // the spectate plate says who
                Team::Spectator => "Spectating  ·  press M to choose a team".into(),
                _ if !v.alive() => if w.client.practice { "Respawning..." } else { "You are dead" }.into(),
                _ => String::new(),
            },
        };
        if t.0 != s {
            t.0 = s;
        }
    }
    // Swap the weapon silhouette when the weapon changes.
    let (wanted, _) = icons::icons_rect(&icons, weapon_icon(weapon));
    if icon.rect != wanted {
        icon.rect = wanted;
    }
}

/// "120 FPS   25 MS", refreshed a few times a second so it's readable.
fn net_stats(w: Res<Walker>, diag: Res<DiagnosticsStore>, time: Res<Time>, mut next: Local<f32>, mut text: Single<&mut Text, With<NetStats>>) {
    if time.elapsed_secs() < *next {
        return;
    }
    *next = time.elapsed_secs() + 0.25;
    let fps = diag.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    let s = format!("{fps:.0} FPS   {:.0} MS", w.client.ping_ms);
    if text.0 != s {
        text.0 = s;
    }
}
