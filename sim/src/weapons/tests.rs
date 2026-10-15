use super::*;

const DT: f32 = 1.0 / 128.0;

struct Sim {
    l: Loadout,
    now: f64,
    tick: u64,
}

impl Sim {
    fn with(id: Id) -> Sim {
        let mut l = Loadout::knife_only();
        l.give(id, 0.0);
        let mut s = Sim { l, now: 0.0, tick: 0 };
        s.idle(2.0); // finish deploying
        s
    }

    fn ctx(&self) -> Context {
        Context {
            now: self.now,
            dt: DT,
            on_ground: true,
            crouched: false,
            speed_2d: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            attack: false,
            attack2: false,
            reload: false,
            slot: 0,
            can_shoot: true,
            seed: self.tick,
        }
    }

    fn step(&mut self, f: impl Fn(&mut Context)) -> Fired {
        self.tick += 1;
        self.now += DT as f64;
        let mut c = self.ctx();
        f(&mut c);
        self.l.update(&c)
    }

    /// Hold an input for `secs`, collecting every shot.
    fn hold(&mut self, secs: f32, f: impl Fn(&mut Context) + Copy) -> Vec<Shot> {
        (0..(secs / DT).round() as usize).flat_map(|_| self.step(f).shots).collect()
    }

    fn idle(&mut self, secs: f32) {
        self.hold(secs, |_| {});
    }

    fn gun(&self) -> GunState {
        *self.l.active_gun().unwrap()
    }
}

fn attack(c: &mut Context) {
    c.attack = true;
}

#[test]
fn ak_fires_about_ten_rounds_a_second_then_reloads() {
    let mut s = Sim::with(Id::Ak47);
    let shots = s.hold(1.0, attack);
    // 0.0955 s cycle, rounded up to whole ticks: 10 or 11 shots in a second.
    assert!((10..=11).contains(&shots.len()), "{} shots", shots.len());
    assert!(shots.iter().all(|sh| sh.damage == 36));
    // Empty the clip, release: it reloads by itself and the ammo arrives at the end.
    s.hold(3.0, attack);
    assert_eq!(s.gun().clip, 0);
    s.idle(0.1);
    assert!(s.gun().reloading);
    assert_eq!(s.gun().clip, 0, "ammo only moves when the reload finishes");
    s.idle(2.5);
    assert_eq!((s.gun().clip, s.gun().reserve), (30, 60));
}

#[test]
fn switching_cancels_a_reload_without_losing_ammo() {
    let mut s = Sim::with(Id::Ak47);
    s.hold(0.5, attack);
    let before = s.gun();
    s.step(|c| c.reload = true);
    s.idle(1.0);
    s.step(|c| c.slot = 3); // knife
    s.idle(3.0);
    s.step(|c| c.slot = 1);
    s.idle(1.0);
    assert_eq!((s.gun().clip, s.gun().reserve), (before.clip, before.reserve));
}

#[test]
fn ak_accuracy_steps_and_first_shot_is_pinpoint() {
    let mut s = Sim::with(Id::Ak47);
    let shots = s.hold(1.0, attack);
    // First shot after deploy: 0.0275 * 0.2.
    assert!((shots[0].spread - 0.0055).abs() < 1e-6, "{}", shots[0].spread);
    // Shots 2-6 use accuracy 0.35 (n = 1..5), from the 7th it is 1.25.
    assert!((shots[1].spread - 0.0275 * 0.35).abs() < 1e-6);
    assert!((shots[5].spread - 0.0275 * 0.35).abs() < 1e-6);
    assert!((shots[6].spread - 0.0275 * 1.25).abs() < 1e-6);
}

#[test]
fn spraying_climbs_to_the_recoil_cap_and_settles_after() {
    let mut s = Sim::with(Id::Ak47);
    // The cap is hit right after each shot; the punch then fades until the next one.
    let mut peak = 0.0f32;
    for _ in 0..256 {
        s.step(attack);
        peak = peak.min(s.l.punch[0]);
    }
    assert!((peak + 5.75).abs() < 0.01, "standing AK caps at -5.75, got {peak}");
    s.idle(1.5);
    assert_eq!(s.l.punch, [0.0; 3]);
}

#[test]
fn moving_and_jumping_ruin_rifle_accuracy() {
    let mut s = Sim::with(Id::Ak47);
    let still = s.step(attack).shots[0].spread;
    s.idle(2.0);
    let running = s.step(|c| {
        c.attack = true;
        c.speed_2d = 200.0;
    });
    s.idle(2.0);
    let jumping = s.step(|c| {
        c.attack = true;
        c.on_ground = false;
    });
    assert!(running.shots[0].spread > still * 5.0);
    assert!(jumping.shots[0].spread > running.shots[0].spread);
}

#[test]
fn pistols_fire_once_per_click() {
    let mut s = Sim::with(Id::Usp);
    assert_eq!(s.hold(1.0, attack).len(), 1, "holding the button fires once");
    s.idle(0.05);
    assert_eq!(s.hold(0.05, attack).len(), 1, "a new click fires again");
    assert_eq!(s.l.punch[0] < 0.0, true, "USP kicks the view up");
}

#[test]
fn deploy_time_blocks_shooting() {
    let mut l = Loadout::knife_only();
    l.give(Id::Awp, 0.0);
    let mut s = Sim { l, now: 0.0, tick: 0 };
    assert!(s.hold(1.4, attack).is_empty(), "AWP takes 1.45 s to draw");
    assert_eq!(s.hold(0.2, attack).len(), 1);
}

