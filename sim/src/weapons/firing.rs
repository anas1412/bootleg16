use super::*;

pub(super) fn fire(g: &mut GunState, gd: &data::Gun, c: &Context, out: &mut Fired, punch: &mut [f32; 3]) {
    {
        let mode = gd.modes[g.mode as usize];
        if g.clip == 0 {
            out.dry_fire = true;
            g.next_primary = c.now + 0.2;
            return;
        }
        // Semi-auto: one shot per click (shots_fired resets when the button is released).
        let semi = gd.semi_auto && mode.burst.is_none();
        if semi {
            g.shots_fired += 1;
            if g.shots_fired > 1 {
                return;
            }
        }
        let since_last = if g.last_fire > 0.0 { Some((c.now - g.last_fire) as f32) } else { None };

        // Spread from the accuracy left by the previous shot (G3SG1 updates first).
        let mut acc_before = g.accuracy;
        if let (Accuracy::SniperTime { base, k, cap, first_shot, .. }, Spread::Sniper { rule: SniperRule::G3, .. }) = (gd.accuracy, mode.spread) {
            g.accuracy = match since_last {
                Some(dt) => (dt * k + base).min(cap),
                None => first_shot.unwrap_or(g.accuracy),
            };
            acc_before = g.accuracy;
        }
        let spread = spread(&mode.spread, acc_before, c, g.zoom > 0);

        // Accuracy for the next shot.
        if !semi {
            g.delay_fire = true;
            g.shots_fired += 1;
        }
        match gd.accuracy {
            Accuracy::Auto { power, div, fdiv, add, cap, .. } => {
                let n = g.shots_fired;
                let p = if power == 3 { n * n * n } else { n * n };
                let step = if div > 0 { (p / div) as f32 } else { p as f32 / fdiv };
                g.accuracy = (step + add).min(cap);
            }
            Accuracy::PistolTime { k, s, lo, hi, .. } => {
                if let Some(dt) = since_last {
                    g.accuracy = (g.accuracy - (k - dt) * s).clamp(lo, hi);
                }
            }
            Accuracy::SniperTime { base, k, cap, .. } if !matches!(mode.spread, Spread::Sniper { rule: SniperRule::G3, .. }) => {
                if let Some(dt) = since_last {
                    g.accuracy = (dt * k + base).min(cap);
                }
            }
            _ => {}
        }
        g.last_fire = c.now;

        g.clip -= 1;
        let side_offset = if g.id == Id::Elite { if g.clip % 2 == 0 { -5.0 } else { 5.0 } } else { 0.0 };
        out.shots.push(Shot {
            weapon: g.id,
            damage: mode.damage,
            range_mod: mode.range_mod,
            penetration: gd.penetration,
            distance: gd.distance,
            bullet: gd.bullet,
            spread,
            angles: [c.pitch + punch[0], c.yaw + punch[1]],
            side_offset,
            seed: c.seed,
            pellets: gd.pellets,
            silenced: matches!(g.id, Id::M4a1 | Id::Usp) && g.mode == 1,
        });
        let cycle = if g.zoom > 0 { gd.zoomed_cycle.unwrap_or(mode.cycle) } else { mode.cycle };
        g.next_primary = c.now + cycle as f64;
        if let Some(b) = mode.burst {
            g.burst = Some((2, c.now + b.delays[0] as f64, spread));
        }
        if gd.unscope_on_fire && g.zoom > 0 {
            g.resume_zoom = g.zoom;
            g.zoom = 0;
        }
        kick(punch, g, gd, c);
    }
}

pub(super) fn start_reload(g: &mut GunState, gd: &data::Gun, now: f64) -> Option<f64> {
    if g.reserve == 0 || g.clip >= gd.clip || now <= g.next_primary {
        return None;
    }
    g.zoom = 0;
    g.resume_zoom = 0;
    if gd.pellets.is_some() {
        if g.shell_state == 0 {
            g.shell_state = 1;
            g.shell_at = now + 0.55;
            g.next_primary = now + 0.55;
        }
        return None;
    }
    g.reloading = true;
    g.shots_fired = 0;
    g.delay_fire = false;
    if let Accuracy::Auto { on_reload, .. } = gd.accuracy {
        g.accuracy = on_reload;
    } else if let Accuracy::PistolTime { initial, .. } = gd.accuracy {
        g.accuracy = initial;
    } else if let Accuracy::SniperTime { on_reload: Some(a), .. } = gd.accuracy {
        g.accuracy = a;
    }
    Some(now + gd.reload as f64)
}

