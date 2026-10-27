//! The one place GoldSrc units meet Godot's. The simulation keeps GoldSrc's: z up, 1 unit =
//! 1 inch (a player is 72 tall). Godot scenes are y up, in metres, looking down -z.
use godot::prelude::*;

/// Metres per GoldSrc unit.
pub const METRES: f32 = 0.0254;

/// A Godot position (metres, y up) in GoldSrc units (z up).
#[cfg(test)]
pub fn to_sim(v: Vector3) -> [f32; 3] {
    [v.x / METRES, -v.z / METRES, v.y / METRES]
}

/// A GoldSrc position in Godot metres.
pub fn to_godot(v: [f32; 3]) -> Vector3 {
    Vector3::new(v[0] * METRES, v[2] * METRES, -v[1] * METRES)
}

/// A Godot camera's yaw and pitch (radians; yaw 0 looks down -z, pitch up is positive) as
/// GoldSrc view angles (degrees; yaw 0 looks down +x, pitch down is positive).
pub fn view_to_sim(yaw: f32, pitch: f32) -> (f32, f32) {
    (yaw.to_degrees() + 90.0, -pitch.to_degrees())
}

/// A GoldSrc yaw (degrees) as a Godot yaw (radians).
pub fn yaw_to_godot(yaw: f32) -> f32 {
    (yaw - 90.0).to_radians()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_round_trip_and_axes_match() {
        let p = [100.0, -40.0, 72.0];
        let g = to_godot(p);
        assert!((g.y - 72.0 * METRES).abs() < 1e-6, "up is up");
        let back = to_sim(g);
        assert!((0..3).all(|i| (back[i] - p[i]).abs() < 1e-3));
        // a Godot camera turned to look down +x (yaw -90 degrees) is GoldSrc yaw 0
        let (yaw, pitch) = view_to_sim(-std::f32::consts::FRAC_PI_2, 0.1);
        assert!(yaw.abs() < 1e-4 && pitch < 0.0);
        assert!((yaw_to_godot(0.0) + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }
}
