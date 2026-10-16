//! A team vote on screen (design/mockups/ui `vote`), under the round score: what it's about
//! (surrender, or kicking a teammate), one segment per teammate (yes fills green from the left,
//! no red from the right), a mark at what it takes, the time left and the F1 / F2 keys.
//! It tunes in when the vote starts, segments burst as votes come in, and it tunes out when
//! the vote ends. The keys are the player's bindings (`binds`); `match_flow::vote` reads them.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::ui::{hud_panel, kit_text, t, vh, Anim, Kit, Type, AMBER, BAD, LINE_2, LINE_3, OK, TX1, TX2};
use bevy::prelude::*;
use net::protocol::VoteNet;

pub struct VotePlugin;

impl Plugin for VotePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show.in_set(Frame::Ui).run_if(resource_exists::<Walker>));
    }
}

/// Under the score and alive ticks.
const TOP: f32 = 150.0;

#[derive(Component)]
struct VoteRoot;
#[derive(Component)]
struct Left;

/// What the panel was drawn with: the vote, minus the clock.
#[derive(PartialEq, Clone, Copy)]
struct Drawn {
    kick: Option<u8>,
    yes: u8,
    no: u8,
    needed: u8,
    total: u8,
}

#[allow(clippy::too_many_arguments)]
fn show(mut commands: Commands, w: Res<Walker>, kit: Res<Kit>, binds: Res<crate::binds::Binds>, roots: Query<Entity, With<VoteRoot>>, mut left: Query<&mut Text, With<Left>>, mut drawn: Local<Option<Drawn>>) {
    let me = w.client.team();
    // yours to answer: your team's, and not about you (the banner tells you that one)
    let vote: Option<VoteNet> = w.client.round.vote.filter(|v| v.team == me && v.kick != w.client.id());
    let want = vote.map(|v| Drawn { kick: v.kick, yes: v.yes, no: v.no, needed: v.needed, total: (w.client.scores.iter().filter(|s| s.team == me).count() as u8).max(v.needed).max(v.yes + v.no) });
    if let Some(v) = vote {
        for mut t in &mut left {
            let s = format!("0:{:02}", v.left.max(0.0).ceil() as u32);
            if t.0 != s {
                t.0 = s;
            }
        }
    }
    if *drawn == want {
        return;
    }
    let before = *drawn;
    *drawn = want;
    for e in &roots {
        if want.is_none() {
            commands.entity(e).remove::<VoteRoot>().insert(Anim::tune_out());
        } else {
            commands.entity(e).despawn();
        }
    }
    let Some(d) = want else { return };
    let what = match d.kick {
        Some(id) => format!("Kick {}", w.client.scores.iter().find(|s| s.id == id).map(|s| s.name.as_str()).unwrap_or("a teammate")),
        None => "Surrender".to_string(),
    };
    let mut root = commands.spawn((VoteRoot, DespawnOnExit(AppState::InGame), crate::ui::HudLayer, Node { position_type: PositionType::Absolute, left: px(0), right: px(0), top: vh(TOP), justify_content: JustifyContent::Center, ..default() }));
    if before.is_none() {
        root.insert(Anim::tune_in(0.0));
    }
    root.with_children(|row| {
        row.spawn(hud_panel(Node { width: vh(460.0), flex_direction: FlexDirection::Column, padding: UiRect { left: vh(20.0), right: vh(20.0), top: vh(14.0), bottom: vh(12.0) }, border: UiRect::all(px(1)), ..default() })).insert(BorderColor::all(LINE_2)).with_children(|p| {
            p.spawn(Node { align_items: AlignItems::Baseline, column_gap: vh(12.0), ..default() }).with_children(|h| {
                h.spawn(t(&kit, Type::Label, "Vote", AMBER));
                h.spawn(kit_text(what.to_uppercase(), 20.0, TX1, &kit.text));
                h.spawn(Node { flex_grow: 1.0, ..default() });
                h.spawn((Left, t(&kit, Type::Label, "", TX2)));
            });
            // one segment per teammate; the mark at what it takes
            p.spawn(Node { column_gap: vh(4.0), margin: UiRect::top(vh(12.0)), ..default() }).with_children(|bar| {
                for i in 0..d.total {
                    let c = if i < d.yes { OK } else if i >= d.total - d.no { BAD } else { LINE_2 };
                    let mut seg = bar.spawn((Node { flex_grow: 1.0, height: vh(8.0), border_radius: BorderRadius::all(px(2)), ..default() }, BackgroundColor(c)));
                    let new = before.is_some_and(|b| (i < d.yes && i >= b.yes) || (i >= d.total - d.no && i < d.total - b.no.min(d.total)));
                    if new {
                        seg.insert(Anim::burst());
                    }
                }
                let at = d.needed as f32 / d.total.max(1) as f32 * 100.0;
                bar.spawn((Node { position_type: PositionType::Absolute, left: percent(at), top: vh(-4.0), bottom: vh(-4.0), width: vh(2.0), margin: UiRect::left(vh(-3.0)), ..default() }, BackgroundColor(TX1)));
            });
            p.spawn(Node { justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Center, margin: UiRect::top(vh(10.0)), ..default() }).with_children(|k| {
                k.spawn(Node { align_items: AlignItems::Center, column_gap: vh(8.0), ..default() }).with_children(|l| {
                    key(l, &kit, &binds.get(crate::binds::Action::VoteYes).label(binds.layout));
                    l.spawn(t(&kit, Type::Label, &format!("Yes · {}", d.yes), TX2));
                });
                k.spawn(t(&kit, Type::Label, &format!("{} needed", d.needed), TX2));
                k.spawn(Node { align_items: AlignItems::Center, column_gap: vh(8.0), ..default() }).with_children(|r| {
                    r.spawn(t(&kit, Type::Label, &format!("No · {}", d.no), TX2));
                    key(r, &kit, &binds.get(crate::binds::Action::VoteNo).label(binds.layout));
                });
            });
        });
    });
}

/// A key cap.
fn key(p: &mut ChildSpawnerCommands, kit: &Kit, k: &str) {
    p.spawn((Node { padding: UiRect::axes(vh(6.0), vh(1.0)), border: UiRect::all(px(1)), border_radius: BorderRadius::all(vh(3.0)), ..default() }, BorderColor::all(LINE_3))).with_children(|c| {
        c.spawn(kit_text(k, 11.0, TX1, &kit.num));
    });
}
