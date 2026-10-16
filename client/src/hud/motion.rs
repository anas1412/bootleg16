//! The HUD's life (design/mockups/ui hud.js): it reacts instead of just changing.
//! - Your side colours the HUD: glass tint and edge lights (knife rounds too).
//! - Hit: the vitals panel bursts, washes red and its edge flares red; at 25 health and
//!   under the edge beats like a pulse. Numbers count and meters drain with a trail (`ui`).
//! - Each shot rolls the clip number and flares the ammo edge; it turns red when nearly empty.
//! - Reloading dims the clip while a line fills the bottom edge out from the centre; done, the
//!   full clip rolls in.
//! - A weapon switch tunes the weapon's name and silhouette in.
//! - The HUD sways a few pixels as you turn, and settles.
use super::{AmmoEdge, ClipNumber, ReloadLine, Sway, VitalsEdge, VitalsPanel, VitalsWash, WeaponBlock, LOW_HP};
use crate::game::{FlyCam, Frame, Walker};
use crate::ui::{vh, Anim, Edge, Side, BAD, CT_COLOR, TX1, TX3, T_COLOR};
use bevy::prelude::*;
use net::protocol::Team;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (side, hit, ammo, sway).in_set(Frame::Ui).run_if(resource_exists::<Walker>));
}

/// The clip turns red at or under this share of a full one.
const LOW_CLIP: f32 = 0.2;
/// How long the red hit wash and edge take to fade.
const T_HIT: f32 = 0.52;

fn side(w: Res<Walker>, mut side: ResMut<Side>) {
    let want = match w.client.team() {
        Team::Ct => Some(CT_COLOR),
        Team::T => Some(T_COLOR),
        Team::Spectator => None,
    };
    if side.0 != want {
        side.0 = want;
    }
}

#[allow(clippy::type_complexity)]
fn hit(
    time: Res<Time>,
    w: Res<Walker>,
    mut last: Local<f32>,
    mut hit_at: Local<f32>,
    panel: Single<Entity, With<VitalsPanel>>,
    mut wash: Single<&mut BackgroundColor, With<VitalsWash>>,
    mut edge: Single<&mut Edge, With<VitalsEdge>>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs();
    let health = w.client.vitals.health;
    if health < *last - 0.5 && health > 0.0 {
        *hit_at = now;
        commands.entity(*panel).insert(Anim::burst());
        edge.flare();
    }
    *last = health;
    let t = ((now - *hit_at) / T_HIT).clamp(0.0, 1.0);
    let flash = 1.0 - t;
    wash.0 = BAD.with_alpha(0.24 * flash * flash);
    let low = health > 0.0 && health <= LOW_HP;
    edge.alert = low || t < 1.0;
    edge.beat = low;
}

/// What the ammo panel showed last frame: weapon, clip, reloading.
#[derive(Default, Clone, Copy, PartialEq)]
struct Shown {
    weapon: u8,
    clip: u32,
    reloading: bool,
}

#[allow(clippy::type_complexity)]
fn ammo(
    time: Res<Time>,
    w: Res<Walker>,
    mut last: Local<Option<Shown>>,
    mut reload_from: Local<f32>,
    clip: Single<(Entity, &mut TextColor, Has<Anim>), With<ClipNumber>>,
    block: Single<Entity, With<WeaponBlock>>,
    mut edge: Single<&mut Edge, With<AmmoEdge>>,
    line: Single<(&mut UiTransform, &mut Visibility), With<ReloadLine>>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs();
    let l = &w.client.state.loadout;
    let id = l.active_id();
    let g = l.active_gun();
    let s = Shown { weapon: id as u8, clip: g.map_or(0, |g| g.clip as u32), reloading: g.is_some_and(|g| g.reloading) };
    let (clip_e, mut clip_color, animating) = clip.into_inner();
    if let Some(p) = *last {
        if p.weapon != s.weapon {
            commands.entity(*block).insert(Anim::tune_in(0.0));
            commands.entity(clip_e).insert(Anim::roll(true));
            edge.flare();
        } else if s.clip < p.clip {
            commands.entity(clip_e).insert(Anim::roll(false));
            edge.flare();
        } else if p.reloading && !s.reloading {
            commands.entity(clip_e).insert(Anim::roll(true));
            edge.flare();
        }
        if s.reloading && !p.reloading {
            *reload_from = now;
        }
    }
    *last = Some(s);
    // the reload line fills out from the centre over the weapon's reload time
    let full = sim::weapons::gun(id).map_or(0, |d| d.clip);
    let reload = sim::weapons::gun(id).map_or(1.0, |d| d.reload).max(0.1);
    let (mut tr, mut vis) = line.into_inner();
    vis.set_if_neq(if s.reloading { Visibility::Inherited } else { Visibility::Hidden });
    tr.scale.x = if s.reloading { ((now - *reload_from) / reload).clamp(0.0, 1.0) } else { 0.0 };
    let low = full > 0 && (s.clip as f32) <= full as f32 * LOW_CLIP;
    let want = if s.reloading { TX3 } else if low { BAD } else { TX1 };
    // while it rolls, the animation owns its colour
    if !animating {
        clip_color.set_if_neq(TextColor(want));
    }
}

/// Turning moves the HUD a little against the turn; it springs back.
fn sway(time: Res<Time>, cam: Single<&FlyCam>, mut last: Local<Option<(f32, f32)>>, mut off: Local<Vec2>, mut roots: Query<&mut UiTransform, (With<Sway>, Without<Anim>)>) {
    let dt = time.delta_secs().max(1e-4);
    let (yaw, pitch) = (cam.yaw.to_degrees(), cam.pitch.to_degrees());
    let (ly, lp) = last.unwrap_or((yaw, pitch));
    *last = Some((yaw, pitch));
    let wrap = |d: f32| (d + 180.0).rem_euclid(360.0) - 180.0;
    // degrees per second of turn → a few pixels, capped
    let target = Vec2::new((wrap(yaw - ly) / dt * 0.02).clamp(-8.0, 8.0), (-(pitch - lp) / dt * 0.02).clamp(-6.0, 6.0));
    let k = 1.0 - (-dt / 0.12).exp();
    let cur = *off;
    *off = cur + (target - cur) * k;
    for mut tr in &mut roots {
        tr.translation = Val2::new(vh(off.x), vh(off.y));
    }
}
