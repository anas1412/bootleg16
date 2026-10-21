//! Smoke clouds the CS2 way: the smoke floods the space around where it popped instead of
//! sitting in a ball. A grid of cells (one every `CELL` units) is filled outward from the pop,
//! never through a wall or a floor, up to `RADIUS` across and `HEIGHT` up, so it pours along
//! corridors and stops at doorways. Each cell is a soft grey puff; cells at the edge of the
//! cloud are thinner, so it reads as one dense body with soft edges, darker underneath. It
//! billows out over the first `BLOOM` seconds, churns slowly, and thins out over the last
//! `FADE`. (The server still hides what's behind it with a sphere: server/visibility.rs.)
use crate::game::{FlyCam, Frame, Walker};
use crate::level::rgba_image;
use crate::menu::AppState;
use crate::{map, sound};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use std::collections::{HashSet, VecDeque};

pub struct SmokePlugin;

impl Plugin for SmokePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load).add_systems(Update, (clouds, puffs).chain().in_set(Frame::World));
    }
}

/// Cell size, how far the smoke reaches from the pop (sideways and up), and the most cells.
const CELL: f32 = 32.0;
const RADIUS: f32 = 160.0;
const HEIGHT: f32 = 130.0;
const MAX_CELLS: usize = 320;
/// Seconds to fill out, and to thin out at the end.
const BLOOM: f32 = 1.6;
const FADE: f32 = 3.0;

#[derive(Resource)]
struct Looks {
    quad: Handle<Mesh>,
    puff: Handle<Image>,
}

/// A soft puff: alpha falls off smoothly to the edge, lumpy, with a little inner texture.
fn puff_image(n: u32) -> Image {
    let mut px = Vec::with_capacity((n * n * 4) as usize);
    let c = n as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let ang = dy.atan2(dx);
            let lump = 1.0 + 0.07 * (ang * 5.0).sin() + 0.05 * (ang * 9.0 + 2.0).cos();
            let r = (dx * dx + dy * dy).sqrt() / c / lump;
            let grain = 0.92 + 0.08 * ((x as f32 * 0.37).sin() * (y as f32 * 0.41).cos());
            let a = (1.0 - r).clamp(0.0, 1.0).powf(1.3) * grain;
            px.extend_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    rgba_image(n, n, px, false, TextureFormat::Rgba8UnormSrgb)
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(Looks { quad: meshes.add(Rectangle::new(1.0, 1.0)), puff: images.add(puff_image(64)) });
}

/// One cloud, keyed by where it popped.
#[derive(Component)]
struct Cloud([i32; 3]);

/// One cell's puff: where (relative to the cloud, Y-up), when it fills in (seconds after the
/// pop), its size, how solid it is (edges thinner), its churn phase, and its own material.
#[derive(Component)]
struct Puff {
    home: Vec3,
    arrive: f32,
    size: f32,
    solid: f32,
    phase: f32,
    material: Handle<StandardMaterial>,
}

/// The cells the smoke fills from `pop` (GoldSrc), in the order it reaches them: (offset from
/// the pop, steps from it, open neighbours out of six). `open` says whether a cell's middle is
/// free and `clear` whether the smoke can pass from one cell to the next.
pub(crate) fn flood(pop: [f32; 3], open: impl Fn([f32; 3]) -> bool, clear: impl Fn([f32; 3], [f32; 3]) -> bool) -> Vec<([f32; 3], u32, u32)> {
    let at = |c: [i32; 3]| [pop[0] + c[0] as f32 * CELL, pop[1] + c[1] as f32 * CELL, pop[2] + c[2] as f32 * CELL];
    let dirs = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
    let fits = |c: [i32; 3]| {
        let (x, y) = (c[0] as f32 * CELL, c[1] as f32 * CELL);
        (x * x + y * y).sqrt() <= RADIUS && c[2] as f32 * CELL <= HEIGHT && c[2] >= -1
    };
    let mut seen: HashSet<[i32; 3]> = HashSet::from([[0, 0, 0]]);
    let mut order = Vec::new();
    let mut queue = VecDeque::from([([0, 0, 0], 0u32)]);
    while let Some((c, steps)) = queue.pop_front() {
        order.push((c, steps));
        if order.len() >= MAX_CELLS {
            break;
        }
        for d in dirs {
            let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
            if !fits(n) || seen.contains(&n) {
                continue;
            }
            let p = at(n);
            if open(p) && clear(at(c), p) {
                seen.insert(n);
                queue.push_back((n, steps + 1));
            }
        }
    }
    let filled: HashSet<[i32; 3]> = order.iter().map(|(c, _)| *c).collect();
    order
        .iter()
        .map(|(c, steps)| {
            let neighbours = dirs.iter().filter(|d| filled.contains(&[c[0] + d[0], c[1] + d[1], c[2] + d[2]])).count() as u32;
            ([c[0] as f32 * CELL, c[1] as f32 * CELL, c[2] as f32 * CELL], *steps, neighbours)
        })
        .collect()
}

