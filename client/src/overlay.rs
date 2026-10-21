//! Menus over the game, in one place: which one is open (`Overlay`: the Esc menu, team
//! selection or the shop), the keys that open and close them (Esc closes whatever is open, or
//! opens the Esc menu; M team selection; B the shop) and the mouse (free while any is open or
//! the scoreboard has it, captured otherwise). Each menu draws itself: the Esc menu and team
//! selection here, the shop in `buy`. They tune in over the blurred game and out when closed.
use crate::game::Walker;
use crate::menu::AppState;
use crate::ui::{card, chip, clickable_keep_border, kit_text, nav, page_title, vh, Anim, Blurs, Kit, NavItem, UiAction, UiActions, BAR, CT_COLOR, TX1, TX2, TX3, T_COLOR};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions, PrimaryWindow};
use net::protocol::Team;

/// The menu open over the game, if any: only one at a time.
#[derive(Resource, Default, PartialEq, Clone, Copy)]
pub enum Overlay {
    #[default]
    None,
    /// The Esc menu. The game keeps running under it: there is no pause, as in CS 1.6.
    Menu,
    Teams,
    Buy,
}

impl Overlay {
    /// It takes the keyboard too. The shop doesn't: you can move (and talk) while buying.
    pub fn takes_keys(self) -> bool {
        matches!(self, Overlay::Menu | Overlay::Teams)
    }
}

#[derive(Component)]
pub(crate) struct OverlayRoot;

