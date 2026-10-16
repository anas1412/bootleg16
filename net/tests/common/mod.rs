//! Shared by the network tests: a server and two clients over real localhost UDP.
//! Tests are skipped when de_dust2 isn't installed.
#![allow(dead_code)]
use goldsrc::bsp::Bsp;
use net::{Client, Server};
use sim::collision::World;
use sim::pmove::Input;
use std::net::SocketAddr;
use std::time::Duration;

pub fn game_dir() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    Some(std::env::var("BOOTLEG16_DIR").unwrap_or(format!("{home}/.steam/steam/steamapps/common/Half-Life")))
}

pub fn map(name: &str) -> Option<Bsp> {
    Bsp::load(format!("{}/cstrike/maps/{name}.bsp", game_dir()?)).ok()
}

pub fn dust2() -> Option<Bsp> {
    Bsp::load(format!("{}/cstrike/maps/de_dust2.bsp", game_dir()?)).ok()
}

pub fn assets() -> net::server::Assets {
    let dir = game_dir().unwrap();
    let model = goldsrc::mdl::Model::load(format!("{dir}/cstrike/models/player/arctic/arctic.mdl")).expect("player model");
    net::server::Assets {
        hitboxes: sim::hitbox::Hitboxes::from_model(&model, "carbine").unwrap(),
        materials: goldsrc::materials::Materials::parse(&std::fs::read_to_string(format!("{dir}/cstrike/sound/materials.txt")).unwrap_or_default()),
    }
}

pub struct Game {
    pub server: Server,
    pub a: Client,
    pub b: Client,
}

/// A server and two clients; engine tests use rules that stay out of the way:
/// no freeze time, money to spare, teammates hittable.
pub fn setup(bsp: &Bsp, ping_ms: u32, loss: f32) -> Game {
    setup_with(bsp, ping_ms, loss, net::config::MatchConfig { freeze_time: 0.0, buy_time: 1e6, round_time: 1e6, start_money: 16_000, friendly_fire: true, ..Default::default() })
}

pub fn setup_with(bsp: &Bsp, ping_ms: u32, loss: f32, cfg: net::config::MatchConfig) -> Game {
    let mut server = Server::new(SocketAddr::from(([127, 0, 0, 1], 0)), "test", bsp, assets()).unwrap();
    server.set_config(cfg);
    let addr = server.local_addr();
    let mut a = Client::connect(addr, "a", World::from_bsp(bsp)).unwrap();
    let mut b = Client::connect(addr, "b", World::from_bsp(bsp)).unwrap();
    a.simulate_network(ping_ms, loss);
    b.simulate_network(ping_ms, loss);
    Game { server, a, b }
}

impl Game {
    /// One tick of everything; a short sleep lets localhost packets arrive.
    pub fn tick(&mut self, ia: Input, ib: Input) {
        self.server.step();
        self.a.tick(ia);
        self.b.tick(ib);
        std::thread::sleep(Duration::from_micros(150));
    }

    /// Connect and pick sides (auto puts them on opposite teams).
    pub fn connect(&mut self) {
        self.connect_as(None, None);
    }

    /// Connect and join the given teams (same team = neighbouring spawns, handy for shooting tests).
    pub fn connect_as(&mut self, a: Option<net::protocol::Team>, b: Option<net::protocol::Team>) {
        for _ in 0..128 {
            self.tick(Input::default(), Input::default());
        }
        self.a.join_team(a);
        for _ in 0..8 {
            self.tick(Input::default(), Input::default());
        }
        self.b.join_team(b);
        for _ in 0..128 {
            self.tick(Input::default(), Input::default());
        }
        assert!(self.a.id().is_some() && self.b.id().is_some(), "{:?} {:?}", self.a.status, self.b.status);
        assert!(self.server.player_count() >= 2);
        assert!(self.server.vitals(self.a.id().unwrap()).unwrap().alive(), "A spawned");
    }
}

pub fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

