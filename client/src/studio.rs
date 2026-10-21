//! Lighting studio models (players, guns, grenades, the weapon in your hands) the way GoldSrc
//! does: the light under the model (`R_LightPoint`: the lightmap texel a line straight down
//! from its origin meets) gives an ambient and a shade level (Quake's split: ambient up to
//! 128, both together up to 192); the shade falls off away from the sun when the model can
//! see the sky (`light_environment`), else away from straight down. studio.wesl does the
//! per-pixel part; this keeps each model's light up to date as it moves.
use crate::game::{Frame, Walker};
use crate::map;
use crate::menu::AppState;
use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, ShaderType};
use bevy::shader::ShaderRef;
use std::collections::HashMap;

pub type StudioMaterial = ExtendedMaterial<StandardMaterial, StudioLight>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
pub struct StudioLight {
    #[uniform(100)]
    pub light: StudioUniform,
}

#[derive(ShaderType, Reflect, Debug, Clone, Copy, PartialEq)]
pub struct StudioUniform {
    /// Which way the light travels (world, Y-up).
    pub dir: Vec4,
    /// Light colour, brightest channel 1.
    pub color: Vec4,
    /// x ambient, y shade (GoldSrc 0..255 units).
    pub levels: Vec4,
}

impl Default for StudioUniform {
    /// Mid-grey from straight above, until the model's first frame.
    fn default() -> Self {
        StudioUniform { dir: Vec4::new(0.0, -1.0, 0.0, 0.0), color: Vec4::ONE, levels: Vec4::new(96.0, 64.0, 0.0, 0.0) }
    }
}

impl MaterialExtension for StudioLight {
    fn fragment_shader() -> ShaderRef {
        "embedded://client/studio.wesl".into()
    }
}

/// A model's texture as a studio material (masked, lit by `StudioLight`).
pub fn material(image: Handle<Image>) -> StudioMaterial {
    ExtendedMaterial {
        base: StandardMaterial { base_color_texture: Some(image), unlit: true, alpha_mode: AlphaMode::Mask(0.5), ..default() },
        extension: StudioLight::default(),
    }
}

/// A mesh part lit by where its parent stands (each instance has its own material).
#[derive(Component)]
pub struct LitPart;

/// Put on a model's parent when its light should come from where you stand (the view model).
#[derive(Component)]
pub struct LitByMe;

/// Put on a model's parent to brighten it toward white (0..1): a player who can't be hurt yet.
#[derive(Component)]
pub struct Glow(pub f32);

pub struct StudioPlugin;

impl Plugin for StudioPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "studio.wesl");
        app.add_plugins(MaterialPlugin::<StudioMaterial>::default())
            .add_systems(OnEnter(AppState::InGame), load)
            .add_systems(Update, light_models.in_set(Frame::World).run_if(resource_exists::<Probe>));
    }
}

/// One lit world face's lightmap: where its luxels start in the lighting lump and how its
/// texture axes map onto them (16 units a luxel).
struct FaceLight {
    offset: usize,
    s: [f32; 4],
    t: [f32; 4],
    s0: f32,
    t0: f32,
    w: usize,
    h: usize,
}

/// What `R_LightPoint` needs: every world face's lightmap, and the sun.
#[derive(Resource)]
struct Probe {
    faces: Vec<Option<FaceLight>>,
    lighting: Vec<u8>,
    /// The sun's light travels this way (GoldSrc axes), if the map has one.
    sun: Option<[f32; 3]>,
}

fn load(mut commands: Commands, map: Res<crate::loading::MapData>) {
    let bsp = &map.bsp;
    let dot = |v: [f32; 3], a: &[f32; 4]| v[0] * a[0] + v[1] * a[1] + v[2] * a[2] + a[3];
    let faces = bsp
        .faces
        .iter()
        .map(|f| {
            if f.light_offset < 0 || f.styles[0] == 255 {
                return None;
            }
            let ti = bsp.texinfo[f.texinfo as usize];
            let (mut smin, mut smax, mut tmin, mut tmax) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
            for v in bsp.face_vertices(f) {
                let (s, t) = (dot(v, &ti.s), dot(v, &ti.t));
                (smin, smax, tmin, tmax) = (smin.min(s), smax.max(s), tmin.min(t), tmax.max(t));
            }
            let (s0, t0) = ((smin / 16.0).floor(), (tmin / 16.0).floor());
            let w = ((smax / 16.0).ceil() - s0) as usize + 1;
            let h = ((tmax / 16.0).ceil() - t0) as usize + 1;
            Some(FaceLight { offset: f.light_offset as usize, s: ti.s, t: ti.t, s0, t0, w, h })
        })
        .collect();
    // light_environment: `angles` yaw, `pitch` (-90 = straight down).
    let sun = bsp.entity_list().iter().find(|e| e.get("classname").map(String::as_str) == Some("light_environment")).map(|e| {
        let yaw = e.get("angles").and_then(|a| a.split_whitespace().nth(1)?.parse::<f32>().ok()).unwrap_or(0.0).to_radians();
        let pitch = e.get("pitch").and_then(|p| p.parse::<f32>().ok()).unwrap_or(-90.0).to_radians();
        [pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), pitch.sin()]
    });
    commands.insert_resource(Probe { faces, lighting: bsp.lighting.clone(), sun });
}