/// Opens and closes the menus (see the top of this file), and draws the Esc menu and team
/// selection (team selection also shows when you join).
pub(crate) fn menus(
    mut commands: Commands,
    kit: Res<Kit>,
    keys: Res<ButtonInput<KeyCode>>,
    (mouse, binds): (Res<ButtonInput<MouseButton>>, Res<crate::binds::Binds>),
    mut overlay: ResMut<Overlay>,
    mut shown: Local<Option<Overlay>>,
    w: Res<Walker>,
    mut asked: Local<bool>,
    roots: Query<Entity, With<OverlayRoot>>,
    mut hints: MessageWriter<crate::hud::Hint>,
) {
    let auto = std::env::var_os("BOOTLEG16_SCREENSHOT").is_some();
    if !auto {
        // Esc closes whatever is open, or opens the Esc menu
        if keys.just_pressed(KeyCode::Escape) {
            *overlay = if *overlay == Overlay::None { Overlay::Menu } else { Overlay::None };
        } else if binds.just(crate::binds::Action::Teams, &keys, &mouse) {
            *overlay = if *overlay == Overlay::Teams { Overlay::None } else { Overlay::Teams };
        } else if binds.just(crate::binds::Action::Buy, &keys, &mouse) {
            // B toggles the shop; where you can't buy it says why instead
            match *overlay {
                Overlay::Buy => *overlay = Overlay::None,
                Overlay::None => match crate::buy::why_not(&w) {
                    None => *overlay = Overlay::Buy,
                    // dead or spectating: no reason to give
                    Some(why) if why.is_empty() => {}
                    Some(why) => {
                        hints.write(crate::hud::Hint(why.into()));
                    }
                },
                _ => {}
            }
        }
        // buy time over, left the buy zone, died: the shop goes away
        if *overlay == Overlay::Buy && !w.client.me_info.can_buy {
            *overlay = Overlay::None;
        }
        // First time in: pick a side, like CS.
        if !*asked && w.client.id().is_some() && !w.client.scores.is_empty() && w.client.team() == Team::Spectator {
            *asked = true;
            *overlay = Overlay::Teams;
        }
    } else if !*asked && w.client.id().is_some() {
        // Screenshot runs pick automatically (BOOTLEG16_TEAM=t|ct|spec, default auto).
        *asked = true;
        commands.queue(|world: &mut World| {
            let team = match std::env::var("BOOTLEG16_TEAM").as_deref() {
                Ok("t") => Some(Team::T),
                Ok("ct") => Some(Team::Ct),
                Ok("spec") => Some(Team::Spectator),
                _ => None,
            };
            if let Some(mut w) = world.get_resource_mut::<Walker>() {
                w.client.join_team(team);
            }
        });
    }
    // Screenshot runs can show an overlay (BOOTLEG16_OVERLAY=pause|teams).
    if auto {
        match std::env::var("BOOTLEG16_OVERLAY").as_deref() {
            Ok("pause" | "menu") => *overlay = Overlay::Menu,
            Ok("teams") => *overlay = Overlay::Teams,
            _ => {}
        }
    }
    if *shown == Some(*overlay) {
        return;
    }
    *shown = Some(*overlay);
    // the open overlay tunes out; the blur and scrim fade with it
    for e in &roots {
        commands.entity(e).remove::<(OverlayRoot, Blurs)>().insert(Anim::tune_out());
    }
    let me_team = w.client.team();
    let practice = w.client.practice;
    match *overlay {
        // the shop draws itself (buy.rs)
        Overlay::None | Overlay::Buy => {}
        Overlay::Menu => {
            // a teammate left: you may call a surrender vote
            let abandoned = w.client.round.abandoned[(me_team == Team::Ct) as usize] && me_team != Team::Spectator;
            commands.spawn((OverlayRoot, DespawnOnExit(AppState::InGame), full_dim())).with_children(|root| {
                root.spawn((Node { width: vh(560.0), flex_direction: FlexDirection::Column, ..default() }, Anim::stagger(0))).with_children(|col| {
                    col.spawn(Node { margin: UiRect::bottom(vh(24.0)), flex_direction: FlexDirection::Column, ..default() }).with_children(|t| {
                        page_title(t, &kit, if practice { "Practice" } else { "Match" }, "Menu", "メニュー");
                    });
                    let item = |num, label, jp, action, selected| NavItem { num, label, jp, action, selected };
                    let mut items = vec![item("01", "Resume", "再開", UiAction::Resume, true), item("02", "Choose team", "チーム", UiAction::ShowTeams, false)];
                    if abandoned {
                        items.push(item("03", "Call a surrender vote", "降伏", UiAction::Surrender, false));
                    }
                    items.push(item(if abandoned { "04" } else { "03" }, "Leave match", "退出", UiAction::Leave, false));
                    nav(col, &kit, "pause.nav", items, false);
                    col.spawn((kit_text("ESC · BACK TO THE GAME", 13.0, TX3, &kit.text), Node { margin: UiRect::top(vh(24.0)), padding: UiRect::left(vh(16.0)), ..default() }));
                });
            });
        }
        Overlay::Teams => {
            commands.spawn((OverlayRoot, DespawnOnExit(AppState::InGame), full_dim())).with_children(|root| {
                root.spawn((Node { flex_direction: FlexDirection::Column, ..default() }, Anim::stagger(0))).with_children(|col| {
                    col.spawn(Node { flex_direction: FlexDirection::Column, margin: UiRect::bottom(vh(32.0)), ..default() }).with_children(|t| {
                        page_title(t, &kit, "Pick a side", "Choose your team", "チーム選択");
                    });
                    col.spawn(Node { column_gap: vh(16.0), ..default() }).with_children(|row| {
                        team_card(row, &kit, 1, "1", "Strays", "ストレイズ", "Attack · plant the bomb", T_COLOR, UiAction::ChooseTeam(Some(Team::T)), me_team == Team::T);
                        team_card(row, &kit, 2, "2", "Bureau", "ビューロー", "Defend · defuse the bomb", CT_COLOR, UiAction::ChooseTeam(Some(Team::Ct)), me_team == Team::Ct);
                    });
                    col.spawn((Node { column_gap: vh(8.0), margin: UiRect::top(vh(16.0)), ..default() }, Anim::stagger(3))).with_children(|row| {
                        chip(row, &kit, "5 · Auto-select", UiAction::ChooseTeam(None), false);
                        chip(row, &kit, "6 · Spectate", UiAction::ChooseTeam(Some(Team::Spectator)), me_team == Team::Spectator);
                    });
                });
            });
        }
    }
}

fn full_dim() -> impl Bundle {
    (
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
        Blurs(0.93),
        GlobalZIndex(20),
    )
}

