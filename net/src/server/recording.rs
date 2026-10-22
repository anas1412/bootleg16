//! Recording booked matches (see `demo`): started with the match, a frame per snapshot with
//! the whole truth, closed at the end. Reports point at the recording and the moment.
use super::*;

/// The scoreboard goes into the recording this often (in snapshots: once a second).
const SCORES_EVERY: u32 = crate::tick::SNAPSHOT_RATE;

impl Server {
    /// Start recording this match (if recordings are on: `demos` in the config).
    pub(super) fn start_recording(&mut self) {
        let cfg = &self.round.cfg;
        if cfg.demos.is_empty() {
            return;
        }
        let dir = std::path::PathBuf::from(&cfg.demos);
        crate::demo::prune(&dir, cfg.demo_days as u64);
        match crate::demo::Recorder::start(&dir, &self.map, self.map_hash, &cfg.name, self.rate) {
            Ok(r) => self.recorder = Some(r),
            Err(e) => println!("can't record to {}: {e}", dir.display()),
        }
    }

    pub(super) fn stop_recording(&mut self) {
        if let Some(r) = self.recorder.take() {
            match r.finish() {
                Ok(path) => println!("match recorded: {}", path.display()),
                Err(e) => println!("recording failed at the end: {e}"),
            }
        }
    }

    /// The recording's name and the tick now (for reports), if recording.
    pub(super) fn recording_now(&self) -> (String, u32) {
        (self.recorder.as_ref().map(|r| r.name()).unwrap_or_default(), self.tick)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn record(&mut self, players: &[PlayerNet], events: &[GameEvent], doors: &[sim::doors::DoorNet], broken: &[u16], grenades: &[(sim::grenades::Kind, [f32; 3])], smokes: &[SmokeNet], ground: &[(Id, [f32; 3])], round: &RoundNet) {
        if self.recorder.is_none() {
            return;
        }
        let scores = (self.tick / self.rate.snapshot_every() % SCORES_EVERY == 0).then(|| {
            self.clients.values().filter(|c| !c.bot).map(|c| c.score()).collect()
        });
        let frame = crate::demo::Frame {
            tick: self.tick,
            players: players.to_vec(),
            health: self.clients.values().map(|c| (c.id, c.vitals.health.clamp(0.0, 255.0) as u8)).collect(),
            events: events.to_vec(),
            doors: doors.to_vec(),
            broken: broken.to_vec(),
            grenades: grenades.to_vec(),
            smokes: smokes.to_vec(),
            ground: ground.to_vec(),
            round: *round,
            bomb: self.bomb_net(Team::T, |_| true, [0.0; 3]),
            scores,
        };
        if let Err(e) = self.recorder.as_mut().unwrap().frame(&frame) {
            println!("recording stopped: {e}");
            self.recorder = None;
        }
    }
}
