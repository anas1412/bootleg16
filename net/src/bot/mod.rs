//! Test bots: real network players for filling test matches and load tests. A bot is a normal
//! `Client` (it signs in, gets a ticket, connects, predicts) with a brain on top, after the CS
//! 1.6 / Condition Zero bot (states Buy, Hunt, Attack, FetchBomb, PlantBomb, DefuseBomb,
//! Guard, EscapeFromBomb, InvestigateNoise; a reaction delay and aim error from its profile).
//! It only knows what the server shows it (the same anti-wallhack view as a person). Tests can
//! give it orders (`control`): go afk, hold, rush a site, grief, leave and rejoin... Never in
//! real matches: the official lobby requires Steam.
use crate::client::Client;
use crate::protocol::GameEvent;
use std::sync::Arc;

mod brain;
pub mod control;
pub mod nav;
pub mod profile;
pub use control::{Mode, Order, Place, Who};
pub use nav::Nav;
pub use profile::{Difficulty, Profile};

/// What bots know about a map: the way around, the spawns, the bomb sites.
pub struct BotMap {
    pub source: Source,
    pub nav: Nav,
    /// (terrorist, counter-terrorist) spawn.
    pub spawns: ([f32; 3], [f32; 3]),
    /// Bomb site centres, A first.
    pub sites: Vec<[f32; 3]>,
    pub zones: sim::zones::Zones,
}

impl BotMap {
    /// Takes a moment (the navigation is worked out from the map): load once, share.
    /// `name`: the map's name (which bomb site is A depends on the map).
    pub fn new(bsp: goldsrc::bsp::Bsp, name: &str) -> Arc<BotMap> {
        let (lo, hi) = (bsp.models[0].mins, bsp.models[0].maxs);
        BotMap::build(Source::Bsp(bsp), name, lo, hi)
    }

    /// The same for one of our own maps (`.bmap`).
    pub fn from_map(map: sim::mapfile::MapFile, hash: u64) -> Arc<BotMap> {
        let (lo, hi) = map.bounds();
        let name = map.name.clone();
        BotMap::build(Source::Map(map, hash), &name, lo, hi)
    }

    fn build(source: Source, name: &str, lo: [f32; 3], hi: [f32; 3]) -> Arc<BotMap> {
        let world = source.world();
        let nav = Nav::build(&world, lo, hi);
        let zones = world.zones.clone();
        let sites = zones.sites_a_first(name);
        Arc::new(BotMap { spawns: spawns_of(&zones), nav, sites, zones, source })
    }

    pub fn world(&self) -> sim::collision::World {
        self.source.world()
    }

    /// A game server on this map (tests: the bots' server).
    pub fn server(&self, bind: std::net::SocketAddr, name: &str, assets: crate::server::Assets) -> std::io::Result<crate::Server> {
        match &self.source {
            Source::Bsp(bsp) => crate::Server::new(bind, name, bsp, assets),
            Source::Map(map, hash) => crate::Server::with_map(bind, map, *hash, assets),
        }
    }
}

/// Where a map comes from: a GoldSrc BSP or our own format (with its file's hash).
pub enum Source {
    Bsp(goldsrc::bsp::Bsp),
    Map(sim::mapfile::MapFile, u64),
}

impl Source {
    pub fn world(&self) -> sim::collision::World {
        match self {
            Source::Bsp(bsp) => sim::collision::World::from_bsp(bsp),
            Source::Map(map, _) => map.world(),
        }
    }
}

pub struct Bot {
    pub client: Client,
    pub map: Arc<BotMap>,
    /// Its number for orders (`bot3`).
    pub index: usize,
    pub name: String,
    pub profile: Profile,
    pub mode: Mode,
    /// The site it plays (`plant a`); None = its own pick each round.
    pub site: Option<usize>,
    /// Left with `leave` (rejoin with `rejoin`).
    pub gone: bool,
    events: Vec<GameEvent>,
    brain: brain::State,
}

impl Bot {
    pub fn new(client: Client, map: Arc<BotMap>, index: usize, profile: Profile) -> Bot {
        let seed = index as u64 + 1;
        Bot { client, map, index, name: format!("bot{index}"), profile, mode: Mode::Normal, site: None, gone: false, events: Vec::new(), brain: brain::State::new(seed) }
    }

    /// Decide and send this tick's input.
    pub fn tick(&mut self) {
        if self.gone {
            return;
        }
        let input = self.think();
        self.client.tick(input);
        let new = self.client.take_events();
        self.hear(&new);
        self.events.extend(new);
    }

    /// What happened since last asked (the bot reads them too).
    pub fn take_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn is(&self, who: &Who) -> bool {
        match who {
            Who::All => true,
            Who::Team(t) => self.client.team() == *t,
            Who::Bot(i) => self.index == *i,
        }
    }

    pub fn order(&mut self, order: Order) {
        match order {
            Order::Mode(m) => self.mode = m,
            Order::Site(s) => self.site = s,
            Order::Difficulty(d) => self.profile = Profile::of(d),
            Order::Buy(w) => self.client.buy(crate::protocol::BuyItem::Weapon(w)),
            Order::Leave => {
                self.client.disconnect();
                self.gone = true;
            }
            Order::Rejoin if self.gone => {
                let (server, ticket, key) = (self.client.server(), self.client.ticket, self.client.key);
                if let Ok(mut c) = Client::connect(server, &self.name, self.map.world()) {
                    c.ticket = ticket;
                    c.key = key;
                    self.client = c;
                    self.gone = false;
                }
            }
            Order::Rejoin => {}
        }
    }
}

/// Give orders (`control`, `;` between several) to the bots they're for; the number of bots
/// that took each order, or what was wrong.
pub fn command(bots: &mut [Bot], line: &str) -> Result<Vec<usize>, String> {
    let mut taken = Vec::new();
    for part in line.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let (who, order) = control::parse(part)?;
        let mut n = 0;
        for b in bots.iter_mut().filter(|b| b.is(&who)) {
            b.order(order.clone());
            n += 1;
        }
        taken.push(n);
    }
    Ok(taken)
}

/// The first terrorist and counter-terrorist spawn of a map.
pub fn spawns_of(zones: &sim::zones::Zones) -> ([f32; 3], [f32; 3]) {
    let first = |side| zones.spawns(side).first().copied().unwrap_or_default();
    (first(sim::zones::Side::T), first(sim::zones::Side::Ct))
}
