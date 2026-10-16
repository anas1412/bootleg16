//! The radar (top left, direction E): the map's own overview image (`overviews/<map>.bmp`
//! and its `.txt`), centred on you and cropped to a window. On it: your arrow, your
//! teammates (circles), enemies the server shows you (squares; it only sends the ones you
//! can see), and the bomb when it's dropped, planted, or carried by a teammate.
use crate::game::{FlyCam, Frame, Walker};
use crate::hud::SAFE;
use crate::menu::{AppState, GameConfig};
use crate::ui::{vh, AMBER, BG0, CT_COLOR, LINE, T_COLOR};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use net::protocol::{BombNet, Team};

pub struct RadarPlugin;

impl Plugin for RadarPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn).add_systems(Update, update.in_set(Frame::Ui).run_if(resource_exists::<Walker>));
    }
}

/// Radar size at 1080p, and how many game units it shows across.
pub const SIZE: f32 = 256.0;
const SPAN: f32 = 2400.0;
const DOTS: usize = 10;

/// How the overview image maps to the world (GoldSrc overview `.txt`).
#[derive(Resource, Clone, Copy)]
struct Overview {
    zoom: f32,
    origin: [f32; 2],
    rotated: bool,
    size: Vec2,
}

impl Overview {
    /// World (x, y) to image pixels. 1024 px cover 8192 / zoom units, centred on origin.
    fn to_px(&self, x: f32, y: f32) -> Vec2 {
        let k = self.zoom * 1024.0 / 8192.0;
        let (dx, dy) = (x - self.origin[0], y - self.origin[1]);
        let (u, v) = if self.rotated { (dy, dx) } else { (dx, -dy) };
        Vec2::new(self.size.x / 2.0 + u * k, self.size.y / 2.0 + v * k)
    }
}

#[derive(Component)]
struct RadarRoot;
#[derive(Component)]
struct Map;
#[derive(Component)]
struct Me;
#[derive(Component)]
struct Dot(usize);
#[derive(Component)]
struct Bomb;

/// 8-bit paletted BMP (what the overviews are) to RGBA, the green background clear.
fn load_bmp(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let u32_at = |o: usize| Some(u32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?));
    let i32_at = |o: usize| Some(i32::from_le_bytes(data.get(o..o + 4)?.try_into().ok()?));
    if data.get(..2)? != b"BM" || u16::from_le_bytes(data.get(28..30)?.try_into().ok()?) != 8 {
        return None;
    }
    let pixels = u32_at(10)? as usize;
    let (w, h) = (i32_at(18)?.unsigned_abs(), i32_at(22)?);
    let palette = 14 + u32_at(14)? as usize;
    let stride = (w as usize + 3) & !3;
    let mut rgba = vec![0u8; (w * h.unsigned_abs() * 4) as usize];
    for y in 0..h.unsigned_abs() as usize {
        // Rows are stored bottom-up unless the height is negative.
        let src = if h > 0 { h as usize - 1 - y } else { y };
        for x in 0..w as usize {
            let i = *data.get(pixels + src * stride + x)? as usize;
            let c = data.get(palette + i * 4..palette + i * 4 + 3)?;
            let (b, g, r) = (c[0], c[1], c[2]);
            let clear = g > 200 && r < 60 && b < 60;
            rgba[(y * w as usize + x) * 4..][..4].copy_from_slice(&[r, g, b, if clear { 0 } else { 255 }]);
        }
    }
    Some((w, h.unsigned_abs(), rgba))
}

fn parse_txt(text: &str) -> Option<(f32, [f32; 2], bool)> {
    let mut zoom = None;
    let mut origin = None;
    let mut rotated = false;
    for l in text.lines() {
        let p: Vec<&str> = l.split_whitespace().collect();
        match p.first().map(|s| s.to_ascii_uppercase()).as_deref() {
            Some("ZOOM") => zoom = p.get(1)?.parse().ok(),
            Some("ORIGIN") => origin = Some([p.get(1)?.parse().ok()?, p.get(2)?.parse().ok()?]),
            Some("ROTATED") => rotated = p.get(1) == Some(&"1"),
            _ => {}
        }
    }
    Some((zoom?, origin?, rotated))
}

