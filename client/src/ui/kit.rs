//! The font kit and shared images (embedded, so no asset paths), and the text helpers.
use super::tokens::Type;
use bevy::prelude::*;
use bevy::text::FontSize;

#[derive(Resource)]
pub struct Kit {
    /// Big Shoulders Display ExtraBold: large words.
    pub display: Handle<Font>,
    /// Chakra Petch SemiBold: labels and running text.
    pub text: Handle<Font>,
    /// JetBrains Mono Bold: every number (tabular, so widths never jump).
    pub num: Handle<Font>,
    /// Noto Sans CJK JP Bold, subset to kana and the menu kanji: the Japanese subtitles.
    pub jp: Handle<Font>,
}

impl FromWorld for Kit {
    fn from_world(world: &mut World) -> Self {
        let mut fonts = world.resource_mut::<Assets<Font>>();
        let display = fonts.add(Font::from_bytes(include_bytes!("../../assets/fonts/BigShouldersDisplay-ExtraBold.ttf").to_vec()));
        let text = fonts.add(Font::from_bytes(include_bytes!("../../assets/fonts/ChakraPetch-SemiBold.ttf").to_vec()));
        let num = fonts.add(Font::from_bytes(include_bytes!("../../assets/fonts/JetBrainsMono-Bold.ttf").to_vec()));
        let jp = fonts.add(Font::from_bytes(include_bytes!("../../assets/fonts/NotoSansJP-Bold-subset.otf").to_vec()));
        // Text without a font set (anything not built from the kit) still gets Chakra Petch.
        let _ = fonts.insert(AssetId::default(), Font::from_bytes(include_bytes!("../../assets/fonts/ChakraPetch-SemiBold.ttf").to_vec()));
        Kit { display, text, num, jp }
    }
}

impl Kit {
    fn font(&self, t: Type) -> &Handle<Font> {
        match t {
            Type::Display | Type::H2 => &self.display,
            Type::Jp => &self.jp,
            Type::NumL | Type::NumM | Type::NumS => &self.num,
            Type::H3 | Type::Body | Type::Label => &self.text,
        }
    }
}

/// A length designed at 1080p that scales with the window height.
pub fn vh(px_at_1080: f32) -> Val {
    Val::Vh(px_at_1080 / 10.8)
}

/// Text on the type scale. Labels and titles are uppercased here, so callers write
/// sentences normally.
pub fn t(kit: &Kit, ty: Type, s: impl Into<String>, color: Color) -> impl Bundle {
    let s: String = s.into();
    let s = if matches!(ty, Type::Label | Type::H2 | Type::Display) { s.to_uppercase() } else { s };
    kit_text(s, ty.size(), color, kit.font(ty))
}

/// Text in a kit font at any 1080p size (prefer `t` and the type scale).
pub fn kit_text(s: impl Into<String>, px_at_1080: f32, color: Color, font: &Handle<Font>) -> impl Bundle {
    (Text::new(s), TextFont { font: font.clone().into(), font_size: FontSize::Vh(px_at_1080 / 10.8), ..default() }, TextColor(color))
}

/// Text over the 3D world (team names, voice tags): default font, 1080p size, dark shadow
/// so it reads on any wall.
pub fn text(s: impl Into<String>, px_at_1080: f32, color: Color) -> impl Bundle {
    (Text::new(s), TextFont { font_size: FontSize::Vh(px_at_1080 / 10.8), ..default() }, TextColor(color), TextShadow { offset: Vec2::splat(1.5), color: Color::srgba(0.0, 0.0, 0.0, 0.8) })
}
