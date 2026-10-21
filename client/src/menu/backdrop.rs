//! The main menu's backdrop: `gfx/menu/background.png` (our key art) filling the screen
//! behind the menu, cropped to fit like a wallpaper, out of focus and dark so it reads as a
//! scene behind the menu, darker still toward the left where the list sits. Plain black when
//! the file is missing.
use super::AppState;
use crate::ui::BG0;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// How out of focus: the picture is shrunk this many times (and drawn back up smoothly).
const BLUR: u32 = 10;

/// The picture shrunk `k` times by averaging; drawn full size, the GPU's smooth scaling
/// makes it a soft blur (cheap, once, on any machine).
fn soft(img: Image, k: u32) -> Image {
    let (w, h) = (img.width(), img.height());
    let Some(src) = img.convert(bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb).and_then(|i| i.data) else { return img };
    let (sw, sh) = ((w / k).max(1), (h / k).max(1));
    let mut out = vec![0u8; (sw * sh * 4) as usize];
    for y in 0..sh {
        for x in 0..sw {
            let mut sum = [0u32; 4];
            for yy in y * k..((y + 1) * k).min(h) {
                for xx in x * k..((x + 1) * k).min(w) {
                    let i = ((yy * w + xx) * 4) as usize;
                    for c in 0..4 {
                        sum[c] += src[i + c] as u32;
                    }
                }
            }
            let o = ((y * sw + x) * 4) as usize;
            for c in 0..4 {
                out[o + c] = (sum[c] / (k * k)) as u8;
            }
        }
    }
    crate::level::rgba_image(sw, sh, out, false, bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb)
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
    let img = soft(img, BLUR);
    let full = || Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    // under everything on the menu
    commands
        .spawn((GlobalZIndex(-10), DespawnOnExit(AppState::Menu), Pickable::IGNORE, Node { overflow: Overflow::clip(), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..full() }))
        .with_children(|r| {
            r.spawn((Backdrop { aspect }, ImageNode::new(images.add(img)), Node { flex_shrink: 0.0, ..default() }, Pickable::IGNORE));
            // dim it all a little, and the left (the menu list) a lot
            r.spawn((full(), BackgroundColor(BG0.with_alpha(0.72)), Pickable::IGNORE));
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
