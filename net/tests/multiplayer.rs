//! Server + two clients over real localhost UDP. Skipped when de_dust2 isn't installed.
use goldsrc::bsp::Bsp;
use net::{Client, Server};
use sim::collision::World;
use sim::pmove::Input;
use std::net::SocketAddr;
use std::time::Duration;

fn dust2() -> Option<Bsp> {
    let home = std::env::var("HOME").ok()?;
    let dir = std::env::var("BOOTLEG16_DIR").unwrap_or(format!("{home}/.steam/steam/steamapps/common/Half-Life"));
    Bsp::load(format!("{dir}/cstrike/maps/de_dust2.bsp")).ok()
}

struct Game {
    server: Server,
    a: Client,
    b: Client,
}

fn setup(bsp: &Bsp, ping_ms: u32, loss: f32) -> Game {
    let server = Server::new(SocketAddr::from(([127, 0, 0, 1], 0)), "de_dust2", bsp).unwrap();
    let addr = server.local_addr();
    let mut a = Client::connect(addr, "a", World::from_bsp(bsp)).unwrap();
    let mut b = Client::connect(addr, "b", World::from_bsp(bsp)).unwrap();
    a.simulate_network(ping_ms, loss);
    b.simulate_network(ping_ms, loss);
    Game { server, a, b }
}

impl Game {
    /// One tick of everything; a short sleep lets localhost packets arrive.
    fn tick(&mut self, ia: Input, ib: Input) {
        self.server.step();
        self.a.tick(ia);
        self.b.tick(ib);
        std::thread::sleep(Duration::from_micros(150));
    }

    fn connect(&mut self) {
        for _ in 0..256 {
            self.tick(Input::default(), Input::default());
        }
        assert!(self.a.id().is_some() && self.b.id().is_some(), "{:?} {:?}", self.a.status, self.b.status);
        assert_eq!(self.server.player_count(), 2);
    }
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Run A along a path, then stand still; check the server, B's view and A's prediction agree.
fn run_and_check(g: &mut Game, label: &str) {
    let id = g.a.id().unwrap();
    let mut last = g.server.player(id).unwrap().origin;
    let mut travelled = 0.0;
    let mut worst_correction = 0.0f32;
    for t in 0..256 {
        // Run in wide circles with jumps, bouncing off whatever walls are near the spawn.
        let input = Input { forward: 400.0, yaw: t as f32 * 2.0, jump: t % 70 == 0, ..Default::default() };
        g.tick(input, Input::default());
        worst_correction = worst_correction.max(g.a.last_correction);
        let now = g.server.player(id).unwrap().origin;
        travelled += dist(now, last);
        last = now;
    }
    for _ in 0..128 {
        g.tick(Input::default(), Input::default());
    }
    let server_a = *g.server.player(g.a.id().unwrap()).unwrap();
    assert!(travelled > 200.0, "{label}: A only travelled {travelled} units on the server");
    // A's prediction ends exactly where the server says (same code, same inputs).
    assert!(dist(g.a.me.origin, server_a.origin) < 0.01, "{label}: prediction {:?} vs server {:?}", g.a.me.origin, server_a.origin);
    // Mid-run corrections stay tiny: prediction was right the whole way.
    assert!(worst_correction < 1.0, "{label}: worst correction {worst_correction}");
    // B sees A where the server has them.
    let seen = g.b.others().into_iter().find(|r| Some(r.id) == g.a.id()).expect("B sees A");
    assert!(dist(seen.origin, server_a.origin) < 1.0, "{label}: B sees A at {:?}, server {:?}", seen.origin, server_a.origin);
}

#[test]
fn two_clients_on_lan() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.connect();
    run_and_check(&mut g, "lan");
}

#[test]
fn two_clients_at_100ms_ping_with_5_percent_loss() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 100, 0.05);
    g.connect();
    run_and_check(&mut g, "100ms/5%");
}

#[test]
fn speed_hack_gains_nothing() {
    let Some(bsp) = dust2() else { return };
    let mut g = setup(&bsp, 0, 0.0);
    g.connect();
    let id = g.a.id().unwrap();
    let mut last = g.server.player(id).unwrap().origin;
    let mut travelled = 0.0;
    // A hacked client sends inputs 3x faster than real time for 2 seconds,
    // running in circles so walls don't cut the run short.
    for t in 0..256 {
        let run = Input { forward: 400.0, yaw: t as f32 * 2.0, ..Default::default() };
        g.server.step();
        for _ in 0..3 {
            g.a.tick(run);
        }
        g.b.tick(Input::default());
        std::thread::sleep(Duration::from_micros(150));
        let now = g.server.player(id).unwrap().origin;
        travelled += dist(now, last);
        last = now;
    }
    let moved = travelled;
    // Honest max: 2 s at 250 u/s = 500 units (+ the 16-input burst allowance). Walls may cut it short.
    assert!(moved < 540.0, "speed hack moved {moved} units in 2 s");
    assert!(moved > 300.0, "player should still move at normal speed, moved {moved}");
}