/// One side: key, big name, Japanese name, what you do; the side's colour as a bar on top.
#[allow(clippy::too_many_arguments)]
fn team_card(row: &mut ChildSpawnerCommands, kit: &Kit, i: usize, key: &str, name: &str, jp: &str, role: &str, color: Color, action: UiAction, current: bool) {
    let node = Node { width: vh(420.0), height: vh(280.0), flex_direction: FlexDirection::Column, padding: UiRect::all(vh(28.0)), ..default() };
    let mut e = row.spawn((card(node, current), Anim::stagger(i)));
    let base = if current { crate::ui::BG1.mix(&crate::ui::AMBER, 0.08) } else { crate::ui::BG1 };
    e.with_children(|c| {
        c.spawn((Node { position_type: PositionType::Absolute, left: px(0), right: px(0), top: px(0), height: vh(BAR), ..default() }, BackgroundColor(color), Pickable::IGNORE));
        c.spawn((kit_text(key, 13.0, TX2, &kit.text), Pickable::IGNORE));
        c.spawn((Node { flex_grow: 1.0, ..default() }, Pickable::IGNORE));
        c.spawn((kit_text(name.to_uppercase(), 52.0, color, &kit.display), Pickable::IGNORE));
        c.spawn((kit_text(jp, 13.0, TX3, &kit.jp), Node { margin: UiRect::top(vh(6.0)), ..default() }, Pickable::IGNORE));
        c.spawn((kit_text(role.to_uppercase(), 13.0, if current { TX1 } else { TX2 }, &kit.text), Node { margin: UiRect::top(vh(16.0)), ..default() }, Pickable::IGNORE));
    });
    clickable_keep_border(&mut e, action, base);
}

pub fn in_game_actions(mut actions: ResMut<UiActions>, mut overlay: ResMut<Overlay>, mut w: ResMut<Walker>, mut next: ResMut<NextState<AppState>>, keys: Res<ButtonInput<KeyCode>>) {
    // CS team menu keys.
    if *overlay == Overlay::Teams {
        for (k, team) in [(KeyCode::Digit1, Some(Team::T)), (KeyCode::Digit2, Some(Team::Ct)), (KeyCode::Digit5, None), (KeyCode::Digit6, Some(Team::Spectator))] {
            if keys.just_pressed(k) {
                actions.0.push(UiAction::ChooseTeam(team));
            }
        }
    }
    let mut keep = Vec::new();
    for a in std::mem::take(&mut actions.0) {
        match a {
            UiAction::Resume => *overlay = Overlay::None,
            UiAction::ShowTeams => *overlay = Overlay::Teams,
            UiAction::Leave => next.set(AppState::Menu),
            UiAction::Surrender => {
                w.client.call_surrender();
                *overlay = Overlay::None;
            }
            UiAction::ChooseTeam(team) => {
                w.client.join_team(team);
                *overlay = Overlay::None;
            }
            other => keep.push(other), // buy menu actions are handled by the HUD
        }
    }
    actions.0 = keep;
}

/// A menu over the game has the keyboard (the game and push-to-talk leave it alone).
pub fn menu_has_keys(overlay: Option<Res<Overlay>>) -> bool {
    overlay.is_some_and(|o| o.takes_keys())
}

/// The mouse: free while a menu is open over the game or the scoreboard has it; otherwise
/// captured while the window has focus (a click also captures it).
pub fn cursor(mut cursor: Single<&mut CursorOptions>, window: Single<&Window, With<PrimaryWindow>>, mouse: Res<ButtonInput<MouseButton>>, overlay: Option<Res<Overlay>>, scores: Res<crate::scoreboard::ScoreMouse>) {
    // Automated screenshot runs never take the user's mouse.
    if std::env::var_os("BOOTLEG16_SCREENSHOT").is_some() {
        return;
    }
    let free = overlay.is_some_and(|o| *o != Overlay::None) || scores.on;
    if free {
        if cursor.grab_mode != CursorGrabMode::None {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    } else if cursor.grab_mode == CursorGrabMode::None && (window.focused || mouse.just_pressed(MouseButton::Left)) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
}
