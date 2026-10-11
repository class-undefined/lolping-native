pub mod catalog;
pub mod helper;
pub mod settings;

/// Screen coordinates: north is slot 0, clockwise, with the original 86-point dead zone.
pub fn pick_slice(dx: f32, dy: f32) -> Option<usize> {
    if !dx.is_finite() || !dy.is_finite() || dx.hypot(dy) < 86.0 {
        return None;
    }
    Some(((dy.atan2(dx).to_degrees() + 112.5).rem_euclid(360.0) / 45.0) as usize % 8)
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
}