fn spawn(mut commands: Commands, config: Res<GameConfig>, w: Option<Res<Walker>>, mut images: ResMut<Assets<Image>>) {
    let map = w.as_ref().and_then(|w| w.client.map.clone()).unwrap_or_else(|| config.map.clone());
    let dir = crate::game_dir().join("cstrike/overviews");
    let img = std::fs::read(dir.join(format!("{map}.bmp"))).ok().and_then(|d| load_bmp(&d));
    let txt = std::fs::read_to_string(dir.join(format!("{map}.txt"))).ok().and_then(|t| parse_txt(&t));
    let (Some((iw, ih, rgba)), Some((zoom, origin, rotated))) = (img, txt) else { return };
    let ov = Overview { zoom, origin, rotated, size: Vec2::new(iw as f32, ih as f32) };
    commands.insert_resource(ov);
    let image = images.add(crate::level::rgba_image(iw, ih, rgba, false, TextureFormat::Rgba8UnormSrgb));
    let wedge = images_wedge(&mut images);
    commands
        .spawn((
            RadarRoot,
            crate::ui::HudLayer,
            DespawnOnExit(AppState::InGame),
            Node { position_type: PositionType::Absolute, left: vh(SAFE), top: vh(SAFE), width: vh(SIZE), height: vh(SIZE), overflow: Overflow::clip(), border: UiRect::all(px(1)), ..default() },
            BackgroundColor(BG0.with_alpha(0.78)),
            BorderColor::all(LINE),
        ))
        .with_children(|r| {
            r.spawn((Map, ImageNode { image, color: Color::srgba(0.75, 0.78, 0.82, 0.85), ..default() }, Node { width: percent(100), height: percent(100), ..default() }));
            for i in 0..DOTS {
                r.spawn((Dot(i), Visibility::Hidden, Node { position_type: PositionType::Absolute, width: vh(10.0), height: vh(10.0), border: UiRect::all(vh(2.0)), border_radius: BorderRadius::MAX, ..default() }, BorderColor::all(BG0), BackgroundColor(CT_COLOR)));
            }
            r.spawn((Bomb, Visibility::Hidden, Node { position_type: PositionType::Absolute, width: vh(12.0), height: vh(12.0), margin: UiRect { left: vh(-6.0), top: vh(-6.0), ..default() }, border: UiRect::all(vh(2.0)), ..default() }, UiTransform { rotation: Rot2::degrees(45.0), ..default() }, BorderColor::all(BG0), BackgroundColor(AMBER)));
            // You: an amber wedge in the centre, pointing where you look.
            r.spawn((Me, UiTransform::default(), Node { position_type: PositionType::Absolute, left: percent(50), top: percent(50), margin: UiRect { left: vh(-7.0), top: vh(-9.0), ..default() }, width: vh(14.0), height: vh(18.0), ..default() }))
                .with_children(|m| {
                    m.spawn((ImageNode { image: wedge.clone(), color: AMBER, ..default() }, Node { width: percent(100), height: percent(100), ..default() }));
                });
        });
}

/// A 28x36 arrow pointing up.
fn images_wedge(images: &mut Assets<Image>) -> Handle<Image> {
    let (w, h) = (28u32, 36u32);
    let mut rgba = vec![255u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let fy = y as f32 + 0.5;
            let half = w as f32 / 2.0 * (fy / h as f32);
            let inside = (x as f32 + 0.5 - w as f32 / 2.0).abs() <= half && !(fy > h as f32 * 0.72 && (x as f32 + 0.5 - w as f32 / 2.0).abs() < (fy - h as f32 * 0.72) * 1.2);
            rgba[((y * w + x) * 4 + 3) as usize] = if inside { 255 } else { 0 };
        }
    }
    images.add(crate::level::rgba_image(w, h, rgba, false, TextureFormat::Rgba8UnormSrgb))
}

