//! Hidden skill, for fair matches (never shown: casual has no ranks). Each player is a
//! Gaussian belief about their skill (mu, sigma), updated from wins and losses with the
//! Weng-Lin Bayesian approximation (Thurstone-Mosteller, the model OpenSkill implements):
//! open and patent-free, built for teams. Upsets move ratings more than expected results;
//! new players (big sigma) move fast, established ones slowly; sigma creeps back up between
//! matches so ratings keep following people who improve.
use serde::{Deserialize, Serialize};

pub const MU: f64 = 25.0;
pub const SIGMA: f64 = MU / 3.0;
/// The skill difference that gives the better side ~76% to win.
const BETA: f64 = MU / 6.0;
/// How much uncertainty comes back each match (ratings never freeze).
const TAU: f64 = MU / 300.0;
/// Sigma never shrinks past this share per match (keeps the update stable).
const KAPPA: f64 = 0.0001;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct Rating {
    pub mu: f64,
    pub sigma: f64,
    pub matches: u32,
}

impl Default for Rating {
    fn default() -> Rating {
        Rating { mu: MU, sigma: SIGMA, matches: 0 }
    }
}

impl Rating {
    /// A cautious single number (we're ~99% sure they're at least this good).
    pub fn ordinal(&self) -> f64 {
        self.mu - 3.0 * self.sigma
    }
}

fn pdf(x: f64) -> f64 {
    (-x * x / 2.0).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

/// The standard normal CDF (Abramowitz-Stegun 7.1.26 erf: error < 1.5e-7).
pub fn cdf(x: f64) -> f64 {
    let z = x.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * z);
    let erf = 1.0 - t * (0.254_829_592 + t * (-0.284_496_736 + t * (1.421_413_741 + t * (-1.453_152_027 + t * 1.061_405_429)))) * (-z * z).exp();
    if x >= 0.0 {
        0.5 * (1.0 + erf)
    } else {
        0.5 * (1.0 - erf)
    }
}

fn team(ratings: &[Rating]) -> (f64, f64) {
    (ratings.iter().map(|r| r.mu).sum(), ratings.iter().map(|r| r.sigma * r.sigma).sum())
}

/// The chance team `a` beats team `b`.
pub fn win_chance(a: &[Rating], b: &[Rating]) -> f64 {
    let ((ma, sa), (mb, sb)) = (team(a), team(b));
    let n = (a.len() + b.len()) as f64;
    cdf((ma - mb) / (sa + sb + n * BETA * BETA).sqrt())
}

/// After a match: `winners` beat `losers` (everyone's rating, updated in place).
pub fn update(winners: &mut [Rating], losers: &mut [Rating]) {
    for r in winners.iter_mut().chain(losers.iter_mut()) {
        r.sigma = (r.sigma * r.sigma + TAU * TAU).sqrt();
    }
    let ((mw, sw), (ml, sl)) = (team(winners), team(losers));
    let c = (sw + sl + 2.0 * BETA * BETA).sqrt();
    let t = (mw - ml) / c;
    // v: how surprising the result was (big for an upset); w: how much it tells us.
    let v = pdf(t) / cdf(t).max(1e-12);
    let w = v * (v + t);
    for (side, s2, sign) in [(winners, sw, 1.0), (losers, sl, -1.0)] {
        let omega = sign * s2 / c * v;
        let gamma = s2.sqrt() / c;
        let delta = gamma * s2 / (c * c) * w;
        for r in side.iter_mut() {
            let share = r.sigma * r.sigma / s2;
            r.mu += share * omega;
            r.sigma *= (1.0 - share * delta).max(KAPPA).sqrt();
            r.matches += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratings_move_the_right_way_and_upsets_move_more() {
        assert!((cdf(0.0) - 0.5).abs() < 1e-7 && (cdf(1.96) - 0.975).abs() < 1e-4 && (cdf(-1.0) - 0.1587).abs() < 1e-4);
        let even = || vec![Rating::default(); 5];
        assert!((win_chance(&even(), &even()) - 0.5).abs() < 1e-9);
        let (mut w, mut l) = (even(), even());
        update(&mut w, &mut l);
        assert!(w[0].mu > MU && l[0].mu < MU, "winners up, losers down");
        assert!((w[0].mu - MU + l[0].mu - MU).abs() < 1e-9, "equal teams: symmetric");
        assert!(w[0].sigma < SIGMA, "we know more after a match");
        // Strong beat weak: expected, small change. Weak beat strong: an upset, big change.
        let strong = vec![Rating { mu: 32.0, sigma: 3.0, matches: 50 }; 5];
        let weak = vec![Rating { mu: 20.0, sigma: 3.0, matches: 50 }; 5];
        assert!(win_chance(&strong, &weak) > 0.8);
        let (mut s1, mut w1) = (strong.clone(), weak.clone());
        update(&mut s1, &mut w1);
        let (mut w2, mut s2) = (weak.clone(), strong.clone());
        update(&mut w2, &mut s2);
        assert!(w2[0].mu - 20.0 > (s1[0].mu - 32.0) * 3.0, "an upset moves ratings much more");
        // A new player moves faster than an established one in the same result.
        let mut mixed = vec![Rating::default(), strong[0], strong[0], strong[0], strong[0]];
        let mut other = weak.clone();
        update(&mut mixed, &mut other);
        assert!(mixed[0].mu - MU > mixed[1].mu - 32.0);
    }
}
