//! Kill feed, top right (direction E): killer, weapon icon, headshot, victim; newest at
//! the bottom. Kills you made or took are tinted amber with the amber bar on the outer edge.
//! Rows tune in and out (the shared `ui::feed_row`, like the voice rows).
use crate::game::Frame;
use crate::hud::{weapon_icon, Icons};
use crate::menu::AppState;
use crate::ui::{feed_row, kit_text, vh, Anim, Kit, Outer, CT_COLOR, TX1, T_COLOR};
use bevy::prelude::*;
use net::protocol::Team;

pub struct KillFeedPlugin;

impl Plugin for KillFeedPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<KillFeed>().add_systems(OnEnter(AppState::InGame), spawn).add_systems(Update, (age, show).chain().in_set(Frame::Ui));
    }
}

/// How long a line stays, and how many are shown.
const LIFETIME: f32 = 6.0;
const MAX_LINES: usize = 5;

/// One kill. `killer` is None for falls and the bomb.
#[derive(Clone)]
pub struct Kill {
    pub killer: Option<(String, Team)>,
    pub victim: (String, Team),
    pub weapon: Option<sim::weapons::Id>,
    pub headshot: bool,
    /// You killed or died.
    pub mine: bool,
}

#[derive(Resource, Default)]
pub struct KillFeed {
    lines: Vec<(u32, Kill, f32)>,
    next: u32,
}

impl KillFeed {
    pub fn push(&mut self, kill: Kill) {
        self.lines.push((self.next, kill, LIFETIME));
        self.next += 1;
        let n = self.lines.len();
        if n > MAX_LINES {
            self.lines.drain(..n - MAX_LINES);
        }
    }
}

#[derive(Component)]
struct FeedRoot;

fn spawn(mut commands: Commands) {
    commands.spawn((
        FeedRoot,
        crate::ui::HudLayer,
        DespawnOnExit(AppState::InGame),
        Node { position_type: PositionType::Absolute, right: vh(crate::hud::SAFE), top: vh(crate::hud::FEED_TOP), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: vh(4.0), ..default() },
    ));
}

fn age(time: Res<Time>, mut feed: ResMut<KillFeed>) {
    let dt = time.delta_secs();
    for line in &mut feed.lines {
        line.2 -= dt;
    }
    feed.lines.retain(|l| l.2 > 0.0);
}

fn team_color(team: Team) -> Color {
    match team {
        Team::Ct => CT_COLOR,
        Team::T => T_COLOR,
        Team::Spectator => TX1,
    }
}

/// One row on screen: which kill.
#[derive(Component)]
struct Line(u32);

/// New kills tune in at the bottom; expired ones tune out.
fn show(mut commands: Commands, feed: Res<KillFeed>, kit: Res<Kit>, icons: Res<Icons>, root: Single<Entity, With<FeedRoot>>, lines: Query<(Entity, &Line)>) {
    let live: Vec<u32> = feed.lines.iter().map(|l| l.0).collect();
    for (e, Line(id)) in &lines {
        if !live.contains(id) {
            commands.entity(e).remove::<Line>().insert(Anim::tune_out());
        }
    }
    let shown: Vec<u32> = lines.iter().map(|(_, l)| l.0).collect();
    commands.entity(*root).with_children(|col| {
        for (id, k, _) in feed.lines.iter().filter(|l| !shown.contains(&l.0)) {
            col.spawn((Line(*id), Anim::tune_in(0.0), feed_row(40.0, k.mine, Outer::Right))).with_children(|row| {
                if let Some((name, team)) = &k.killer {
                    row.spawn(kit_text(name.clone(), 16.0, team_color(*team), &kit.num));
                }
                let icon = k.weapon.map(weapon_icon).unwrap_or("d_skull");
                row.spawn(icons.node(icon, 28.0, TX1));
                if k.headshot {
                    row.spawn(icons.node("d_headshot", 28.0, TX1));
                }
                row.spawn(kit_text(k.victim.0.clone(), 16.0, team_color(k.victim.1), &kit.num));
            });
        }
    });
}