pub(super) fn shotgun_reload_step(g: &mut GunState, gd: &data::Gun, p: &data::Pellets, now: f64) {
    if g.shell_state == 0 || now < g.shell_at {
        return;
    }
    match g.shell_state {
        1 => {
            g.shell_state = 2;
            g.shell_at = now + p.shell_time as f64;
        }
        _ => {
            if g.reserve > 0 && g.clip < gd.clip {
                g.clip += 1;
                g.reserve -= 1;
            }
            g.shell_state = if g.clip < gd.clip && g.reserve > 0 { 1 } else { 0 };
            g.shell_at = now;
        }
    }
}

/// The spread number for a shot in the current movement state.
pub(super) fn spread(s: &Spread, acc: f32, c: &Context, zoomed: bool) -> f32 {
    let air = !c.on_ground;
    match *s {
        Spread::Rifle { air: a, fast_speed, fast, still } => {
            let (b, k) = if air {
                a
            } else if c.speed_2d > fast_speed {
                fast
            } else {
                still
            };
            b + k * acc
        }
        Spread::Smg { air: a, fast, still } => {
            if air {
                a * acc
            } else if let Some((speed, k)) = fast.filter(|(s, _)| c.speed_2d > *s) {
                let _ = speed;
                k * acc
            } else {
                still * acc
            }
        }
        Spread::Pistol { air: a, moving, duck, stand } => {
            let k = if air {
                a
            } else if c.speed_2d > 0.0 {
                moving
            } else if c.crouched {
                duck
            } else {
                stand
            };
            k * (1.0 - acc)
        }
        Spread::Sniper { air: a, moving, duck, stand, unscoped, rule } => {
            let moving_value = moving.iter().find(|(speed, _)| c.speed_2d > *speed).map(|(_, v)| *v);
            let (base, uses_acc) = if air {
                (a, true)
            } else if let Some(v) = moving_value {
                (v, false)
            } else if c.crouched {
                (duck, true)
            } else {
                (stand, true)
            };
            let extra = if zoomed { 0.0 } else { unscoped };
            match rule {
                SniperRule::Flat => base + extra,
                SniperRule::G3 => (base + extra) * (1.0 - acc),
                SniperRule::Sg550 => (if uses_acc { base * (1.0 - acc) } else { base }) + extra,
            }
        }
    }
}

/// Recoil of one main shot (burst follow-ups don't kick).
pub(super) fn kick(punch: &mut [f32; 3], g: &mut GunState, gd: &data::Gun, c: &Context) {
    match gd.recoil {
        Recoil::Kick { smg_order, sets } => {
            let moving = c.speed_2d > 0.0;
            let air = !c.on_ground;
            let set = if smg_order {
                if air {
                    sets[1]
                } else if moving {
                    sets[0]
                } else if c.crouched {
                    sets[2]
                } else {
                    sets[3]
                }
            } else if moving {
                sets[0]
            } else if air {
                sets[1]
            } else if c.crouched {
                sets[2]
            } else {
                sets[3]
            };
            let [up_base, lat_base, up_mod, lat_mod, up_max, lat_max, dir_change] = set;
            let n = g.shots_fired as f32;
            let (up, lat) = if g.shots_fired <= 1 { (up_base, lat_base) } else { (n * up_mod + up_base, n * lat_mod + lat_base) };
            punch[0] = (punch[0] - up).max(-up_max);
            if g.direction == 1 {
                punch[1] = (punch[1] + lat).min(lat_max);
            } else {
                punch[1] = (punch[1] - lat).max(-lat_max);
            }
            if rand_int(c.seed, 10, 0, dir_change as i32) == 0 {
                g.direction = 1 - g.direction;
            }
        }
        Recoil::Flat(p) => punch[0] -= p,
        Recoil::AutoSniper { lo, hi } => {
            punch[0] -= rand_range(c.seed, 4, lo, hi) + punch[0] * 0.25;
            punch[1] += rand_range(c.seed, 5, -0.75, 0.75);
        }
        Recoil::Shotgun { ground, air } => {
            let (lo, hi) = if c.on_ground { ground } else { air };
            punch[0] -= rand_int(c.seed, 1, lo, hi) as f32;
        }
        Recoil::None => {}
    }
}

