use super::*;
use lolping_native::{WHEEL_INNER_RADIUS, WHEEL_OUTER_RADIUS};
use std::path::Path as FilePath;

pub(super) const SURFACE_SIZE: f32 = 408.;
const CENTER: f32 = SURFACE_SIZE / 2.;

pub(super) fn polar(center: Point<Pixels>, radius: f32, angle: f32) -> Point<Pixels> {
    center + point(px(radius * angle.cos()), px(radius * angle.sin()))
}

fn sector(center: Point<Pixels>, slot: usize) -> Path<Pixels> {
    let start = (-112.5 + slot as f32 * 45.).to_radians();
    let end = start + 45_f32.to_radians();
    let mut path = PathBuilder::fill();
    path.move_to(polar(center, WHEEL_INNER_RADIUS, start));
    for i in 0..=16 {
        path.line_to(polar(
            center,
            WHEEL_OUTER_RADIUS,
            start + (end - start) * i as f32 / 16.,
        ));
    }
    for i in (0..=16).rev() {
        path.line_to(polar(
            center,
            WHEEL_INNER_RADIUS,
            start + (end - start) * i as f32 / 16.,
        ));
    }
    path.close();
    path.build().expect("valid wheel sector")
}

/// Shared by the desktop overlay and the editor so slot angles, art and radii stay identical.
pub(super) fn surface(
    assets: &FilePath,
    layout: &[String; 8],
    highlights: [f32; 8],
    previous: Option<&[String; 8]>,
    progress: f32,
    label: &str,
) -> Div {
    let mut root = div()
        .relative()
        .size(px(SURFACE_SIZE))
        .flex_shrink_0()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    let center = bounds.center();
                    for (slot, strength) in highlights.into_iter().enumerate() {
                        // Keep the original translucent gray/gold appearance at both endpoints.
                        let base = rgba(0x242b32ed);
                        let active = rgba(0x88826cdf);
                        let color = Rgba {
                            r: base.r + (active.r - base.r) * strength,
                            g: base.g + (active.g - base.g) * strength,
                            b: base.b + (active.b - base.b) * strength,
                            a: base.a + (active.a - base.a) * strength,
                        };
                        window.paint_path(sector(center, slot), color);
                        let mut border = PathBuilder::stroke(px(1.));
                        let angle = (-112.5 + slot as f32 * 45.).to_radians();
                        border.move_to(polar(center, WHEEL_INNER_RADIUS, angle));
                        border.line_to(polar(center, WHEEL_OUTER_RADIUS, angle));
                        if let Ok(path) = border.build() {
                            window.paint_path(path, rgba(0xc8aa6e88));
                        }
                    }
                },
            )
            .absolute()
            .size_full(),
        );
    let center = point(px(CENTER), px(CENTER));
    for (slot, id) in layout.iter().enumerate() {
        let ping = &PINGS[index(id).unwrap_or(8)];
        let old_slot = previous.and_then(|old| old.iter().position(|old_id| old_id == id));
        let offset = old_slot
            .map(|old| (old as f32 - slot as f32 + 4.).rem_euclid(8.) - 4.)
            .unwrap_or(0.);
        let angle = (-90. + (slot as f32 + offset * (1. - progress)) * 45.).to_radians();
        let pos = polar(center, 137., angle);
        let strength = highlights[slot];
        let entering = if previous.is_some() && old_slot.is_none() {
            progress
        } else {
            1.
        };
        let icon_size = (48. + strength * 10.) * (0.8 + entering * 0.2);
        root = root.child(
            img(assets.join(format!("textures/{}.png", ping.icon)))
                .absolute()
                .left(pos.x - px(icon_size / 2.))
                .top(pos.y - px(icon_size / 2.))
                .size(px(icon_size))
                .opacity(entering),
        );
    }
    root.child(
        div()
            .absolute()
            .left(px(CENTER - WHEEL_INNER_RADIUS))
            .top(px(CENTER - WHEEL_INNER_RADIUS))
            .size(px(WHEEL_INNER_RADIUS * 2.))
            .rounded_full()
            .border_2()
            .border_color(rgb(0xc8aa6e))
            .bg(rgba(0x123033f5))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .text_size(px(16.))
            .text_color(rgb(0xf0dfb9))
            .child(img(assets.join("textures/generic_ping.png")).size(px(34.)))
            .child(label.to_owned()),
    )
}

pub(super) struct Highlight {
    values: [f32; 8],
    updated: Instant,
}

impl Default for Highlight {
    fn default() -> Self {
        Self {
            values: [0.; 8],
            updated: Instant::now(),
        }
    }
}

impl Highlight {
    pub(super) fn frame(&mut self, active: Option<usize>, window: &mut Window) -> [f32; 8] {
        let delta = self.updated.elapsed().as_secs_f32().min(0.05);
        self.updated = Instant::now();
        let blend = 1. - (-delta * 24.).exp();
        for (slot, value) in self.values.iter_mut().enumerate() {
            let target = if active == Some(slot) { 1. } else { 0. };
            *value += (target - *value) * blend;
            if (*value - target).abs() < 0.002 {
                *value = target;
            } else {
                window.request_animation_frame();
            }
        }
        self.values
    }
}

pub(super) fn settle(progress: f32) -> f32 {
    1. - (1. - progress.clamp(0., 1.)).powi(4)
}
