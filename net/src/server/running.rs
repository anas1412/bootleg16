//! Running at the tick rate, and stopping without cutting a match short (draining).
use super::*;

impl Server {
    /// Run forever at the tick rate.
    pub fn run(&mut self) {
        self.run_until(&std::sync::atomic::AtomicBool::new(false));
    }

    /// Run at the tick rate until `stop` is set (the game's built-in server stops when you leave).
    pub fn run_until(&mut self, stop: &std::sync::atomic::AtomicBool) {
        self.run_while(|_| !stop.load(std::sync::atomic::Ordering::Relaxed));
    }

    /// Run at the tick rate; once `stop` is set, take no new matches and return when the
    /// current one is over (updates don't cut matches short).
    pub fn run_draining(&mut self, stop: &std::sync::atomic::AtomicBool) {
        self.run_while(|s| {
            if stop.load(std::sync::atomic::Ordering::Relaxed) && s.draining.is_none() {
                println!("{}", if s.is_reserved() { "stopping after this match" } else { "stopping" });
                s.draining = Some(s.tick);
                s.heartbeat(); // tell the lobby now: no new matches here
            }
            !s.drained()
        });
    }

    /// Draining and done: no match, and no booking can still be on its way (a few seconds
    /// since the lobby heard we're busy).
    pub fn drained(&self) -> bool {
        self.draining.is_some_and(|since| !self.is_reserved() && self.tick - since > 2 * self.rate.hz())
    }

    /// Start draining (see `run_draining`).
    pub fn drain(&mut self) {
        self.draining.get_or_insert(self.tick);
    }

    pub(super) fn run_while(&mut self, mut go_on: impl FnMut(&mut Self) -> bool) {
        let tick = Duration::from_secs_f64(1.0 / self.rate.hz() as f64);
        let mut next = Instant::now();
        while go_on(self) {
            self.step();
            next += tick;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else if now - next > Duration::from_millis(250) {
                next = now; // fell far behind (machine was suspended): don't fast-forward
            }
        }
    }
}