#[allow(clippy::type_complexity)]
fn update(
    w: Res<Walker>,
    ov: Option<Res<Overview>>,
    cam: Query<&GlobalTransform, With<FlyCam>>,
    mut map: Query<&mut ImageNode, With<Map>>,
    mut me: Query<&mut UiTransform, With<Me>>,
    mut dots: Query<(&Dot, &mut Node, &mut Visibility, &mut BackgroundColor, &mut BorderColor), Without<Bomb>>,
    mut bomb: Query<(&mut Node, &mut Visibility), With<Bomb>>,
) {
    let Some(ov) = ov else { return };
    let c = &w.client;
    let o = c.me().origin;
    let centre = ov.to_px(o[0], o[1]);
    let half = SPAN * ov.zoom * 1024.0 / 8192.0 / 2.0;
    // Crop the overview to the window around you (the image fills the radar).
    for mut img in &mut map {
        let rect = Some(Rect::new(centre.x - half, centre.y - half, centre.x + half, centre.y + half));
        if img.rect != rect {
            img.rect = rect;
        }
    }
    if let Ok(t) = cam.single() {
        // Bevy forward (Y-up) back to GoldSrc: x stays, y = -z. Screen angle follows the image axes.
        let f = t.forward();
        let a = ov.to_px(o[0] + f.x, o[1] - f.z) - centre;
        for mut tr in &mut me {
            tr.rotation = Rot2::radians(a.x.atan2(-a.y));
        }
    }
    let my_team = c.team();
    let carrier = match c.bomb {
        BombNet::Carried(id) => Some(id),
        _ => None,
    };
    // Position on the radar (0..1), with off-window players held on its edge.
    let place = |x: f32, y: f32, edge: bool| {
        let d = (ov.to_px(x, y) - centre) / (2.0 * half);
        let d = if edge { d.clamp(Vec2::splat(-0.48), Vec2::splat(0.48)) } else { d };
        Vec2::new(0.5 + d.x, 0.5 + d.y)
    };
    let others: Vec<_> = c.others().into_iter().filter(|r| r.alive && r.team != Team::Spectator).collect();
    for (Dot(i), mut node, mut vis, mut bg, mut border) in &mut dots {
        let Some(r) = others.get(*i) else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let mate = r.team == my_team;
        let p = place(r.origin[0], r.origin[1], mate);
        // Enemies leave the radar when they leave its window.
        if !mate && !(0.0..=1.0).contains(&p.x) || !mate && !(0.0..=1.0).contains(&p.y) {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        }
        node.left = percent(p.x * 100.0);
        node.top = percent(p.y * 100.0);
        node.margin = UiRect { left: vh(-5.0), top: vh(-5.0), ..default() };
        node.border_radius = if mate { BorderRadius::MAX } else { BorderRadius::ZERO };
        bg.set_if_neq(BackgroundColor(if r.team == Team::T { T_COLOR } else { CT_COLOR }));
        // The teammate with the bomb gets an amber ring.
        border.set_if_neq(BorderColor::all(if carrier == Some(r.id) { AMBER } else { BG0 }));
        vis.set_if_neq(Visibility::Inherited);
    }
    let at = match c.bomb {
        BombNet::Dropped(p) | BombNet::Planted { at: p, .. } => Some(p),
        _ => None,
    };
    for (mut node, mut vis) in &mut bomb {
        let Some(p) = at else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        let q = place(p[0], p[1], true);
        node.left = percent(q.x * 100.0);
        node.top = percent(q.y * 100.0);
        vis.set_if_neq(Visibility::Inherited);
    }
}
