//! The map's skybox (worldspawn `skyname`, six TGAs in gfx/env) instead of a flat colour.
use crate::game::FlyCam;
use crate::menu::AppState;
use bevy::light::Skybox;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension};

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, add_sky.run_if(in_state(AppState::InGame)));
    }
}

/// GoldSrc's six faces, in the engine's order (gl_warp.c): rt +X, lf -X, bk +Y, ft -Y, up +Z, dn -Z.
const FACES: [&str; 6] = ["rt", "lf", "bk", "ft", "up", "dn"];

/// Read one face, case-insensitively (maps say "Des", files are "Desbk.tga"), add-on, cstrike, valve.
fn face(sky: &str, suffix: &str) -> Option<goldsrc::tga::Tga> {
    let want = format!("{sky}{suffix}.tga").to_ascii_lowercase();
    for dir in crate::gamefiles::paths("gfx/env") {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().to_ascii_lowercase() == want {
                return goldsrc::tga::decode(&std::fs::read(e.path()).ok()?);
            }
        }
    }
    None
}

/// Where a GoldSrc direction lands on the six faces: (face, s, t) with s, t in -1..1, using
/// the engine's face axes (Quake's `st_to_vec`): sides have t up, s to the right as seen from inside.
fn goldsrc_face(g: [f32; 3]) -> (usize, f32, f32) {
    let [x, y, z] = g;
    let (ax, ay, az) = (x.abs(), y.abs(), z.abs());
    if ax >= ay && ax >= az {
        if x > 0.0 { (0, -y / ax, z / ax) } else { (1, y / ax, z / ax) }
    } else if ay >= az {
        if y > 0.0 { (2, x / ay, z / ay) } else { (3, -x / ay, z / ay) }
    } else if z > 0.0 {
        (4, -y / az, -x / az)
    } else {
        (5, -y / az, x / az)
    }
}

/// The six faces resampled into one cubemap. For every cubemap texel: its direction (WebGPU
/// cube layout, our Y-up space), back to GoldSrc space, and the GoldSrc face pixel there.
fn cubemap(sky: &str) -> Option<Image> {
    let faces: Vec<goldsrc::tga::Tga> = FACES.iter().map(|s| face(sky, s)).collect::<Option<_>>()?;
    let n = faces[0].width as usize;
    if faces.iter().any(|f| f.width as usize != n || f.height as usize != n) {
        return None;
    }
    let mut data = Vec::with_capacity(n * n * 4 * 6);
    for cube in 0..6 {
        for j in 0..n {
            for i in 0..n {
                let u = 2.0 * (i as f32 + 0.5) / n as f32 - 1.0;
                let v = 2.0 * (j as f32 + 0.5) / n as f32 - 1.0;
                let d = match cube {
                    0 => [1.0, -v, -u],
                    1 => [-1.0, -v, u],
                    2 => [u, 1.0, v],
                    3 => [u, -1.0, -v],
                    4 => [u, -v, 1.0],
                    _ => [-u, -v, -1.0],
                };
                // Y-up (x, y, z) is GoldSrc (x, -z, y).
                let (f, s, t) = goldsrc_face([d[0], -d[2], d[1]]);
                let px = (((s + 1.0) * 0.5) * (n - 1) as f32).round() as usize;
                let py = (((1.0 - t) * 0.5) * (n - 1) as f32).round() as usize;
                let k = (py.min(n - 1) * n + px.min(n - 1)) * 4;
                data.extend_from_slice(&faces[f].rgba[k..k + 4]);
            }
        }
    }
    let n = n as u32;
    let mut img = Image::new(Extent3d { width: n, height: n, depth_or_array_layers: 6 }, TextureDimension::D2, data, TextureFormat::Rgba8UnormSrgb, default());
    img.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
    Some(img)
}

/// Once the camera exists: give it the map's sky.
fn add_sky(mut commands: Commands, cams: Query<Entity, Added<FlyCam>>, map: Res<crate::loading::MapData>, mut images: ResMut<Assets<Image>>) {
    for cam in &cams {
        let sky = Some(&map.bsp)
            .and_then(|b| b.entity_list().into_iter().find(|e| e.get("classname").map(String::as_str) == Some("worldspawn"))?.get("skyname").cloned())
            .unwrap_or_else(|| "desert".into());
        match cubemap(&sky) {
            Some(img) => {
                // Brightness 1: with our exposure the texture shows as it is, like the walls.
                commands.entity(cam).insert(Skybox { image: Some(images.add(img)), brightness: 1.0, ..default() });
            }
            None => warn!("sky {sky}: faces missing"),
        }
    }
}
