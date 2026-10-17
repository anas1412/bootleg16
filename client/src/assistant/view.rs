//! The assistants' faces (design/mockups/ui: faces.js, rig.js). In matches they appear in
//! the uplink (`uplink`); in Settings on the assistant chips.
//!
//! The portrait is a rig of layers from the face sheets (tools/export_faces.sh renders them
//! from design/mockups/ui/faces.js): turning the head moves each layer by its depth, the head
//! tilts on the neck, eyes blink, the mouth moves while talking, each mood has a pose, and
//! the assistant's personality scales all of it.
use super::lines::Mood;
use crate::ui::{vh, Anim, BG1};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// Face sheets: per face, 11 layers across × 9 moods down, 256 px cells.
#[derive(Resource)]
pub struct Faces {
    /// Claudette, ChildGBT, Twins her, Twins him, Grumpk.
    sheets: [Handle<Image>; 5],
    scan: Handle<Image>,
}

impl FromWorld for Faces {
    fn from_world(world: &mut World) -> Self {
        let pngs: [&[u8]; 5] = [
            include_bytes!("../../assets/assistant/rig_claudette.png"),
            include_bytes!("../../assets/assistant/rig_childgbt.png"),
            include_bytes!("../../assets/assistant/rig_sister.png"),
            include_bytes!("../../assets/assistant/rig_brother.png"),
            include_bytes!("../../assets/assistant/rig_grumpk.png"),
        ];
        let mut images = world.resource_mut::<Assets<Image>>();
        let sheets = pngs.map(|png| images.add(Image::from_buffer(png, bevy::image::ImageType::Extension("png"), Default::default(), true, Default::default(), Default::default()).expect("built-in face sheet")));
        Faces { sheets, scan: images.add(scanlines()) }
    }
}

/// Projection scanlines: a dark row in every three.
fn scanlines() -> Image {
    let (w, h) = (4u32, 96u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            rgba[i..i + 3].copy_from_slice(&[7, 8, 11]);
            rgba[i + 3] = if y % 3 == 0 { 70 } else { 0 };
        }
    }
    crate::level::rgba_image(w, h, rgba, false, TextureFormat::Rgba8UnormSrgb)
}

// ---- the rig ------------------------------------------------------------------------------
// Sheet columns.
const BACK: usize = 0;
const HEAD: usize = 2;
const EYES: usize = 4;
const SHUT: usize = 5;
const MOUTH: usize = 7;
const TALK: usize = 8;
const FRONT: usize = 10;
/// How far each layer moves per unit of turn or nod: back hair against it, the features most.
const DEPTH: [f32; 11] = [-0.35, 0.0, 0.4, 0.95, 0.95, 0.95, 0.95, 0.95, 0.95, 0.95, 0.75];
/// The portrait crops 112 face units (the mockup's TILE_VIEW).
const VIEW: f32 = 112.0;

fn cell(layer: usize, mood: Mood) -> Rect {
    let (x, y) = (layer as f32 * 256.0, mood as usize as f32 * 256.0);
    Rect::new(x, y, x + 256.0, y + 256.0)
}

/// The face sheet of the Twins' brother (the portrait turns to him when he speaks).
pub(super) const BROTHER_FACE: usize = 3;

/// Face sheet for an assistant (0..3); the Twins start with her.
pub(super) fn face_for(who: usize) -> usize {
    [0, 1, 2, 4][who.min(3)]
}

/// yaw, pitch, roll per mood (units like the mockup's rig).
fn pose(m: Mood) -> Vec3 {
    let p = match m {
        Mood::Neutral => [0.0, 0.0, 0.0],
        Mood::Happy => [2.0, -2.0, 5.0],
        Mood::Laughing => [0.0, -6.0, 7.0],
        Mood::Smug => [5.0, -4.0, -7.0],
        Mood::Shocked => [0.0, -7.0, 0.0],
        Mood::Thinking => [-7.0, -4.0, 8.0],
        Mood::Worried => [-2.0, 3.0, -4.0],
        Mood::Sad => [-3.0, 7.0, -5.0],
        Mood::Annoyed => [9.0, 1.0, -4.0],
    };
    Vec3::from(p)
}

/// How much each face moves: ChildGBT barely, Grumpk and the sister overact.
const LIVELY: [f32; 5] = [0.75, 0.25, 1.25, 0.6, 1.35];

/// A living portrait.
#[derive(Component)]
pub struct Rig {
    face: usize,
    mood: Mood,
    drawn: Option<(usize, Mood)>,
    pub talking: bool,
    size: f32,
    phase: f32,
    pose: Vec3,
    kick: Vec3,
    next_blink: f32,
    blink_until: f32,
    flap_at: f32,
    open: bool,
    layers: [Entity; 11],
    head: Entity,
    back: Entity,
}

impl Rig {
    /// Change face and mood; the change glitches in.
    pub(super) fn set(&mut self, face: usize, mood: Mood) {
        if (face, mood) != (self.face, self.mood) {
            self.face = face;
            self.mood = mood;
            if mood == Mood::Shocked {
                self.kick = Vec3::new(0.0, -8.0, 0.0);
            }
            if mood == Mood::Laughing {
                self.kick = Vec3::new(0.0, -4.0, 6.0);
            }
        }
    }
}