impl Probe {
    /// The light colour (0..255) at the floor below `origin`, bilinear across luxels.
    fn light_point(&self, world: &sim::collision::World, origin: [f32; 3]) -> Option<[f32; 3]> {
        let end = [origin[0], origin[1], origin[2] - 2048.0];
        let face = world.face_at(origin, end)?;
        let f = self.faces.get(face)?.as_ref()?;
        let hit = world.trace_line(origin, end).end;
        let dot = |a: &[f32; 4]| hit[0] * a[0] + hit[1] * a[1] + hit[2] * a[2] + a[3];
        let (s, t) = ((dot(&f.s) / 16.0 - f.s0).clamp(0.0, (f.w - 1) as f32), (dot(&f.t) / 16.0 - f.t0).clamp(0.0, (f.h - 1) as f32));
        let (x0, y0) = (s.floor() as usize, t.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(f.w - 1), (y0 + 1).min(f.h - 1));
        let (fx, fy) = (s - x0 as f32, t - y0 as f32);
        let px = |x: usize, y: usize, c: usize| self.lighting.get(f.offset + (y * f.w + x) * 3 + c).copied().unwrap_or(0) as f32;
        Some(std::array::from_fn(|c| {
            let top = px(x0, y0, c) * (1.0 - fx) + px(x1, y0, c) * fx;
            let bottom = px(x0, y1, c) * (1.0 - fx) + px(x1, y1, c) * fx;
            top * (1.0 - fy) + bottom * fy
        }))
    }

    /// A model's light at `origin` (GoldSrc coordinates).
    fn light(&self, world: &sim::collision::World, origin: [f32; 3]) -> StudioUniform {
        let c = self.light_point(world, origin).unwrap_or([128.0; 3]);
        let total = c[0].max(c[1]).max(c[2]);
        let ambient = total.min(128.0);
        let shade = total.min(192.0 - ambient);
        let color = if total > 0.0 { Vec4::new(c[0] / total, c[1] / total, c[2] / total, 1.0) } else { Vec4::ONE };
        // Out under the sky: shaded away from the sun; indoors: from straight above.
        let dir = match self.sun {
            Some(sun) if world.trace_line(origin, std::array::from_fn(|i| origin[i] - sun[i] * 8192.0)).hit_sky => sun,
            _ => [0.0, 0.0, -1.0],
        };
        StudioUniform { dir: Vec3::from(map::to_y_up(dir)).extend(0.0), color, levels: Vec4::new(ambient, shade, 0.0, 0.0) }
    }
}

/// The least ambient light (GoldSrc 0..128) on the weapon you hold.
const VIEW_MIN_AMBIENT: f32 = 72.0;

/// Every lit model part gets the light where its parent stands (a few traces per model). The
/// traces run again only once a model has moved, and a material is written only when its light
/// changed: writing one makes the renderer prepare it again.
fn light_models(
    probe: Res<Probe>,
    w: Res<Walker>,
    parts: Query<(&MeshMaterial3d<StudioMaterial>, &ChildOf), With<LitPart>>,
    parents: Query<(&GlobalTransform, Has<LitByMe>, Option<&Glow>)>,
    mut materials: ResMut<Assets<StudioMaterial>>,
    mut traced: Local<HashMap<Entity, ([f32; 3], StudioUniform)>>,
) {
    let mut cache: HashMap<Entity, StudioUniform> = HashMap::new();
    for (mat, parent) in &parts {
        let p = parent.parent();
        let light = *cache.entry(p).or_insert_with(|| {
            let mine = matches!(parents.get(p), Ok((_, true, _)) | Err(_));
            let origin = if mine {
                w.client.me().origin
            } else {
                let v = parents.get(p).map(|(tf, _, _)| tf.translation()).unwrap_or_default();
                [v.x, -v.z, v.y] // back from Y-up to GoldSrc axes
            };
            // trace again only after moving more than a unit (the light under a model barely changes)
            let moved = |o: &[f32; 3]| (0..3).any(|i| (o[i] - origin[i]).abs() > 1.0);
            let mut light = match traced.get(&p) {
                Some((o, l)) if !moved(o) => *l,
                _ => {
                    let l = probe.light(&w.client.world, origin);
                    traced.insert(p, (origin, l));
                    l
                }
            };
            // The weapon in your hands never goes black in a dark corner (its textures are dark already).
            if mine {
                light.levels.x = light.levels.x.max(VIEW_MIN_AMBIENT);
            }
            light.levels.z = parents.get(p).ok().and_then(|(_, _, g)| g).map_or(0.0, |g| g.0);
            light
        });
        if materials.get(&mat.0).is_some_and(|m| m.extension.light != light) {
            if let Some(mut m) = materials.get_mut(&mat.0) {
                m.extension.light = light;
            }
        }
    }
    // forget models that are gone
    if traced.len() > cache.len() {
        traced.retain(|e, _| cache.contains_key(e));
    }
}
