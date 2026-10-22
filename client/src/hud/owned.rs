//! What you carry, as icons over the weapon panel (bottom right): primary, pistol, knife,
//! each grenade with its count, the C4 (red inside a bomb site: you can plant), the defuse
//! kit. The one in your hands is bright, the rest dimmed, so you know your kit without cycling
//! through it. Hidden while you watch someone (their
//! inventory isn't sent).
use super::{weapon_icon, Icons};
use crate::game::Walker;
use crate::ui::{kit_text, vh, Kit, BAD, TX1, TX2};
use bevy::prelude::*;
use sim::weapons::{Id, Slot};

/// The strip's row.
#[derive(Component)]
pub(super) struct Owned;

/// One thing carried: its icon, how many, and its colour (bright in your hands).
type Item = (&'static str, u8, Color);

/// The carried items, in the order of their number keys.
fn carried(w: &Walker) -> Vec<Item> {
    let l = &w.client.state.loadout;
    let tint = |active: bool| if active { TX1 } else { TX2.with_alpha(0.85) };
    let mut out = Vec::new();
    if let Some(g) = &l.primary {
        out.push((weapon_icon(g.id), 1, tint(l.active == Slot::Primary)));
    }
    if let Some(g) = &l.secondary {
        out.push((weapon_icon(g.id), 1, tint(l.active == Slot::Secondary)));
    }
    if l.knife {
        out.push((weapon_icon(Id::Knife), 1, tint(l.active == Slot::Knife)));
    }
    let in_hand = (l.active == Slot::Grenade).then(|| l.grenade_id()).flatten();
    for (i, id) in [Id::HeGrenade, Id::Flashbang, Id::Smoke].into_iter().enumerate() {
        if l.grenades[i] > 0 {
            out.push((weapon_icon(id), l.grenades[i], tint(in_hand == Some(id))));
        }
    }
    if l.c4 {
        let in_site = w.client.world.zones.in_bomb_site(w.client.me().origin, w.client.me().hull());
        out.push((weapon_icon(Id::C4), 1, if in_site { BAD } else { tint(l.active == Slot::Bomb) }));
    }
    if w.client.me_info.defuse_kit {
        out.push(("defuser", 1, tint(false)));
    }
    out
}

/// Rebuilds the row when what you carry, or what's in your hands, changes.
pub(super) fn owned(mut commands: Commands, w: Res<Walker>, watch: Res<crate::spectate::Watch>, kit: Res<Kit>, icons: Res<Icons>, row: Single<Entity, With<Owned>>, mut shown: Local<Option<Vec<Item>>>) {
    let watching = crate::spectate::spectating(&w) && crate::spectate::target(&w, &watch).is_some();
    let items = if watching { Vec::new() } else { carried(&w) };
    if shown.as_ref() == Some(&items) {
        return;
    }
    *shown = Some(items.clone());
    commands.entity(*row).despawn_related::<Children>().with_children(|r| {
        for (icon, n, color) in items {
            r.spawn(Node { align_items: AlignItems::Center, column_gap: vh(4.0), ..default() }).with_children(|c| {
                // long guns shrink to the same footprint as the rest
                c.spawn(icons.node_in(icon, 26.0, 3.2, color));
                if n > 1 {
                    c.spawn(kit_text(format!("{n}"), 16.0, color, &kit.num));
                }
            });
        }
    });
}

pub(super) fn node() -> impl Bundle {
    (Owned, Node { column_gap: vh(14.0), align_items: AlignItems::Center, ..default() })
}
