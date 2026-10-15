//! Every gun's numbers. Source: docs/WEAPONS.md (checked against CS 1.6's cs.so)
//! and docs/RULES.md (prices).
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Id {
    Knife,
    Glock,
    Usp,
    P228,
    Deagle,
    FiveSeven,
    Elite,
    M3,
    Xm1014,
    Mac10,
    Tmp,
    Mp5,
    Ump45,
    P90,
    Galil,
    Famas,
    Ak47,
    M4a1,
    Sg552,
    Aug,
    Scout,
    Awp,
    G3sg1,
    Sg550,
    M249,
    HeGrenade,
    Flashbang,
    Smoke,
    C4,
}

/// Inventory slot (the number key that selects it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Slot {
    Primary = 1,
    Secondary = 2,
    Knife = 3,
    Grenade = 4,
    Bomb = 5,
}

/// Penetration power and the longest segment that still penetrates, per bullet type.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bullet {
    pub power: i32,
    pub max_pen_distance: f32,
}

pub const B9MM: Bullet = Bullet { power: 21, max_pen_distance: 800.0 };
pub const B45ACP: Bullet = Bullet { power: 15, max_pen_distance: 500.0 };
pub const B338: Bullet = Bullet { power: 45, max_pen_distance: 8000.0 };
pub const B762: Bullet = Bullet { power: 39, max_pen_distance: 5000.0 };
pub const B556: Bullet = Bullet { power: 35, max_pen_distance: 4000.0 };
pub const B50AE: Bullet = Bullet { power: 30, max_pen_distance: 1000.0 };
pub const B57: Bullet = Bullet { power: 30, max_pen_distance: 2000.0 };
pub const B357: Bullet = Bullet { power: 25, max_pen_distance: 800.0 };