/// Spawn clouds that appeared, remove ones that are gone.
fn clouds(mut commands: Commands, mut w: ResMut<Walker>, looks: Res<Looks>, mut materials: ResMut<Assets<StandardMaterial>>, existing: Query<(Entity, &Cloud)>) {
    let key = |at: [f32; 3]| at.map(|v| v.round() as i32);
    let live: Vec<_> = w.client.smokes.iter().map(|s| (key(s.at), *s)).collect();
    for (e, c) in &existing {
        if !live.iter().any(|(k, _)| *k == c.0) {
            commands.entity(e).despawn();
        }
    }
    for (k, smoke) in live {
        if existing.iter().any(|(_, c)| c.0 == k) {
            continue;
        }
        if smoke.left > sim::grenades::SMOKE_SECONDS as f32 - 1.0 {
            w.cues.push(sound::Cue::Smoke { at: smoke.at }); // just popped (not one we joined late)
        }
        // A little off the floor, so the first cell is in the open.
        let pop = [smoke.at[0], smoke.at[1], smoke.at[2] + 16.0];
        let world = &w.client.world;
        let cells = flood(pop, |p| world.point_contents(p) == sim::collision::Contents::Empty, |a, b| world.trace_line(a, b).fraction >= 1.0);
        let far = cells.iter().map(|c| c.1).max().unwrap_or(1).max(1) as f32;
        let centre = Vec3::from(map::to_y_up(pop));
        let parent = commands.spawn((Cloud(k), Transform::from_translation(centre), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        for (i, (off, steps, neighbours)) in cells.into_iter().enumerate() {
            let f = i as f32;
            let jitter = Vec3::new((f * 1.37).sin(), (f * 0.71).cos() * 0.6, (f * 2.11).sin()) * CELL * 0.3;
            let home = Vec3::from(map::to_y_up(off)) + jitter;
            // darker underneath, lighter on top (CS2's smoke is lit from above)
            let lift = (off[2] / HEIGHT).clamp(0.0, 1.0);
            let grey = 0.62 + 0.18 * lift + 0.04 * (f * 3.3).sin();
            let material = materials.add(StandardMaterial { base_color: Color::srgba(grey, grey, grey * 1.02, 0.0), base_color_texture: Some(looks.puff.clone()), unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
            let puff = Puff {
                home,
                arrive: BLOOM * (steps as f32 / far).powf(0.8),
                size: CELL * (2.3 + 0.5 * ((f * 1.7).sin() * 0.5 + 0.5)),
                solid: if neighbours >= 5 { 0.85 } else { 0.25 + 0.12 * neighbours as f32 },
                phase: f * 0.618,
                material: material.clone(),
            };
            let child = commands.spawn((puff, Mesh3d(looks.quad.clone()), MeshMaterial3d(material), Transform::from_scale(Vec3::ZERO))).id();
            commands.entity(parent).add_child(child);
        }
    }
}

/// Fill in, churn and thin out each puff, always facing the camera.
fn puffs(
    w: Res<Walker>,
    time: Res<Time>,
    cam: Single<&GlobalTransform, With<FlyCam>>,
    clouds: Query<(&Cloud, &Children)>,
    mut puffs: Query<(&Puff, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    let cam_rot = cam.compute_transform().rotation;
    for (cloud, children) in &clouds {
        let Some(s) = w.client.smokes.iter().find(|s| s.at.map(|v| v.round() as i32) == cloud.0) else { continue };
        let age = sim::grenades::SMOKE_SECONDS as f32 - s.left;
        let thin = (s.left / FADE).clamp(0.0, 1.0);
        for c in children {
            let Ok((p, mut tf)) = puffs.get_mut(*c) else { continue };
            let grow = ((age - p.arrive) / 0.6).clamp(0.0, 1.0);
            let grow = 1.0 - (1.0 - grow).powi(3);
            // slow churn: each puff drifts on its own small loop
            let churn = Vec3::new((t * 0.35 + p.phase).sin(), (t * 0.27 + p.phase * 1.3).sin() * 0.5, (t * 0.31 + p.phase * 0.7).cos()) * 7.0;
            tf.translation = p.home * (0.4 + 0.6 * grow) + churn;
            tf.rotation = cam_rot * Quat::from_rotation_z(p.phase + t * 0.05 * (p.phase.sin()));
            tf.scale = Vec3::splat(p.size * (0.5 + 0.5 * grow) * (0.85 + 0.15 * thin));
            let alpha = p.solid * grow * thin;
            if let Some(mut m) = materials.get_mut(&p.material) {
                if (m.base_color.alpha() - alpha).abs() > 0.01 {
                    m.base_color.set_alpha(alpha);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_fills_the_open_and_stops_at_walls() {
        // open everywhere: a capped disc-ish volume, the pop first
        let all = flood([0.0; 3], |_| true, |_, _| true);
        assert_eq!(all[0].0, [0.0; 3]);
        assert!(all.len() == MAX_CELLS);
        assert!(all.iter().all(|(o, _, _)| (o[0] * o[0] + o[1] * o[1]).sqrt() <= RADIUS + 0.1 && o[2] <= HEIGHT + 0.1));
        // a wall at x = 48: nothing beyond it
        let walled = flood([0.0; 3], |p| p[0] < 48.0, |a, b| a[0] < 48.0 && b[0] < 48.0);
        assert!(walled.iter().all(|(o, _, _)| o[0] < 48.0));
        // a corridor 1 cell wide: the smoke pours along it further than it spreads
        let corridor = flood([0.0; 3], |p| p[1].abs() < 1.0 && p[2].abs() < 1.0, |_, _| true);
        assert!(corridor.iter().any(|(o, _, _)| o[0] >= RADIUS - CELL));
    }
}
