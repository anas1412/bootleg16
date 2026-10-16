//! Shared look for menus and overlays: dark glass panels, orange accent, buttons with
//! hover highlight. Buttons report clicks as `UiAction`s for the menu systems.
use bevy::prelude::*;
use bevy::text::FontSize;

pub const TEXT: Color = Color::srgb(0.93, 0.94, 0.96);
pub const MUTED: Color = Color::srgb(0.58, 0.62, 0.70);
pub const ACCENT: Color = Color::srgb(1.0, 0.56, 0.12);
pub const PANEL: Color = Color::srgba(0.07, 0.08, 0.11, 0.88);
const BUTTON: Color = Color::srgba(1.0, 1.0, 1.0, 0.05);
const BUTTON_HOVER: Color = Color::srgba(1.0, 0.56, 0.12, 0.20);
pub const T_COLOR: Color = Color::srgb(0.92, 0.66, 0.25);
pub const CT_COLOR: Color = Color::srgb(0.45, 0.68, 1.0);

/// Something a button asks for. Menus read these from `UiActions` each frame.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    Page(crate::menu::Page),
    PickMap(String),
    Dummies(i32),
    StartPractice,
    Connect,
    Sensitivity(f32),
    Volume(f32),
    FpsMax(i32),
    ToggleVsync,
    Quit,
    Resume,
    Leave,
    ChooseTeam(Option<net::protocol::Team>),
    ShowTeams,
    Buy(net::protocol::BuyItem),
    BuyCategory(usize),
    CloseBuy,
}

#[derive(Resource, Default)]
pub struct UiActions(pub Vec<UiAction>);

pub fn text(s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (Text::new(s), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color), TextShadow { offset: Vec2::splat(1.5), color: Color::srgba(0.0, 0.0, 0.0, 0.8) })
}

/// A glass panel node.
pub fn panel(node: Node) -> impl Bundle {
    (Node { border_radius: BorderRadius::all(px(10)), ..node }, BackgroundColor(PANEL), BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.08)))
}

/// A wide menu button. `selected` draws it highlighted (current map, current page).
pub fn button(parent: &mut ChildSpawnerCommands, label: &str, action: UiAction, selected: bool) {
    let base = if selected { BUTTON_HOVER } else { BUTTON };
    parent
        .spawn((
            Node {
                padding: UiRect::axes(px(18), px(10)),
                margin: UiRect::vertical(px(3)),
                border: UiRect::left(px(3)),
                min_width: px(220),
                border_radius: BorderRadius::all(px(6)),
                ..default()
            },
            BackgroundColor(base),
            BorderColor::all(if selected { ACCENT } else { Color::NONE }),
        ))
        .with_children(|b| {
            b.spawn((text(label, 20.0, TEXT), Pickable::IGNORE));
        })
        .observe(|ev: On<PointerOver>, mut q: Query<(&mut BackgroundColor, &mut BorderColor)>| {
            if let Ok((mut bg, mut border)) = q.get_mut(ev.entity) {
                bg.0 = BUTTON_HOVER;
                *border = BorderColor::all(ACCENT);
            }
        })
        .observe(move |ev: On<PointerOut>, mut q: Query<(&mut BackgroundColor, &mut BorderColor)>| {
            if let Ok((mut bg, mut border)) = q.get_mut(ev.entity) {
                bg.0 = base;
                *border = BorderColor::all(if selected { ACCENT } else { Color::NONE });
            }
        })
        .observe(move |_: On<PointerClick>, mut actions: ResMut<UiActions>| actions.0.push(action.clone()));
}