#[test]
fn glock_burst_fires_three_with_weaker_follow_ups() {
    let mut s = Sim::with(Id::Glock);
    s.step(|c| c.attack2 = true); // burst mode
    s.idle(0.5);
    let shots = s.hold(0.1, attack);
    let dmg: Vec<i32> = shots.iter().map(|sh| sh.damage).collect();
    assert_eq!(dmg, vec![25, 18, 18]);
    assert_eq!(s.gun().clip, 17);
}

#[test]
fn m4_silencer_takes_two_seconds_and_changes_damage() {
    let mut s = Sim::with(Id::M4a1);
    assert_eq!(s.step(attack).shots[0].damage, 32);
    s.idle(0.5);
    s.step(|c| c.attack2 = true);
    assert!(s.hold(1.9, attack).is_empty(), "busy attaching the silencer");
    s.idle(0.2);
    assert_eq!(s.step(attack).shots[0].damage, 33);
}

#[test]
fn awp_unscopes_on_fire_and_rescopes() {
    let mut s = Sim::with(Id::Awp);
    s.step(|c| c.attack2 = true);
    assert_eq!(s.gun().fov(), 40);
    assert_eq!(s.l.max_speed(), 150.0);
    let shot = s.step(attack).shots[0];
    assert!(shot.spread < 0.01, "scoped and still: {}", shot.spread);
    assert_eq!(s.gun().fov(), 90, "bolt cycling drops the scope");
    s.idle(1.6);
    assert_eq!(s.gun().fov(), 40, "and it comes back");
    // Unscoped AWP is inaccurate (each zoom press has a 0.3 s lockout).
    s.step(|c| c.attack2 = true);
    s.idle(0.35);
    s.step(|c| c.attack2 = true);
    s.idle(0.4);
    assert_eq!(s.gun().fov(), 90);
    assert!(s.step(attack).shots[0].spread >= 0.08);
}

#[test]
fn shotgun_reloads_shell_by_shell_and_firing_interrupts() {
    let mut s = Sim::with(Id::M3);
    s.hold(1.0, attack); // 2 shots: at 0 and 0.875 s
    let clip = s.gun().clip;
    assert_eq!(clip, 6);
    s.idle(1.0); // reload is ignored until the pump cycle is over
    s.step(|c| c.reload = true);
    s.idle(0.9);
    assert_eq!(s.gun().clip, 6, "first shell lands 0.55 + 0.45 s after the start");
    s.idle(0.15);
    assert_eq!(s.gun().clip, 7);
    s.idle(0.45);
    assert_eq!((s.gun().clip, s.gun().reserve), (8, 30));
    // Firing interrupts: start again, shoot during the reload.
    s.hold(0.3, attack);
    s.idle(1.0);
    s.step(|c| c.reload = true);
    s.idle(0.6);
    let before = s.gun().clip;
    assert_eq!(s.step(attack).shots.len(), 1, "can fire after 0.55 s");
    s.idle(1.0);
    assert_eq!(s.gun().clip, before - 1, "the interrupted reload stopped");
}

#[test]
fn spread_offsets_are_triangular_and_bounded() {
    let (mut sum, mut sumsq) = (0.0f64, 0.0f64);
    for seed in 0..20000u64 {
        let x = rand01(seed, 0) - 0.5 + rand01(seed, 1) - 0.5;
        assert!((-1.0..=1.0).contains(&x));
        sum += x as f64;
        sumsq += (x * x) as f64;
    }
    let mean = sum / 20000.0;
    let var = sumsq / 20000.0 - mean * mean;
    assert!(mean.abs() < 0.01);
    assert!((var - 1.0 / 6.0).abs() < 0.01, "triangular(-1,1) variance is 1/6, got {var}");
}

#[test]
fn grenades_throw_on_release_and_cycle() {
    let mut l = Loadout::knife_only();
    assert!(l.give(Id::Flashbang, 0.0) && l.give(Id::Flashbang, 0.0));
    assert!(!l.give(Id::Flashbang, 0.0), "only two flashes");
    assert!(l.give(Id::HeGrenade, 0.0));
    let mut s = Sim { l, now: 0.0, tick: 0 };
    s.step(|c| c.slot = 4);
    s.idle(1.0);
    assert_eq!(s.l.active_id(), Id::HeGrenade);
    s.step(|c| c.slot = 4); // next type
    s.idle(1.0);
    assert_eq!(s.l.active_id(), Id::Flashbang);
    // Holding the pin never throws; letting go does.
    let held: Vec<_> = (0..128).filter_map(|_| s.step(attack).throw).collect();
    assert!(held.is_empty());
    let thrown = s.step(|_| {}).throw;
    assert_eq!(thrown.map(|t| t.0), Some(crate::grenades::Kind::Flash));
    assert_eq!(s.l.grenades, [1, 1, 0]);
    // A quick tap still throws, once the pin time (0.5 s) is up.
    s.idle(1.0);
    s.step(attack);
    let early: Vec<_> = (0..32).filter_map(|_| s.step(|_| {}).throw).collect();
    assert!(early.is_empty(), "not before 0.5 s");
    let later: Vec<_> = (0..64).filter_map(|_| s.step(|_| {}).throw).collect();
    assert_eq!(later.len(), 1);
    assert_eq!(s.l.grenades, [1, 0, 0]);
}
