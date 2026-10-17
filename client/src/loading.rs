//! What the loading screen shows (`Loading`, while `AppState::Connecting`), the map loaded
//! for the match (`MapData`), and `Job`: slow work on its own thread so the screen keeps
//! moving. The flow itself is `menu::connect`; the screen is `status_screens`.
use bevy::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc, Mutex};

/// The steps of getting into a match, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    StartingServer,
    Contacting,
    LoadingMap,
    Joining,
}

impl Step {
    pub const ALL: [Step; 4] = [Step::StartingServer, Step::Contacting, Step::LoadingMap, Step::Joining];

    pub fn label(self) -> &'static str {
        match self {
            Step::StartingServer => "Starting server",
            Step::Contacting => "Contacting server",
            Step::LoadingMap => "Loading map",
            Step::Joining => "Joining",
        }
    }
}

/// Getting into a match, as the loading screen shows it.
#[derive(Resource)]
pub struct Loading {
    /// The map (practice) or the address (joining).
    pub target: String,
    /// The map, once known (practice: from the start; joining: once the server says).
    pub map: Option<String>,
    /// A solo practice on the built-in server (else joining a server).
    pub practice: bool,
    pub step: Step,
    /// 0..1, real: each step and the map's parts (file, collision, geometry) move it.
    pub progress: f32,
    /// Seconds since loading started.
    pub elapsed: f32,
}

/// The map as loaded for this match, shared by everything that reads it on entering the game
/// (the file is parsed once). `world` and `geometry` are taken by the game and the level.
#[derive(Resource)]
pub struct MapData {
    pub bsp: goldsrc::bsp::Bsp,
    pub world: Option<sim::collision::World>,
    pub geometry: Option<crate::map::MapGeometry>,
}

/// Work on its own thread; `poll` hands back the result once (Err: the thread died).
pub struct Job<T>(Mutex<mpsc::Receiver<T>>);

impl<T: Send + 'static> Job<T> {
    pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Job<T> {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || _ = tx.send(f()));
        Job(Mutex::new(rx))
    }

    pub fn poll(&self) -> Result<Option<T>, ()> {
        match self.0.lock().map_err(|_| ())?.try_recv() {
            Ok(v) => Ok(Some(v)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(()),
        }
    }
}

/// Progress a job reports (0..1) while it runs.
#[derive(Clone, Default)]
pub struct Progress(Arc<AtomicU32>);

impl Progress {
    pub fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed);
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
}

/// Load the map for a match: the file (checked against the server's), collision, then the
/// geometry and textures. Err: what to tell the player.
pub fn load_map(name: String, server_hash: u64, progress: Progress) -> Result<MapData, crate::notice::Notice> {
    use crate::notice::Notice;
    let root = crate::game_dir();
    let bsp = goldsrc::bsp::Bsp::load(&root.join("cstrike/maps").join(format!("{name}.bsp"))).map_err(|e| Notice::error("Map missing", format!("You don't have {name} ({e}).")))?;
    // Same map name isn't enough: a changed file would mean different walls on our side.
    if bsp.hash != server_hash {
        return Err(Notice::error("Map mismatch", format!("Your {name} is different from the server's (another version, or modified). Verify your game files.")));
    }
    progress.set(0.3);
    let world = sim::collision::World::from_bsp(&bsp);
    progress.set(0.5);
    let roots = crate::gamefiles::roots();
    let geometry = crate::map::load(&bsp, &roots.iter().map(|r| r.as_path()).collect::<Vec<_>>());
    progress.set(1.0);
    Ok(MapData { bsp, world: Some(world), geometry: Some(geometry) })
}

/// The game opens behind the loading screen, which stays until the first frames are drawn
/// with every shader ready (Bevy compiles pipelines in the background: without this the map
/// shows up a moment after the HUD, sky first).
pub struct LoadingPlugin;

impl Plugin for LoadingPlugin {
    fn build(&self, app: &mut App) {
        let waiting = Waiting::default();
        app.insert_resource(waiting.clone()).add_systems(Update, reveal.run_if(in_state(crate::menu::AppState::InGame)).run_if(resource_exists::<Loading>));
        if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
            render.insert_resource(waiting).add_systems(bevy::render::Render, count_waiting);
        }
    }
}

/// Pipelines still compiling (from the render world).
#[derive(Resource, Clone, Default)]
struct Waiting(Arc<AtomicU32>);

fn count_waiting(cache: Res<bevy::render::render_resource::PipelineCache>, waiting: Res<Waiting>) {
    waiting.0.store(cache.waiting_pipelines().count() as u32, Ordering::Relaxed);
}

/// In the game, still covered: lift the cover once nothing has been compiling for a few frames
/// (15 s at most: never stuck behind it).
fn reveal(mut commands: Commands, mut loading: ResMut<Loading>, waiting: Res<Waiting>, time: Res<Time>, mut frames: Local<u32>, mut quiet: Local<u32>, mut covered: Local<f32>) {
    loading.elapsed += time.delta_secs();
    *covered += time.delta_secs();
    *frames += 1;
    *quiet = if waiting.0.load(Ordering::Relaxed) == 0 { *quiet + 1 } else { 0 };
    loading.progress = 0.95 + 0.05 * (*quiet as f32 / 5.0).min(1.0);
    if (*frames >= 5 && *quiet >= 5) || *covered > 15.0 {
        commands.remove_resource::<Loading>();
        (*frames, *quiet, *covered) = (0, 0, 0.0);
    }
}
