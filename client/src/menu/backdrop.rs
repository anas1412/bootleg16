//! The main menu's backdrop: `gfx/menu/background.png` (our key art) filling the screen
//! behind the menu, cropped to fit like a wallpaper, out of focus and dark so it reads as a
//! scene behind the menu, darker still toward the left where the list sits. Plain black when
//! the file is missing.
use super::AppState;
use crate::ui::BG0;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// How out of focus: the picture is shrunk this many times, then blurred.
const SHRINK: u32 = 2;
/// Box-blur radius (at the shrunk size); three passes each way make it a gaussian.
const RADIUS: usize = 3;

/// A gaussian-looking blur, done once on load: shrink by averaging (cheap and already soft),
/// then three box-blur passes across and down (three boxes in a row are near a gaussian).
fn soft(img: Image) -> Image {
    let (w, h) = (img.width(), img.height());
    let Some(src) = img.convert(bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb).and_then(|i| i.data) else { return img };
    let k = SHRINK;
    let (sw, sh) = ((w / k).max(1) as usize, (h / k).max(1) as usize);
    let mut px = vec![[0f32; 4]; sw * sh];
    for y in 0..sh {
        for x in 0..sw {
            let mut sum = [0f32; 4];
            for yy in y as u32 * k..((y as u32 + 1) * k).min(h) {
                for xx in x as u32 * k..((x as u32 + 1) * k).min(w) {
                    let i = ((yy * w + xx) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += src[i + c] as f32;
                    }
                }
            }
            px[y * sw + x] = sum.map(|v| v / (k * k) as f32);
        }
    }
    for _ in 0..3 {
        px = box_pass(&px, sw, sh, true);
        px = box_pass(&px, sw, sh, false);
    }
    let out = px.iter().flat_map(|p| p.map(|v| v.round().clamp(0.0, 255.0) as u8)).collect();
    crate::level::rgba_image(sw as u32, sh as u32, out, false, bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb)
}

/// One box-blur pass along rows (`across`) or columns, edges clamped.
fn box_pass(px: &[[f32; 4]], w: usize, h: usize, across: bool) -> Vec<[f32; 4]> {
    let (lines, len) = if across { (h, w) } else { (w, h) };
    let at = |line: usize, i: usize| if across { line * w + i } else { i * w + line };
    let mut out = vec![[0f32; 4]; w * h];
    let r = RADIUS as isize;
    for line in 0..lines {
        for i in 0..len {
            let mut sum = [0f32; 4];
            for d in -r..=r {
                let j = (i as isize + d).clamp(0, len as isize - 1) as usize;
                let p = px[at(line, j)];
                for c in 0..4 {
                    sum[c] += p[c];
                }
            }
            out[at(line, i)] = sum.map(|v| v / (2 * r + 1) as f32);
        }
    }
    out
}

#[derive(Component)]
struct Backdrop {
    aspect: f32,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Menu), spawn).add_systems(Update, cover.run_if(in_state(AppState::Menu)));
}

fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
    let Some(img) = crate::gamefiles::paths("gfx/menu/background.png")
        .iter()
        .find_map(|p| std::fs::read(p).ok())
        .and_then(|b| Image::from_buffer(&b, ImageType::Extension("png"), CompressedImageFormats::NONE, true, ImageSampler::linear(), RenderAssetUsages::default()).ok())
    else {
        return;
    };
    let aspect = img.width() as f32 / img.height() as f32;
    let img = soft(img);
    let full = || Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    // under everything on the menu
    commands
        .spawn((GlobalZIndex(-10), DespawnOnExit(AppState::Menu), Pickable::IGNORE, Node { overflow: Overflow::clip(), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..full() }))
        .with_children(|r| {
            r.spawn((Backdrop { aspect }, ImageNode::new(images.add(img)), Node { flex_shrink: 0.0, ..default() }, Pickable::IGNORE));
            // dim it all a little, and the left (the menu list) a lot
            r.spawn((full(), BackgroundColor(BG0.with_alpha(0.4)), Pickable::IGNORE));
            r.spawn((
                full(),
                BackgroundGradient::from(LinearGradient::to_right(vec![ColorStop::percent(BG0.with_alpha(0.8), 0.0), ColorStop::percent(BG0.with_alpha(0.6), 30.0), ColorStop::percent(BG0.with_alpha(0.0), 55.0)])),
                Pickable::IGNORE,
            ));
        });
}

/// Fill the window whatever its shape, cropping the overflow (no stretching).
fn cover(window: Single<&Window, With<PrimaryWindow>>, mut q: Query<(&Backdrop, &mut Node)>) {
    let (w, h) = (window.width(), window.height());
    for (b, mut n) in &mut q {
        let (bw, bh) = if w / h > b.aspect { (w, w / b.aspect) } else { (h * b.aspect, h) };
        if n.width != px(bw) || n.height != px(bh) {
            n.width = px(bw);
            n.height = px(bh);
        }
    }
}
