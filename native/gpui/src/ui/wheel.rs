use super::*;
use lolping_native::{WHEEL_INNER_RADIUS, WHEEL_OUTER_RADIUS};
use std::path::Path as FilePath;

pub(super) const SURFACE_SIZE: f32 = 408.;
pub(super) const DIRECTIONS: [&str; 8] = ["上", "右上", "右", "右下", "下", "左下", "左", "左上"];
const CENTER: f32 = SURFACE_SIZE / 2.;

pub(super) fn polar(center: Point<Pixels>, radius: f32, angle: f32) -> Point<Pixels> {
    center + point(px(radius * angle.cos()), px(radius * angle.sin()))
}

fn sector(center: Point<Pixels>, slot: usize, inner: f32, outer: f32) -> Path<Pixels> {
    let start = (-112.5 + slot as f32 * 45.).to_radians() + 0.009;
    let end = start + 45_f32.to_radians() - 0.018;
    let mut path = PathBuilder::fill();
    path.move_to(polar(center, inner, start));
    for i in 0..=24 {
        path.line_to(polar(center, outer, start + (end - start) * i as f32 / 24.));
    }
    for i in (0..=24).rev() {
        path.line_to(polar(center, inner, start + (end - start) * i as f32 / 24.));
    }
    path.close();
    path.build().expect("valid wheel sector")
}

fn disc(window: &mut Window, center: Point<Pixels>, radius: f32, fill: u32, edge: u32) {
    window.paint_quad(quad(
        Bounds::new(
            center - point(px(radius), px(radius)),
            size(px(radius * 2.), px(radius * 2.)),
        ),
        px(radius),
        rgb(fill),
        px(1.),
        rgb(edge),
        BorderStyle::Solid,
    ));
}

/// Shared by the desktop overlay and the editor so slot angles, art and radii stay identical.
pub(super) fn surface(
    assets: &FilePath,
    layout: &[String; 8],
    highlights: [f32; 8],
    previous: Option<&[String; 8]>,
    progress: f32,
    label: &str,
    sublabel: &str,
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
                    disc(
                        window,
                        center + point(px(0.), px(4.)),
                        196.,
                        0x080d13,
                        0x080d13,
                    );
                    disc(window, center, 196., 0x121a24, 0x3a4655);
                    disc(window, center, 191., 0x090f16, 0x090f16);
                    for (slot, strength) in highlights.into_iter().enumerate() {
                        window.paint_path(
                            sector(center, slot, WHEEL_INNER_RADIUS, WHEEL_OUTER_RADIUS),
                            rgb(0x26313e),
                        );
                        window.paint_path(
                            sector(
                                center,
                                slot,
                                WHEEL_INNER_RADIUS + 2.,
                                WHEEL_OUTER_RADIUS - 2.,
                            ),
                            rgb(0x1c2632),
                        );
                        window.paint_path(
                            sector(
                                center,
                                slot,
                                WHEEL_INNER_RADIUS + 2.,
                                WHEEL_OUTER_RADIUS - 2.,
                            ),
                            Hsla::from(rgb(0x69c5f0)).opacity(strength * 0.14),
                        );
                        window.paint_path(
                            sector(center, slot, WHEEL_OUTER_RADIUS - 3., WHEEL_OUTER_RADIUS),
                            Hsla::from(rgb(0x6bc9f3)).opacity(0.12 + strength * 0.88),
                        );
                        window.paint_path(
                            sector(center, slot, WHEEL_INNER_RADIUS, WHEEL_INNER_RADIUS + 1.),
                            Hsla::from(rgb(0x7196ae)).opacity(0.35),
                        );
                    }
                    disc(window, center, WHEEL_INNER_RADIUS - 4., 0x101923, 0x42556a);
                    disc(window, center, WHEEL_INNER_RADIUS - 9., 0x131e29, 0x1f303f);
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
        let icon_size = (38. + strength * 5.) * (0.8 + entering * 0.2);
        root = root.child(
            div()
                .absolute()
                .left(pos.x - px(49.))
                .top(pos.y - px(31.))
                .w(px(98.))
                .h(px(66.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(3.))
                .opacity(entering)
                .child(img(assets.join(format!("textures/{}.png", ping.icon))).size(px(icon_size)))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(if strength > 0.4 { 0xf2f8fc } else { 0xa7b7c6 }))
                        .child(ping.name),
                ),
        );
    }
    root.child(
        div()
            .absolute()
            .left(px(CENTER - 72.))
            .top(px(CENTER - 58.))
            .w(px(144.))
            .h(px(116.))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(0x71c9ee))
                    .child(sublabel.to_owned()),
            )
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xe5eef6))
                    .child(label.to_owned()),
            )
            .child(div().w(px(24.)).h(px(2.)).bg(rgb(0x608aa4))),
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
