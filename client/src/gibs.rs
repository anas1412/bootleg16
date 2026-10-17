//! Breakables breaking into gibs, as GoldSrc's `TE_BREAKMODEL`: the material's gib model
//! (`func_breakable`'s `gibmodel`, else glass/wood/metal... shards), as many pieces as fit
//! the brush ((xy + yz + zx) / (3 x 12²), up to 100), each a random sub-model somewhere in
//! its box, falling at half gravity with a little random push, most of them spinning,
//! bouncing off the world, gone after 2.5 s (plus up to one). Glass shards are see-through.
use crate::effects::GameNews;
use crate::game::{Frame, Walker};
use crate::map;
use crate::menu::AppState;
use crate::models::{self, ModelCache};
use crate::studio::StudioMaterial;
use bevy::prelude::*;
use net::protocol::GameEvent;
use sim::breakables::Material;
use std::collections::HashMap;

pub struct GibsPlugin;

impl Plugin for GibsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), load).add_systems(Update, (spawn, fly).chain().in_set(Frame::World).run_if(resource_exists::<Breakables>));
    }
}

/// Each breakable's box (world) and its own gib model, if it names one.
#[derive(Resource)]
struct Breakables {
    boxes: HashMap<u16, ([f32; 3], [f32; 3], Option<String>)>,
    rng: u64,
}

impl Breakables {
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % 100_000) as f32 / 100_000.0
    }
}

#[derive(Component)]
struct Gib {
    vel: Vec3,
    spin: Vec3,
    left: f32,
}

/// GoldSrc's gib model for each material (`CBreakable::Precache`).
fn gib_model(m: Material) -> &'static str {
    use Material::*;
    match m {
        Glass | UnbreakableGlass => "glassgibs.mdl",
        Wood => "woodgibs.mdl",
        Metal => "metalplategibs.mdl",
        Flesh => "fleshgibs.mdl",
        Computer => "computergibs.mdl",
        Concrete => "cindergibs.mdl",
        Rocks => "rockgibs.mdl",
        CeilingTile => "ceilinggibs.mdl",
    }
}

fn load(mut commands: Commands, map: Res<crate::loading::MapData>) {
    let bsp = &map.bsp;
    let mut boxes = HashMap::new();
    for e in bsp.entity_list() {
        if e.get("classname").map(String::as_str) != Some("func_breakable") {
            continue;
        }
        let Some(i) = e.get("model").and_then(|m| m.strip_prefix('*')?.parse::<usize>().ok()) else { continue };
        let Some(m) = bsp.models.get(i) else { continue };
        let gib = e.get("gibmodel").map(|g| g.trim_start_matches("models/").to_string());
        boxes.insert(i as u16, (m.mins, m.maxs, gib));
    }
    commands.insert_resource(Breakables { boxes, rng: 0x51A3_C0DE_77F1_0B25 });
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    mut news: MessageReader<GameNews>,
    mut breakables: ResMut<Breakables>,
    mut cache: ResMut<ModelCache>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StudioMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: Local<HashMap<(String, usize), models::Parts>>,
) {
    for GameNews(e) in news.read() {
        let GameEvent::Broke { model, material, .. } = e else { continue };
        let Some((lo, hi, own)) = breakables.boxes.get(model).cloned() else { continue };
        let file = own.unwrap_or_else(|| gib_model(*material).to_string());
        let Some(gibs) = cache.get(&file, &mut images, &mut materials) else { continue };
        let size: [f32; 3] = std::array::from_fn(|i| (hi[i] - lo[i]).max(1.0));
        let count = ((size[0] * size[1] + size[1] * size[2] + size[2] * size[0]) / (3.0 * 12.0 * 12.0)).clamp(1.0, 100.0) as usize;
        let bodies = gibs.model.body_count();
        let glass = matches!(material, Material::Glass | Material::UnbreakableGlass);
        for _ in 0..count {
            let body = ((breakables.rand() * bodies as f32) as usize).min(bodies - 1);
            let p = parts.entry((file.clone(), body)).or_insert_with(|| {
                let pose = gibs.model.pose(0, 0, 0.0);
                gibs.model.posed_body(&pose, body).iter().filter(|p| !p.indices.is_empty()).map(|p| (meshes.add(models::part_mesh(p, map::to_y_up, false)), gibs.materials.get(p.texture).cloned().unwrap_or_default())).collect()
            });
            let r = |b: &mut Breakables| b.rand() - 0.5;
            let at: [f32; 3] = std::array::from_fn(|i| lo[i] + size[i] * (0.5 + r(&mut breakables)));
            // `random` 10: a small push each way, no blast (`expRandom`).
            let vel = Vec3::from(map::to_y_up([r(&mut breakables) * 20.0, r(&mut breakables) * 20.0, r(&mut breakables) * 20.0]));
            let spin = if breakables.rand() < 200.0 / 256.0 { Vec3::new(r(&mut breakables), r(&mut breakables), r(&mut breakables)) * 512f32.to_radians() } else { Vec3::ZERO };
            let left = 2.5 + breakables.rand();
            let parent = commands.spawn((Gib { vel, spin, left }, Transform::from_translation(Vec3::from(map::to_y_up(at))), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
            models::spawn_lit(&mut commands, parent, p, &mut materials);
            if glass {
                commands.entity(parent).insert(SeeThrough);
            }
        }
    }
}

/// Glass shards: `kRenderTransTexture` at half strength.
#[derive(Component)]
struct SeeThrough;

/// Half gravity, bouncing off the world (losing speed), shrinking away at the end.
fn fly(
    mut commands: Commands,
    time: Res<Time>,
    w: Res<Walker>,
    mut gibs: Query<(Entity, &mut Gib, &mut Transform, &Children, Has<SeeThrough>)>,
    parts: Query<&MeshMaterial3d<StudioMaterial>>,
    mut materials: ResMut<Assets<StudioMaterial>>,
) {
    let dt = time.delta_secs().min(0.05);
    for (e, mut g, mut tf, children, glass) in &mut gibs {
        g.left -= dt;
        if g.left <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        g.vel.y -= 400.0 * dt; // sv_gravity x 0.5
        let from = tf.translation;
        let to = from + g.vel * dt;
        let (a, b) = ([from.x, -from.z, from.y], [to.x, -to.z, to.y]);
        let t = w.client.world.trace_line(a, b);
        if t.fraction < 1.0 && !t.start_solid {
            let n = Vec3::from(map::to_y_up(t.normal));
            g.vel = (g.vel - 2.0 * g.vel.dot(n) * n) * 0.5;
            g.spin *= 0.5;
            tf.translation = Vec3::from(map::to_y_up(t.end)) + n * 0.5;
        } else {
            tf.translation = to;
        }
        let spin = g.spin * dt;
        tf.rotate(Quat::from_euler(EulerRot::XYZ, spin.x, spin.y, spin.z));
        let fade = (g.left / 0.5).min(1.0);
        tf.scale = Vec3::splat(fade.max(0.01));
        if glass {
            for c in children.iter() {
                if let Some(mut m) = parts.get(c).ok().and_then(|m| materials.get_mut(&m.0)) {
                    if m.base.alpha_mode != AlphaMode::Blend {
                        m.base.alpha_mode = AlphaMode::Blend;
                        m.base.base_color = Color::WHITE.with_alpha(0.5);
                    }
                }
            }
        }
    }
}
