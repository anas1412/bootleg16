//! Players are solid to each other: nobody stands inside someone else. One rule for the
//! server and the client's prediction, so they agree.
use crate::collision::{Hull, World};
use crate::Vec3;

/// Landing on (or touching the top of) someone's head is not a collision: you can stand there.
const STAND_ON: f32 = 6.0;

/// `me` pushed out of every box in `others` it overlaps (sideways, by the smaller overlap).
/// A push into a wall is skipped.
pub fn push_out(world: &World, hull: Hull, mut me: Vec3, others: &[(Vec3, Hull)]) -> Vec3 {
    for (o, oh) in others {
        let overlap = |axis: usize| (me[axis] + hull.maxs()[axis]).min(o[axis] + oh.maxs()[axis]) - (me[axis] + hull.mins()[axis]).max(o[axis] + oh.mins()[axis]);
        let (dx, dy, dz) = (overlap(0), overlap(1), overlap(2));
        if dx <= 0.0 || dy <= 0.0 || dz <= STAND_ON {
            continue;
        }
        // Out along the axis with the smaller overlap, away from the other player.
        let axis = if dx <= dy { 0 } else { 1 };
        let depth = if axis == 0 { dx } else { dy } + 0.05;
        let mut moved = me;
        moved[axis] += if me[axis] >= o[axis] { depth } else { -depth };
        if !world.is_solid(hull, moved) {
            me = moved;
        }
    }
    me
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_players_are_pushed_apart_but_can_stand_on_a_head() {
        let world = World::flat_floor(-100.0);
        let other = ([0.0, 0.0, 0.0], Hull::Standing);
        // Side by side, overlapping by 8 units on x: pushed out to touching.
        let out = push_out(&world, Hull::Standing, [24.0, 3.0, 0.0], &[other]);
        assert!((out[0] - 32.0).abs() < 0.2 && out[1] == 3.0, "{out:?}");
        // Already apart: untouched.
        assert_eq!(push_out(&world, Hull::Standing, [40.0, 0.0, 0.0], &[other]), [40.0, 0.0, 0.0]);
        // On top of a head (72 up): untouched.
        assert_eq!(push_out(&world, Hull::Standing, [5.0, 0.0, 72.0], &[other]), [5.0, 0.0, 72.0]);
    }
}