/// A portrait tile: the face's layers in its colour behind scanlines.
pub fn portrait(p: &mut ChildSpawnerCommands, faces: &Faces, face: usize, mood: Mood, color: Color, size: f32) -> Entity {
    let full = || Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    let layer = |p: &mut ChildSpawnerCommands, i: usize| -> Entity {
        let hidden = i == SHUT || i == TALK;
        p.spawn((full(), ImageNode { image: faces.sheets[face].clone(), rect: Some(cell(i, mood)), ..default() }, UiTransform::default(), if hidden { Visibility::Hidden } else { Visibility::Inherited }, Pickable::IGNORE)).id()
    };
    let mut tile = p.spawn((
        Node { width: vh(size), height: vh(size), flex_shrink: 0.0, overflow: Overflow::clip(), border: UiRect::all(px(1)), border_radius: BorderRadius::all(vh(crate::ui::RADIUS_S)), ..default() },
        BackgroundColor(BG1.mix(&color, 0.14)),
        BorderColor::all(color.with_alpha(0.45)),
        Pickable::IGNORE,
    ));
    let tile_id = tile.id();
    let mut ids = [Entity::PLACEHOLDER; 11];
    let mut head = Entity::PLACEHOLDER;
    tile.with_children(|t| {
        ids[BACK] = layer(t, BACK);
        ids[1] = layer(t, 1);
        head = t
            .spawn((full(), UiTransform::default(), Pickable::IGNORE))
            .with_children(|h| {
                for i in [HEAD, 3, EYES, SHUT, 6, MOUTH, TALK, 9, FRONT] {
                    ids[i] = layer(h, i);
                }
            })
            .id();
        t.spawn((full(), ImageNode { image: faces.scan.clone(), image_mode: NodeImageMode::Tiled { tile_x: true, tile_y: true, stretch_value: 1.0 }, ..default() }, Pickable::IGNORE));
    });
    let phase = (face as f32 * 7.3 + size) % 100.0;
    p.commands().entity(tile_id).insert(Rig {
        face,
        mood,
        drawn: Some((face, mood)),
        talking: false,
        size,
        phase,
        pose: Vec3::ZERO,
        kick: Vec3::ZERO,
        next_blink: 1.0 + phase % 3.0,
        blink_until: 0.0,
        flap_at: 0.0,
        open: false,
        layers: ids,
        head,
        back: ids[BACK],
    });
    tile_id
}

/// The small live portrait inside the Settings chip.
pub fn chip_portrait(c: &mut ChildSpawnerCommands, faces: &Faces, who: usize, color: Color) {
    let id = portrait(c, faces, face_for(who), Mood::Neutral, color, 28.0);
    c.commands().entity(id).entry::<Node>().and_modify(|mut n| n.margin = UiRect::left(vh(-8.0)));
}

#[allow(clippy::type_complexity)]
fn rig(time: Res<Time>, faces: Res<Faces>, mut rigs: Query<(Entity, &mut Rig)>, mut parts: Query<(&mut ImageNode, &mut UiTransform, &mut Visibility)>, mut heads: Query<&mut UiTransform, Without<ImageNode>>, mut commands: Commands) {
    let now = time.elapsed_secs();
    let dt = time.delta_secs().min(0.05);
    for (tile, mut r) in &mut rigs {
        // a new face or mood: redraw the layers, burst
        if r.drawn != Some((r.face, r.mood)) {
            let (face, mood) = (r.face, r.mood);
            for (i, e) in r.layers.iter().enumerate() {
                if let Ok((mut img, ..)) = parts.get_mut(*e) {
                    img.image = faces.sheets[face].clone();
                    img.rect = Some(cell(i, mood));
                }
            }
            if r.drawn.is_some() {
                commands.entity(tile).insert(Anim::burst());
            }
            r.drawn = Some((face, mood));
        }
        let lively = LIVELY[r.face];
        let t = now + r.phase;
        let idle = Vec3::new((t * 0.7).sin() * 2.2 + (t * 1.3).sin(), (t * 0.9).sin() * 1.2, (t * 0.5).sin() * 1.6) * lively;
        let mut target = pose(r.mood) * lively + idle + r.kick;
        if r.talking {
            target.y += (t * 13.0).sin() * 1.4 * lively;
        }
        if r.mood == Mood::Laughing {
            target.y -= (t * 9.0).sin().abs() * 2.5 * lively;
        }
        let k = 1.0 - (-dt / 0.12).exp();
        let cur = r.pose;
        r.pose = cur + (target - cur) * k;
        r.kick *= (-dt / 0.18).exp();
        let units = r.size / VIEW;
        let (yaw, pitch, roll) = (r.pose.x, r.pose.y, r.pose.z);
        for (i, e) in r.layers.iter().enumerate() {
            if let Ok((_, mut tr, _)) = parts.get_mut(*e) {
                tr.translation = Val2::new(vh(yaw * DEPTH[i] * units), vh(pitch * DEPTH[i] * units));
                if *e == r.back {
                    tr.rotation = Rot2::degrees(roll);
                }
            }
        }
        if let Ok(mut tr) = heads.get_mut(r.head) {
            tr.rotation = Rot2::degrees(roll);
        }
        // blinks (ChildGBT almost never blinks; closed-eye moods don't)
        if now > r.next_blink && !matches!(r.mood, Mood::Happy | Mood::Laughing) {
            r.blink_until = now + 0.11;
            r.next_blink = now + if r.face == 1 { 6.0 + fastrand::f32() * 4.0 } else { 2.2 + fastrand::f32() * 3.2 };
        }
        let shut = now < r.blink_until;
        if r.talking && now > r.flap_at {
            r.flap_at = now + 0.09 + fastrand::f32() * 0.07;
            r.open = !r.open;
        }
        let open = r.talking && r.open;
        for (i, show) in [(EYES, !shut), (SHUT, shut), (MOUTH, !open), (TALK, open)] {
            if let Ok((_, _, mut v)) = parts.get_mut(r.layers[i]) {
                v.set_if_neq(if show { Visibility::Inherited } else { Visibility::Hidden });
            }
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Faces>().add_systems(Update, rig);
}