/// How the spread number is chosen for a shot ("acc" = the gun's accuracy value).
#[derive(Clone, Copy, Debug)]
pub enum Spread {
    /// Rifles/M249: `a + b*acc` for in the air, horizontal speed > `fast_speed`, otherwise.
    Rifle { air: (f32, f32), fast_speed: f32, fast: (f32, f32), still: (f32, f32) },
    /// SMGs: `k*acc`; P90 also has a fast-moving value.
    Smg { air: f32, fast: Option<(f32, f32)>, still: f32 },
    /// Pistols: `k*(1-acc)` for air, moving (any speed), crouched, standing.
    Pistol { air: f32, moving: f32, duck: f32, stand: f32 },
    /// Snipers: flat values (speed thresholds tested in order), plus `unscoped` when not zoomed.
    Sniper { air: f32, moving: &'static [(f32, f32)], duck: f32, stand: f32, unscoped: f32, rule: SniperRule },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SniperRule {
    /// AWP, Scout: accuracy unused.
    Flat,
    /// G3SG1: the whole value (with the unscoped add) times `(1-acc)`, acc updated first.
    G3,
    /// SG550: air/duck/stand times `(1-acc)` (acc before the update), moving flat.
    Sg550,
}

/// How accuracy changes as you shoot.
#[derive(Clone, Copy, Debug)]
pub enum Accuracy {
    /// `shots^power div div + add`, capped. `div` 0 means float divide by `fdiv` (MP5).
    Auto { power: u32, div: i32, fdiv: f32, add: f32, cap: f32, initial: f32, on_reload: f32 },
    /// Pistols: `acc -= (k - since_last_shot) * s`, clamped to `lo..hi`.
    PistolTime { k: f32, s: f32, lo: f32, hi: f32, initial: f32 },
    /// Auto-snipers: `min(cap, since_last_shot * k + base)`.
    SniperTime { base: f32, k: f32, cap: f32, initial: f32, on_reload: Option<f32>, first_shot: Option<f32> },
    None,
}

/// How the view kicks per shot.
#[derive(Clone, Copy, Debug)]
pub enum Recoil {
    /// `KickBack` parameter sets for moving, air, crouched, standing.
    /// Rifles pick moving first; SMGs/M249 pick air first (`smg_order`).
    Kick { smg_order: bool, sets: [[f32; 7]; 4] },
    /// Flat pitch kick per shot (pistols, AWP, Scout).
    Flat(f32),
    /// G3SG1/SG550: random pitch kick in `lo..hi` plus a quarter of the current punch, random yaw.
    AutoSniper { lo: f32, hi: f32 },
    /// Shotguns: random integer pitch kick, bigger in the air.
    Shotgun { ground: (i32, i32), air: (i32, i32) },
    None,
}

/// Follow-up bullets of a burst (Glock, FAMAS).
#[derive(Clone, Copy, Debug)]
pub struct Burst {
    pub damage: i32,
    pub range_mod: f32,
    /// Fixed spread; None = reuse the first bullet's spread.
    pub spread: Option<f32>,
    /// Delay before the 2nd and 3rd bullet (0 = next tick).
    pub delays: [f32; 2],
}

/// One firing mode (silenced/unsilenced, semi/burst).
#[derive(Clone, Copy, Debug)]
pub struct Mode {
    pub damage: i32,
    pub range_mod: f32,
    pub cycle: f32,
    pub spread: Spread,
    pub burst: Option<Burst>,
}

/// What the secondary button does.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Secondary {
    None,
    /// Switch between `modes[0]` and `modes[1]`; seconds the switch takes.
    Toggle(f32),
    /// Field-of-view steps to cycle through (back to 90 after the last).
    Zoom(&'static [u8]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pellets {
    pub count: u32,
    pub cone: f32,
    pub range: f32,
    pub damage: f32,
    /// Seconds per shell when reloading.
    pub shell_time: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Gun {
    pub clip: u32,
    pub reserve: u32,
    pub penetration: u32,
    pub distance: f32,
    pub bullet: Bullet,
    pub reload: f32,
    pub deploy: f32,
    pub speed: f32,
    pub scoped_speed: f32,
    /// Attacker weapon factor in the armor formula.
    pub armor_factor: f32,
    pub modes: &'static [Mode],
    pub secondary: Secondary,
    /// Fire with zoom on drops the zoom until the gun is ready again (AWP, Scout).
    pub unscope_on_fire: bool,
    /// Zoomed fire interval (AUG, SG552).
    pub zoomed_cycle: Option<f32>,
    pub accuracy: Accuracy,
    pub recoil: Recoil,
    /// One shot per click (pistols in semi mode).
    pub semi_auto: bool,
    pub pellets: Option<Pellets>,
}

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Gun(Gun),
    Knife,
    Grenade,
    C4,
}

#[derive(Clone, Copy, Debug)]
pub struct Def {
    pub id: Id,
    pub name: &'static str,
    pub slot: Slot,
    pub price: u32,
    pub kind: Kind,
}

const RIFLE_SPEED: f32 = 140.0;

const M4_KICK: [[f32; 7]; 4] = [
    [1.0, 0.45, 0.28, 0.045, 3.75, 3.0, 7.0],
    [1.2, 0.5, 0.23, 0.15, 5.5, 3.5, 6.0],
    [0.6, 0.3, 0.2, 0.0125, 3.25, 2.0, 7.0],
    [0.65, 0.35, 0.25, 0.015, 3.5, 2.25, 7.0],
];
const FAMAS_KICK: [[f32; 7]; 4] = [
    [1.0, 0.45, 0.275, 0.05, 4.0, 2.5, 7.0],
    [1.25, 0.45, 0.22, 0.18, 5.5, 4.0, 5.0],
    [0.575, 0.325, 0.2, 0.011, 3.25, 2.0, 8.0],
    [0.625, 0.375, 0.25, 0.0125, 3.5, 2.25, 8.0],
];

const fn auto(power: u32, div: i32, add: f32, cap: f32, initial: f32, on_reload: f32) -> Accuracy {
    Accuracy::Auto { power, div, fdiv: 0.0, add, cap, initial, on_reload }
}
const fn pistol_acc(k: f32, s: f32, lo: f32, hi: f32) -> Accuracy {
    Accuracy::PistolTime { k, s, lo, hi, initial: hi }
}
const fn pistol_spread(air: f32, moving: f32, duck: f32, stand: f32) -> Spread {
    Spread::Pistol { air, moving, duck, stand }
}
const fn rifle(air: (f32, f32), fast: (f32, f32), still: (f32, f32)) -> Spread {
    Spread::Rifle { air, fast_speed: RIFLE_SPEED, fast, still }
}
const fn one(damage: i32, range_mod: f32, cycle: f32, spread: Spread) -> Mode {
    Mode { damage, range_mod, cycle, spread, burst: None }
}

/// Base gun with the fields most guns share; each entry overrides what differs.
const BASE: Gun = Gun {
    clip: 30,
    reserve: 90,
    penetration: 1,
    distance: 8192.0,
    bullet: B9MM,
    reload: 2.5,
    deploy: 0.75,
    speed: 250.0,
    scoped_speed: 250.0,
    armor_factor: 1.0,
    modes: &[],
    secondary: Secondary::None,
    unscope_on_fire: false,
    zoomed_cycle: None,
    accuracy: Accuracy::None,
    recoil: Recoil::None,
    semi_auto: false,
    pellets: None,
};

pub static WEAPONS: &[Def] = &[
    Def { id: Id::Knife, name: "knife", slot: Slot::Knife, price: 0, kind: Kind::Knife },
    Def {
        id: Id::Glock,
        name: "glock18",
        slot: Slot::Secondary,
        price: 400,
        kind: Kind::Gun(Gun {
            clip: 20,
            reserve: 120,
            reload: 2.2,
            armor_factor: 1.05,
            modes: &[
                one(25, 0.75, 0.15, pistol_spread(1.0, 0.165, 0.075, 0.1)),
                Mode {
                    damage: 25,
                    range_mod: 0.75,
                    cycle: 0.5,
                    spread: pistol_spread(1.2, 0.185, 0.095, 0.3),
                    burst: Some(Burst { damage: 18, range_mod: 0.9, spread: Some(0.05), delays: [0.0, 0.0] }),
                },
            ],
            secondary: Secondary::Toggle(0.3),
            accuracy: pistol_acc(0.325, 0.275, 0.6, 0.9),
            semi_auto: true,
            ..BASE
        }),
    },
    Def {
        id: Id::Usp,
        name: "usp",
        slot: Slot::Secondary,
        price: 500,
        kind: Kind::Gun(Gun {
            clip: 12,
            reserve: 100,
            distance: 4096.0,
            bullet: B45ACP,
            reload: 2.7,
            modes: &[one(34, 0.79, 0.15, pistol_spread(1.2, 0.225, 0.08, 0.1)), one(30, 0.79, 0.15, pistol_spread(1.3, 0.25, 0.125, 0.15))],
            secondary: Secondary::Toggle(3.0),
            accuracy: pistol_acc(0.3, 0.275, 0.6, 0.92),
            recoil: Recoil::Flat(2.0),
            semi_auto: true,
            ..BASE
        }),
    },
    Def {
        id: Id::P228,
        name: "p228",
        slot: Slot::Secondary,
        price: 600,
        kind: Kind::Gun(Gun {
            clip: 13,
            reserve: 52,
            distance: 4096.0,
            bullet: B357,
            reload: 2.7,
            armor_factor: 1.25,
            modes: &[one(32, 0.8, 0.15, pistol_spread(1.5, 0.255, 0.075, 0.15))],
            accuracy: pistol_acc(0.325, 0.3, 0.6, 0.9),
            recoil: Recoil::Flat(2.0),
            semi_auto: true,
            ..BASE
        }),
    },
    Def {
        id: Id::Deagle,
        name: "deagle",
        slot: Slot::Secondary,
        price: 650,
        kind: Kind::Gun(Gun {
            clip: 7,
            reserve: 35,
            penetration: 2,
            distance: 4096.0,
            bullet: B50AE,
            reload: 2.2,
            armor_factor: 1.5,
            modes: &[one(54, 0.81, 0.225, pistol_spread(1.5, 0.25, 0.115, 0.13))],
            accuracy: pistol_acc(0.4, 0.35, 0.55, 0.9),
            recoil: Recoil::Flat(2.0),
            semi_auto: true,
            ..BASE
        }),
    },
    Def {
        id: Id::FiveSeven,
        name: "fiveseven",
        slot: Slot::Secondary,
        price: 750,
        kind: Kind::Gun(Gun {
            clip: 20,
            reserve: 100,
            distance: 4096.0,
            bullet: B57,
            reload: 2.7,
            armor_factor: 1.5,
            modes: &[one(20, 0.885, 0.15, pistol_spread(1.5, 0.255, 0.075, 0.15))],
            accuracy: pistol_acc(0.275, 0.25, 0.725, 0.92),
            recoil: Recoil::Flat(2.0),
            semi_auto: true,
            ..BASE
        }),
    },
    Def {
        id: Id::Elite,
        name: "elite",
        slot: Slot::Secondary,
        price: 800,
        kind: Kind::Gun(Gun {
            clip: 30,
            reserve: 120,
            reload: 4.5,
            armor_factor: 1.05,
            modes: &[one(36, 0.75, 0.075, pistol_spread(1.3, 0.175, 0.08, 0.1))],
            accuracy: pistol_acc(0.325, 0.275, 0.55, 0.88),
            recoil: Recoil::Flat(2.0),
            semi_auto: true,
            ..BASE
        }),
    },
    Def {
        id: Id::M3,
        name: "m3",
        slot: Slot::Primary,
        price: 1700,
        kind: Kind::Gun(Gun {
            clip: 8,
            reserve: 32,
            penetration: 0,
            distance: 3000.0,
            speed: 230.0,
            scoped_speed: 230.0,
            modes: &[one(20, 1.0, 0.875, Spread::Smg { air: 0.0, fast: None, still: 0.0 })],
            recoil: Recoil::Shotgun { ground: (4, 6), air: (8, 11) },
            pellets: Some(Pellets { count: 9, cone: 0.0675, range: 3000.0, damage: 20.0, shell_time: 0.45 }),
            ..BASE
        }),
    },
    Def {
        id: Id::Xm1014,
        name: "xm1014",
        slot: Slot::Primary,
        price: 3000,
        kind: Kind::Gun(Gun {
            clip: 7,
            reserve: 32,
            penetration: 0,
            distance: 3048.0,
            speed: 240.0,
            scoped_speed: 240.0,
            modes: &[one(20, 1.0, 0.25, Spread::Smg { air: 0.0, fast: None, still: 0.0 })],
            recoil: Recoil::Shotgun { ground: (3, 5), air: (7, 10) },
            pellets: Some(Pellets { count: 6, cone: 0.0725, range: 3048.0, damage: 20.0, shell_time: 0.3 }),
            ..BASE
        }),
    },
    Def {
        id: Id::Mac10,
        name: "mac10",
        slot: Slot::Primary,
        price: 1400,
        kind: Kind::Gun(Gun {
            reserve: 100,
            bullet: B45ACP,
            reload: 3.15,
            armor_factor: 0.95,
            modes: &[one(29, 0.82, 0.07, Spread::Smg { air: 0.375, fast: None, still: 0.03 })],
            accuracy: auto(3, 200, 0.6, 1.65, 0.15, 0.0),
            recoil: Recoil::Kick {
                smg_order: true,
                sets: [
                    [0.9, 0.45, 0.25, 0.035, 3.5, 2.75, 7.0],
                    [1.3, 0.55, 0.4, 0.05, 4.75, 3.75, 5.0],
                    [0.75, 0.4, 0.175, 0.03, 2.75, 2.5, 10.0],
                    [0.775, 0.425, 0.2, 0.03, 3.0, 2.75, 9.0],
                ],
            },
            ..BASE
        }),
    },
    Def {
        id: Id::Tmp,
        name: "tmp",
        slot: Slot::Primary,
        price: 1250,
        kind: Kind::Gun(Gun {
            reserve: 120,
            reload: 2.12,
            modes: &[one(20, 0.85, 0.07, Spread::Smg { air: 0.25, fast: None, still: 0.03 })],
            accuracy: auto(3, 200, 0.55, 1.4, 0.2, 0.2),
            recoil: Recoil::Kick {
                smg_order: true,
                sets: [
                    [0.8, 0.4, 0.2, 0.03, 3.0, 2.5, 7.0],
                    [1.1, 0.5, 0.35, 0.045, 4.5, 3.5, 6.0],
                    [0.7, 0.35, 0.125, 0.025, 2.5, 2.0, 10.0],
                    [0.725, 0.375, 0.15, 0.025, 2.75, 2.25, 9.0],
                ],
            },
            ..BASE
        }),
    },
    Def {
        id: Id::Mp5,
        name: "mp5navy",
        slot: Slot::Primary,
        price: 1500,
        kind: Kind::Gun(Gun {
            reserve: 120,
            reload: 2.63,
            modes: &[one(26, 0.84, 0.075, Spread::Smg { air: 0.2, fast: None, still: 0.04 })],
            accuracy: Accuracy::Auto { power: 2, div: 0, fdiv: 220.1, add: 0.45, cap: 0.75, initial: 0.0, on_reload: 0.0 },
            recoil: Recoil::Kick {
                smg_order: true,
                sets: [
                    [0.5, 0.275, 0.2, 0.03, 3.0, 2.0, 10.0],
                    [0.9, 0.475, 0.35, 0.0425, 5.0, 3.0, 6.0],
                    [0.225, 0.15, 0.1, 0.015, 2.0, 1.0, 10.0],
                    [0.25, 0.175, 0.125, 0.02, 2.25, 1.25, 10.0],
                ],
            },
            ..BASE
        }),
    },
    Def {
        id: Id::Ump45,
        name: "ump45",
        slot: Slot::Primary,
        price: 1700,
        kind: Kind::Gun(Gun {
            clip: 25,
            reserve: 100,
            bullet: B45ACP,
            reload: 3.5,
            modes: &[one(30, 0.82, 0.1, Spread::Smg { air: 0.24, fast: None, still: 0.04 })],
            accuracy: auto(2, 210, 0.5, 1.0, 0.0, 0.0),
            recoil: Recoil::Kick {
                smg_order: true,
                sets: [
                    [0.55, 0.3, 0.225, 0.03, 3.5, 2.5, 10.0],
                    [0.125, 0.65, 0.55, 0.0475, 5.5, 4.0, 10.0],
                    [0.25, 0.175, 0.125, 0.02, 2.25, 1.25, 10.0],
                    [0.275, 0.2, 0.15, 0.0225, 2.5, 1.5, 10.0],
                ],
            },
            ..BASE
        }),
    },
    Def {
        id: Id::P90,
        name: "p90",
        slot: Slot::Primary,
        price: 2350,
        kind: Kind::Gun(Gun {
            clip: 50,
            reserve: 100,
            bullet: B57,
            reload: 3.4,
            speed: 245.0,
            scoped_speed: 245.0,
            armor_factor: 1.5,
            modes: &[one(21, 0.885, 0.066, Spread::Smg { air: 0.3, fast: Some((170.0, 0.115)), still: 0.045 })],
            accuracy: auto(2, 175, 0.45, 1.0, 0.2, 0.2),
            recoil: Recoil::Kick {
                smg_order: true,
                sets: [
                    [0.45, 0.3, 0.2, 0.0275, 4.0, 2.25, 7.0],
                    [0.9, 0.45, 0.35, 0.04, 5.25, 3.5, 4.0],
                    [0.275, 0.2, 0.125, 0.02, 3.0, 1.0, 9.0],
                    [0.3, 0.225, 0.125, 0.02, 3.25, 1.25, 8.0],
                ],
            },
            ..BASE
        }),
    },
    Def {
        id: Id::Galil,
        name: "galil",
        slot: Slot::Primary,
        price: 2000,
        kind: Kind::Gun(Gun {
            clip: 35,
            penetration: 2,
            bullet: B556,
            reload: 2.45,
            speed: 240.0,
            scoped_speed: 240.0,
            armor_factor: 1.55,
            modes: &[one(30, 0.98, 0.0875, rifle((0.04, 0.3), (0.04, 0.07), (0.0, 0.0375)))],
            accuracy: auto(3, 200, 0.35, 1.25, 0.2, 0.2),
            recoil: Recoil::Kick { smg_order: false, sets: M4_KICK },
            ..BASE
        }),
    },
    Def {
        id: Id::Famas,
        name: "famas",
        slot: Slot::Primary,
        price: 2250,
        kind: Kind::Gun(Gun {
            clip: 25,
            penetration: 2,
            bullet: B556,
            reload: 3.3,
            speed: 240.0,
            scoped_speed: 240.0,
            armor_factor: 1.4,
            modes: &[
                // Auto adds 0.01 to every spread term.
                one(34, 0.96, 0.0825, rifle((0.04, 0.3), (0.04, 0.07), (0.01, 0.02))),
                Mode {
                    damage: 34,
                    range_mod: 0.96,
                    cycle: 0.55,
                    spread: rifle((0.03, 0.3), (0.03, 0.07), (0.0, 0.02)),
                    burst: Some(Burst { damage: 30, range_mod: 0.96, spread: None, delays: [0.05, 0.1] }),
                },
            ],
            secondary: Secondary::Toggle(0.3),
            accuracy: auto(3, 215, 0.3, 1.0, 0.2, 0.0),
            recoil: Recoil::Kick { smg_order: false, sets: FAMAS_KICK },
            ..BASE
        }),
    },
    Def {
        id: Id::Ak47,
        name: "ak47",
        slot: Slot::Primary,
        price: 2500,
        kind: Kind::Gun(Gun {
            penetration: 2,
            bullet: B762,
            reload: 2.45,
            speed: 221.0,
            scoped_speed: 221.0,
            armor_factor: 1.55,
            modes: &[one(36, 0.98, 0.0955, rifle((0.04, 0.4), (0.04, 0.07), (0.0, 0.0275)))],
            accuracy: auto(3, 200, 0.35, 1.25, 0.2, 0.2),
            recoil: Recoil::Kick {
                smg_order: false,
                sets: [
                    [1.5, 0.45, 0.225, 0.05, 6.5, 2.5, 7.0],
                    [2.0, 1.0, 0.5, 0.35, 9.0, 6.0, 5.0],
                    [0.9, 0.35, 0.15, 0.025, 5.5, 1.5, 9.0],
                    [1.0, 0.375, 0.175, 0.0375, 5.75, 1.75, 8.0],
                ],
            },
            ..BASE
        }),
    },
    Def {
        id: Id::M4a1,
        name: "m4a1",
        slot: Slot::Primary,
        price: 3100,
        kind: Kind::Gun(Gun {
            penetration: 2,
            bullet: B556,
            reload: 3.05,
            speed: 230.0,
            scoped_speed: 230.0,
            armor_factor: 1.4,
            modes: &[
                one(32, 0.97, 0.0875, rifle((0.035, 0.4), (0.035, 0.07), (0.0, 0.02))),
                one(33, 0.95, 0.0875, rifle((0.035, 0.4), (0.035, 0.07), (0.0, 0.025))),
            ],
            secondary: Secondary::Toggle(2.0),
            accuracy: auto(3, 220, 0.3, 1.0, 0.2, 0.2),
            recoil: Recoil::Kick { smg_order: false, sets: M4_KICK },
            ..BASE
        }),
    },
    Def {
        id: Id::Sg552,
        name: "sg552",
        slot: Slot::Primary,
        price: 3500,
        kind: Kind::Gun(Gun {
            penetration: 2,
            bullet: B556,
            reload: 3.0,
            speed: 235.0,
            scoped_speed: 200.0,
            armor_factor: 1.4,
            modes: &[one(33, 0.955, 0.0825, rifle((0.035, 0.45), (0.035, 0.075), (0.0, 0.02)))],
            secondary: Secondary::Zoom(&[55]),
            zoomed_cycle: Some(0.135),
            accuracy: auto(3, 220, 0.3, 1.0, 0.2, 0.2),
            recoil: Recoil::Kick {
                smg_order: false,
                sets: [
                    [1.0, 0.45, 0.28, 0.04, 4.25, 2.5, 7.0],
                    [1.25, 0.45, 0.22, 0.18, 6.0, 4.0, 5.0],
                    [0.6, 0.35, 0.2, 0.0125, 3.7, 2.0, 10.0],
                    [0.625, 0.375, 0.25, 0.0125, 4.0, 2.25, 9.0],
                ],
            },
            ..BASE
        }),
    },
    Def {
        id: Id::Aug,
        name: "aug",
        slot: Slot::Primary,
        price: 3500,
        kind: Kind::Gun(Gun {
            penetration: 2,
            bullet: B556,
            reload: 3.3,
            speed: 240.0,
            scoped_speed: 240.0,
            armor_factor: 1.4,
            modes: &[one(32, 0.96, 0.0825, rifle((0.035, 0.4), (0.035, 0.07), (0.0, 0.02)))],
            secondary: Secondary::Zoom(&[55]),
            zoomed_cycle: Some(0.135),
            accuracy: auto(3, 215, 0.3, 1.0, 0.2, 0.0),
            recoil: Recoil::Kick { smg_order: false, sets: FAMAS_KICK },
            ..BASE
        }),
    },
    Def {
        id: Id::Scout,
        name: "scout",
        slot: Slot::Primary,
        price: 2750,
        kind: Kind::Gun(Gun {
            clip: 10,
            penetration: 3,
            bullet: B762,
            reload: 2.0,
            deploy: 1.25,
            speed: 260.0,
            scoped_speed: 220.0,
            armor_factor: 1.7,
            modes: &[one(75, 0.98, 1.25, Spread::Sniper { air: 0.2, moving: &[(170.0, 0.075)], duck: 0.0, stand: 0.007, unscoped: 0.025, rule: SniperRule::Flat })],
            secondary: Secondary::Zoom(&[40, 15]),
            unscope_on_fire: true,
            recoil: Recoil::Flat(2.0),
            ..BASE
        }),
    },
    Def {
        id: Id::Awp,
        name: "awp",
        slot: Slot::Primary,
        price: 4750,
        kind: Kind::Gun(Gun {
            clip: 10,
            reserve: 30,
            penetration: 3,
            bullet: B338,
            reload: 2.5,
            deploy: 1.45,
            speed: 210.0,
            scoped_speed: 150.0,
            armor_factor: 1.95,
            modes: &[one(
                115,
                0.99,
                1.45,
                Spread::Sniper { air: 0.85, moving: &[(140.0, 0.25), (10.0, 0.1)], duck: 0.0, stand: 0.001, unscoped: 0.08, rule: SniperRule::Flat },
            )],
            secondary: Secondary::Zoom(&[40, 10]),
            unscope_on_fire: true,
            recoil: Recoil::Flat(2.0),
            ..BASE
        }),
    },
    Def {
        id: Id::G3sg1,
        name: "g3sg1",
        slot: Slot::Primary,
        price: 5000,
        kind: Kind::Gun(Gun {
            clip: 20,
            penetration: 3,
            bullet: B762,
            reload: 3.5,
            speed: 210.0,
            scoped_speed: 150.0,
            armor_factor: 1.65,
            modes: &[one(80, 0.98, 0.25, Spread::Sniper { air: 0.45, moving: &[(0.0, 0.15)], duck: 0.035, stand: 0.055, unscoped: 0.025, rule: SniperRule::G3 })],
            secondary: Secondary::Zoom(&[40, 15]),
            accuracy: Accuracy::SniperTime { base: 0.55, k: 0.3, cap: 0.98, initial: 0.2, on_reload: Some(0.2), first_shot: Some(0.98) },
            recoil: Recoil::AutoSniper { lo: 0.75, hi: 1.75 },
            ..BASE
        }),
    },
    Def {
        id: Id::Sg550,
        name: "sg550",
        slot: Slot::Primary,
        price: 4200,
        kind: Kind::Gun(Gun {
            penetration: 2,
            bullet: B556,
            reload: 3.35,
            speed: 210.0,
            scoped_speed: 150.0,
            armor_factor: 1.45,
            modes: &[one(70, 0.98, 0.25, Spread::Sniper { air: 0.45, moving: &[(0.0, 0.15)], duck: 0.04, stand: 0.05, unscoped: 0.025, rule: SniperRule::Sg550 })],
            secondary: Secondary::Zoom(&[40, 15]),
            // The original never initialises SG550 accuracy (0 until the second shot).
            accuracy: Accuracy::SniperTime { base: 0.65, k: 0.35, cap: 0.98, initial: 0.0, on_reload: None, first_shot: None },
            recoil: Recoil::AutoSniper { lo: 0.75, hi: 1.25 },
            ..BASE
        }),
    },
    Def {
        id: Id::M249,
        name: "m249",
        slot: Slot::Primary,
        price: 5750,
        kind: Kind::Gun(Gun {
            clip: 100,
            reserve: 200,
            penetration: 2,
            bullet: B556,
            reload: 4.7,
            speed: 220.0,
            scoped_speed: 220.0,
            armor_factor: 1.5,
            modes: &[one(32, 0.97, 0.1, rifle((0.045, 0.5), (0.045, 0.095), (0.0, 0.03)))],
            accuracy: auto(3, 175, 0.4, 0.9, 0.2, 0.2),
            recoil: Recoil::Kick {
                smg_order: true,
                sets: [
                    [1.1, 0.5, 0.3, 0.06, 4.0, 3.0, 8.0],
                    [1.8, 0.65, 0.45, 0.125, 5.0, 3.5, 8.0],
                    [0.75, 0.325, 0.25, 0.025, 3.5, 2.5, 9.0],
                    [0.8, 0.35, 0.3, 0.03, 3.75, 3.0, 9.0],
                ],
            },
            ..BASE
        }),
    },
    Def { id: Id::HeGrenade, name: "hegrenade", slot: Slot::Grenade, price: 300, kind: Kind::Grenade },
    Def { id: Id::Flashbang, name: "flashbang", slot: Slot::Grenade, price: 200, kind: Kind::Grenade },
    Def { id: Id::Smoke, name: "smokegrenade", slot: Slot::Grenade, price: 300, kind: Kind::Grenade },
    Def { id: Id::C4, name: "c4", slot: Slot::Bomb, price: 0, kind: Kind::C4 },
];

pub fn def(id: Id) -> &'static Def {
    WEAPONS.iter().find(|d| d.id == id).expect("every Id has a table entry")
}

pub fn gun(id: Id) -> Option<&'static Gun> {
    match &def(id).kind {
        Kind::Gun(g) => Some(g),
        _ => None,
    }
}

/// Max speed with this weapon out (scoped speed when zoomed).
pub fn max_speed(id: Id, zoomed: bool) -> f32 {
    match gun(id) {
        Some(g) if zoomed => g.scoped_speed,
        Some(g) => g.speed,
        None => 250.0,
    }
}

/// Armor formula factor of the attacker's weapon (knife 1.7).
pub fn armor_factor(id: Id) -> f32 {
    match &def(id).kind {
        Kind::Gun(g) => g.armor_factor,
        Kind::Knife => 1.7,
        _ => 1.0,
    }
}

/// Look a weapon up by its CS name ("ak47", "awp", "deagle", ...).
pub fn by_name(name: &str) -> Option<Id> {
    let n = name.trim().to_ascii_lowercase();
    let n = n.strip_prefix("weapon_").unwrap_or(&n);
    WEAPONS.iter().find(|d| d.name == n || (n == "mp5" && d.name == "mp5navy") || (n == "glock" && d.name == "glock18")).map(|d| d.id)
}
