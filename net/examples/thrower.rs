//! Headless test client: connects, throws one grenade (slot 4, pressed `presses` times to
//! pick the type), then stands still. Usage: thrower <server addr> [presses] [pitch]
use net::Client;
use sim::pmove::Input;
use std::time::{Duration, Instant};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let addr = a.get(1).expect("server address").parse().expect("ip:port");
    let presses: usize = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(1);
    let pitch: f32 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(-20.0);
    let home = std::env::var("HOME").unwrap();
    let bsp = goldsrc::bsp::Bsp::load(format!("{home}/.steam/steam/steamapps/common/Half-Life/cstrike/maps/de_dust2.bsp")).unwrap();
    let mut c = Client::connect(addr, "thrower", sim::collision::World::from_bsp(&bsp)).unwrap();
    let tick = Duration::from_secs_f64(1.0 / 128.0);
    let start = Instant::now();
    let mut t = 0u32;
    let mut joined = false;
    while start.elapsed() < Duration::from_secs(30) {
        t += 1;
        if !joined && c.id().is_some() {
            c.join_team(Some(net::protocol::Team::T));
            joined = true;
        }
        let base = Input { yaw: 270.0, pitch, ..Default::default() };
        let input = match t {
            n if n > 50 && n <= 50 + presses as u32 * 120 && (n - 50) % 120 == 1 => Input { slot: 4, ..base },
            n if n > 50 + presses as u32 * 120 && n < 50 + presses as u32 * 120 + 100 => Input { attack: true, ..base },
            _ => base,
        };
        c.tick(input);
        if t % 256 == 0 {
            eprintln!("t={t} active={:?} grenades={:?} flying={} smokes={}", c.state.loadout.active_id(), c.state.loadout.grenades, c.grenades.len(), c.smokes.len());
        }
        std::thread::sleep(tick);
    }
}
