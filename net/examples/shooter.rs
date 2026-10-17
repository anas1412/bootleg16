//! Headless test client: joins a team and shoots at the nearest other player, for checking
//! blood, flinches, deaths and bullet holes. Usage: shooter <server addr> [t|ct] [seconds] [pitch: just look that way, no shooting] [strafe: and walk side to side (for leg animations)]
use net::protocol::Team;
use net::Client;
use sim::pmove::Input;
use std::time::{Duration, Instant};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let addr = a.get(1).expect("server address").parse().expect("ip:port");
    let team = if a.get(2).is_some_and(|t| t == "t") { Team::T } else { Team::Ct };
    let secs: u64 = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(30);
    let fixed_pitch: Option<f32> = a.get(4).and_then(|x| x.parse().ok());
    let strafe = a.get(5).is_some_and(|x| x == "strafe");
    let home = std::env::var("HOME").unwrap();
    let bsp = goldsrc::bsp::Bsp::load(format!("{}/../dist/gamedata/maps/de_dust2.bsp", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut c = Client::connect(addr, "shooter", sim::collision::World::from_bsp(&bsp)).unwrap();
    let tick = Duration::from_secs_f64(1.0 / 128.0);
    let start = Instant::now();
    let (mut t, mut joined) = (0u32, false);
    while start.elapsed() < Duration::from_secs(secs) {
        t += 1;
        if !joined && c.id().is_some() {
            c.join_team(Some(team));
            joined = true;
        }
        let eye = c.me().eye();
        let target = c.others().into_iter().filter(|o| o.alive).min_by(|x, y| {
            let d = |o: &net::client::Remote| (0..3).map(|i| (o.origin[i] - eye[i]).powi(2)).sum::<f32>();
            d(x).total_cmp(&d(y))
        });
        let mut input = Input::default();
        if let Some(o) = target {
            // Alternate head and body.
            let z = if (t / 128) % 2 == 0 { 26.0 } else { 6.0 };
            let d = [o.origin[0] - eye[0], o.origin[1] - eye[1], o.origin[2] + z - eye[2]];
            input.yaw = d[1].atan2(d[0]).to_degrees();
            input.pitch = -(d[2].atan2((d[0] * d[0] + d[1] * d[1]).sqrt())).to_degrees();
            input.attack = t > 300 && t % 64 < 8;
        }
        if let Some(p) = fixed_pitch {
            // Strafing: side to side every second, facing one fixed way.
            let side = if strafe { if (t / 128) % 2 == 0 { 400.0 } else { -400.0 } } else { 0.0 };
            input = Input { yaw: if strafe { 0.0 } else { input.yaw }, pitch: p, side, ..Default::default() };
        }
        c.tick(input);
        std::thread::sleep(tick);
    }
}
