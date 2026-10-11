pub mod catalog;
pub mod helper;
pub mod settings;

pub const WHEEL_INNER_RADIUS: f32 = 86.;
pub const WHEEL_OUTER_RADIUS: f32 = 188.;

/// Screen coordinates: north is slot 0, clockwise, with the original 86-point dead zone.
pub fn pick_slice(dx: f32, dy: f32) -> Option<usize> {
    if !dx.is_finite() || !dy.is_finite() || dx.hypot(dy) < WHEEL_INNER_RADIUS {
        return None;
    }
    Some(((dy.atan2(dx).to_degrees() + 112.5).rem_euclid(360.0) / 45.0) as usize % 8)
}

/// The editor accepts drops only on the visible annulus; desktop gestures remain unbounded.
pub fn pick_editor_slot(dx: f32, dy: f32) -> Option<usize> {
    if dx.hypot(dy) > WHEEL_OUTER_RADIUS {
        return None;
    }
    pick_slice(dx, dy)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_matches_original_compass_and_dead_zone() {
        for (slot, (x, y)) in [
            (0., -150.),
            (150., -150.),
            (150., 0.),
            (150., 150.),
            (0., 150.),
            (-150., 150.),
            (-150., 0.),
            (-150., -150.),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(pick_slice(x, y), Some(slot));
        }
        assert_eq!(pick_slice(0., 85.99), None);
        assert_eq!(pick_slice(0., 86.), Some(4));
        assert_eq!(pick_slice(f32::NAN, 0.), None);
        assert_eq!(pick_slice(0., f32::INFINITY), None);
    }

    #[test]
    fn editor_drops_respect_the_visible_ring() {
        for slot in 0..8 {
            let angle = (-90. + slot as f32 * 45.).to_radians();
            assert_eq!(
                pick_editor_slot(137. * angle.cos(), 137. * angle.sin()),
                Some(slot)
            );
        }
        assert_eq!(pick_editor_slot(0., 85.99), None);
        assert_eq!(pick_editor_slot(188., 0.), Some(2));
        assert_eq!(pick_editor_slot(188.01, 0.), None);
        assert_eq!(pick_editor_slot(180., 180.), None);
        assert_eq!(pick_editor_slot(f32::NAN, 0.), None);
        assert_eq!(pick_slice(300., 0.), Some(2));
    }
}
