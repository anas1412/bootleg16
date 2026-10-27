//! How the first-person view moves, as CS 1.6 does it (`V_CalcBob` and `V_CalcNormalRefdef`
//! in client.so, read with REA). The bob follows your horizontal speed on a 0.8 s cycle (rising
//! for the first half), between -7 and 4 units: the camera moves up and down by it. Stepping
//! up a stair, the view lags behind and catches up at 150 units a second (never more than 18
//! behind), so stairs glide instead of jerking. Server corrections to your predicted position
//! glide out instead of snapping. Only what you see moves: your shots still come from your eye.

/// `cl_bobcycle`, `cl_bobup`, `cl_bob` (client.so defaults) and the bob's limits.
pub const CYCLE: f32 = 0.8;
pub const UP: f32 = 0.5;
pub const AMOUNT: f32 = 0.01;
pub const LIMITS: (f32, f32) = (-7.0, 4.0);
/// Stair smoothing: how fast the view catches up, and how far it may lag.
pub const STEP_SPEED: f32 = 150.0;
pub const STEP_MAX: f32 = 18.0;

/// The bob for a horizontal speed at a point in the cycle (`V_CalcBob`).
pub fn bob_at(time: f32, speed: f32) -> f32 {
    let cycle = (time % CYCLE) / CYCLE;
    let phase = if cycle < UP { std::f32::consts::PI * cycle / UP } else { std::f32::consts::PI + std::f32::consts::PI * (cycle - UP) / (1.0 - UP) };
    let b = speed * AMOUNT;
    (b * 0.3 + b * 0.7 * phase.sin()).clamp(LIMITS.0, LIMITS.1)
}

/// The view's lag behind a player who went up to `z` (`V_CalcNormalRefdef`'s step smoothing):
/// the new view z, never below `z - STEP_MAX`.
pub fn step_up(view_z: f32, z: f32, dt: f32) -> f32 {
    (view_z + dt * STEP_SPEED).min(z).max(z - STEP_MAX)
}

/// Prediction corrections fade with this time constant; beyond this distance they snap.
pub const SMOOTH_SECONDS: f32 = 0.05;
pub const SNAP_DISTANCE: f32 = 64.0;

/// The view's own motion over the eye: the bob and the stair glide (GoldSrc units, z up).
#[derive(Clone, Copy, Debug, Default)]
pub struct View {
    time: f32,
    bob: f32,
    /// Where the view is on the way up a stair.
    view_z: Option<f32>,
    /// What's left of server corrections, still gliding out.
    smooth: [f32; 3],
}

impl View {
    /// One frame: `speed` horizontal, `z` the player's origin height. Returns how far up the
    /// camera sits over the eye (bob plus stair lag, negative while catching up).
    pub fn update(&mut self, dt: f32, on_ground: bool, speed: f32, z: f32) -> f32 {
        if on_ground {
            self.time += dt;
            self.bob = bob_at(self.time, speed);
        }
        let view_z = match self.view_z {
            Some(v) if on_ground && z > v => step_up(v, z, dt),
            _ => z,
        };
        self.view_z = Some(view_z);
        self.bob + (view_z - z)
    }

    /// The bob now (the gun moves forward 0.4 of it and turns a little).
    pub fn bob(&self) -> f32 {
        self.bob
    }

    /// A server correction `moved` (where prediction was wrong) glides out over ~0.1 s; big
    /// jumps (respawn, teleport) snap. Returns the offset to add to the eye this frame.
    pub fn correct(&mut self, moved: [f32; 3], dt: f32) -> [f32; 3] {
        let s: [f32; 3] = std::array::from_fn(|i| self.smooth[i] - moved[i]);
        let len = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
        let k = if len > SNAP_DISTANCE { 0.0 } else { (-dt / SMOOTH_SECONDS).exp() };
        self.smooth = s.map(|v| v * k);
        self.smooth
    }

    /// Dead, spectating or flying: start over.
    pub fn reset(&mut self) {
        *self = View::default();
    }
}

/// The recoil punch between the last two ticks (`a`: how far into the next tick we are).
pub fn punch_at(prev: [f32; 3], now: [f32; 3], a: f32) -> [f32; 3] {
    std::array::from_fn(|i| prev[i] + (now[i] - prev[i]) * a)
}

/// A horizontal field of view (90 = normal, at 4:3, as CS sets it) as a vertical one, radians.
pub fn vertical_fov(horizontal_degrees: f32) -> f32 {
    2.0 * ((horizontal_degrees.to_radians() / 2.0).tan() * 0.75).atan()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standing_still_does_not_bob_and_running_stays_within_cs_limits() {
        assert_eq!(bob_at(0.37, 0.0), 0.0);
        for i in 0..200 {
            let b = bob_at(i as f32 * 0.013, 250.0);
            assert!((LIMITS.0..=LIMITS.1).contains(&b), "{b}");
        }
        // at 250 u/s the peak is 0.3*2.5 + 0.7*2.5 = 2.5 a quarter into the cycle
        assert!((bob_at(CYCLE * UP / 2.0, 250.0) - 2.5).abs() < 1e-3);
    }

    #[test]
    fn a_correction_glides_out_and_a_teleport_snaps() {
        let mut v = View::default();
        let o = v.correct([4.0, 0.0, 0.0], 0.05);
        assert!(o[0] < -1.0 && o[0] > -2.0, "{o:?}");
        assert_eq!(v.correct([500.0, 0.0, 0.0], 0.01), [0.0; 3]);
        assert!((vertical_fov(90.0) - 2.0 * 0.75f32.atan()).abs() < 1e-6);
    }

    #[test]
    fn a_stair_is_climbed_smoothly_and_never_lags_more_than_18() {
        // up 16 in one tick: the view starts 16 behind and catches up at 150 u/s
        let v = step_up(0.0, 16.0, 1.0 / 60.0);
        assert!((v - 2.5).abs() < 1e-3, "{v}");
        assert_eq!(step_up(0.0, 40.0, 0.0), 22.0);
        assert_eq!(step_up(15.0, 16.0, 1.0), 16.0);
    }
}
