use crate::macos;
use anyhow::{Context as _, Result};
use gpui::{prelude::*, *};
use lolping_native::{
    catalog::{PINGS, asset_files, index},
    helper::{Event, Helper},
    pick_slice,
    settings::{Settings, TRIGGERS},
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

actions!(lolping, [Quit, OpenSettings, Toggle]);

struct Wheel {
    origin: Point<Pixels>,
    display: DisplayId,
    hot: Option<usize>,
}
struct PlacedPing {
    origin: Point<Pixels>,
    display: DisplayId,
    ping: usize,
    born: Instant,
    duration: f32,
    size: f32,
}

struct Model {
    settings: Settings,
    enabled: bool,
    status: String,
    notice: String,
    settings_path: PathBuf,
    assets: PathBuf,
    helper_path: PathBuf,
    helper: Option<Helper>,
    generation: u64,
    simulate: bool,
    wheel: Option<Wheel>,
    pings: Vec<PlacedPing>,
    overlays: Vec<(DisplayId, Bounds<Pixels>, WindowHandle<Overlay>)>,
    settings_window: Option<WindowHandle<SettingsView>>,
}

impl Model {
    fn restart_helper(&mut self, cx: &mut Context<Self>) {
        self.generation += 1;
        self.helper = None;
        self.wheel = None;
        self.status = "正在连接输入助手…".into();
        match Helper::start(&self.helper_path, self.simulate) {
            Ok((helper, receiver)) => {
                self.helper = Some(helper);
                let generation = self.generation;
                cx.spawn(async move |this, cx| {
                    while let Ok(event) = receiver.recv().await {
                        if this
                            .update(cx, |model, cx| {
                                if model.generation != generation {
                                    return false;
                                }
                                model.handle(event, cx);
                                true
                            })
                            .ok()
                            != Some(true)
                        {
                            break;
                        }
                    }
                })
                .detach();
            }
            Err(error) => self.status = error.to_string(),
        }
        cx.notify();
    }

    fn configure(&mut self) {
        if let Some(helper) = self.helper.as_mut()
            && let Err(error) = helper.send(&self.settings.helper_config(self.enabled))
        {
            self.status = format!("输入助手连接中断：{error}");
            self.wheel = None;
        }
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        match self.settings.save(&self.settings_path) {
            Ok(()) => self.notice.clear(),
            Err(error) => self.notice = format!("设置保存失败：{error}"),
        }
        self.wheel = None;
        self.configure();
        cx.notify();
    }

    fn toggle(&mut self, cx: &mut Context<Self>) {
        self.enabled = !self.enabled;
        self.wheel = None;
        self.configure();
        macos::set_enabled(self.enabled);
        cx.notify();
    }

    fn sound(&mut self, sound: &str) {
        if !self.settings.muted
            && self.settings.volume > 0
            && !macos::play(
                &self.assets.join(format!("sounds/{sound}.wav")),
                self.settings.volume,
            )
        {
            self.notice = format!("无法播放音效 {sound}");
        }
    }

    fn spawn_ping(&mut self, ping: usize, origin: Point<Pixels>, display: DisplayId) {
        self.pings
            .retain(|p| p.born.elapsed().as_secs_f32() < p.duration);
        if self.pings.len() >= 60 {
            self.pings.remove(0);
        }
        self.pings.push(PlacedPing {
            origin,
            display,
            ping,
            born: Instant::now(),
            duration: self.settings.ping_duration_s,
            size: self.settings.ping_size_px,
        });
        self.sound(PINGS[ping].sound);
    }

    fn display_at(&self, p: Point<Pixels>) -> Option<DisplayId> {
        self.overlays
            .iter()
            .find(|(_, b, _)| b.contains(&p))
            .map(|(id, _, _)| *id)
    }

    fn preview(&mut self, ping: usize, cx: &mut Context<Self>) {
        if let Some(display) = cx.primary_display() {
            self.spawn_ping(
                ping,
                macos::display_bounds(display.id()).center(),
                display.id(),
            );
            cx.notify();
        }
    }

    fn handle(&mut self, event: Event, cx: &mut Context<Self>) {
        match event {
            Event::Ready { version } => {
                self.status = if self.simulate {
                    format!("预览模式 · 输入协议 v{version}")
                } else {
                    "输入助手已连接".into()
                };
                self.configure();
            }
            Event::WheelOpen { x, y, wheel } if self.enabled && wheel != "emote" => {
                let origin = point(px(x as f32), px(y as f32));
                self.wheel = self.display_at(origin).map(|display| Wheel {
                    origin,
                    display,
                    hot: None,
                });
            }
            Event::WheelMove { x, y } => {
                if let Some(wheel) = &mut self.wheel {
                    let hot = pick_slice(
                        x as f32 - f32::from(wheel.origin.x),
                        y as f32 - f32::from(wheel.origin.y),
                    );
                    let tick = hot.is_some() && wheel.hot != hot && self.settings.tick_sound;
                    wheel.hot = hot;
                    if tick {
                        self.sound("button");
                    }
                }
            }
            Event::WheelRelease { x, y } => {
                if let Some(wheel) = self.wheel.take()
                    && let Some(slot) = pick_slice(
                        x as f32 - f32::from(wheel.origin.x),
                        y as f32 - f32::from(wheel.origin.y),
                    )
                {
                    let ping = index(&self.settings.wheel[slot]).unwrap_or(8);
                    self.spawn_ping(ping, wheel.origin, wheel.display);
                }
            }
            Event::Click { x, y, wheel } if self.enabled && wheel != "emote" => {
                let origin = point(px(x as f32), px(y as f32));
                if let Some(display) = self.display_at(origin) {
                    self.spawn_ping(
                        index(&self.settings.click_ping_id).unwrap_or(8),
                        origin,
                        display,
                    );
                }
            }
            Event::Cancel => self.wheel = None,
            Event::Toggled { enabled } => {
                self.enabled = enabled;
                self.wheel = None;
                macos::set_enabled(enabled);
            }
            Event::Error { message, code } => {
                self.wheel = None;
                self.status = if code.as_deref() == Some("noAccess") {
                    "需要辅助功能权限。授权后点击“重新连接”。".into()
                } else {
                    format!("输入错误：{message}")
                };
            }
            Event::Disconnected => {
                self.wheel = None;
                if !self.status.contains("权限") {
                    self.status = "输入助手已断开，请重新连接。".into();
                }
            }
            _ => {}
        }
        cx.notify();
    }

    fn sync_displays(&mut self, cx: &mut Context<Self>) {
        let model = cx.entity();
        cx.defer(move |cx| sync_displays(&model, cx));
    }

    fn show_settings(&mut self, cx: &mut Context<Self>) {
        let model = cx.entity();
        cx.defer(move |cx| show_settings(&model, cx));
    }
}

// GPUI renders a newly opened window synchronously. Open outside Model::update so views
// can read the shared model during their first frame without an entity borrow conflict.
fn sync_displays(model: &Entity<Model>, cx: &mut App) {
    let displays = cx.displays();
    let current = &model.read(cx).overlays;
    if displays.len() == current.len()
        && displays.iter().all(|d| {
            current
                .iter()
                .any(|(id, b, _)| *id == d.id() && *b == macos::display_bounds(d.id()))
        })
    {
        return;
    }
    let previous = model.update(cx, |m, _| {
        m.wheel = None;
        m.pings.clear();
        std::mem::take(&mut m.overlays)
    });
    for (_, _, window) in previous {
        let _ = window.update(cx, |_, window, _| window.remove_window());
    }
    for display in displays {
        let id = display.id();
        let bounds = macos::display_bounds(id);
        let view_model = model.clone();
        match cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                display_id: Some(id),
                titlebar: None,
                focus: false,
                show: true,
                kind: WindowKind::PopUp,
                is_resizable: false,
                is_movable: false,
                is_minimizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            |window, cx| {
                macos::configure_overlay(window, bounds);
                cx.new(|cx| {
                    cx.observe(&view_model, |_, _, cx| cx.notify()).detach();
                    Overlay {
                        model: view_model,
                        display: id,
                        bounds,
                    }
                })
            },
        ) {
            Ok(window) => model.update(cx, |m, _| m.overlays.push((id, bounds, window))),
            Err(error) => model.update(cx, |m, _| m.notice = format!("无法创建信号层：{error}")),
        }
    }
    model.update(cx, |_, cx| cx.notify());
}

fn show_settings(model: &Entity<Model>, cx: &mut App) {
    if let Some(handle) = model.read(cx).settings_window
        && handle
            .update(cx, |_, window, cx| {
                window.activate_window();
                cx.activate(true);
            })
            .is_ok()
    {
        return;
    }
    let bounds = Bounds::centered(None, size(px(820.), px(790.)), cx);
    let view_model = model.clone();
    match cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(700.), px(540.))),
            titlebar: Some(TitlebarOptions {
                title: Some("lolPing · 设置".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            window.on_window_should_close(cx, |window, _| {
                macos::hide(window);
                false
            });
            cx.new(|cx| {
                cx.observe(&view_model, |_, _, cx| cx.notify()).detach();
                SettingsView {
                    model: view_model,
                    selected: 0,
                    scroll: ScrollHandle::new(),
                }
            })
        },
    ) {
        Ok(window) => model.update(cx, |m, _| m.settings_window = Some(window)),
        Err(error) => eprintln!("设置窗口创建失败：{error}"),
    }
    cx.activate(true);
}

struct Overlay {
    model: Entity<Model>,
    display: DisplayId,
    bounds: Bounds<Pixels>,
}

fn polar(center: Point<Pixels>, r: f32, angle: f32) -> Point<Pixels> {
    center + point(px(r * angle.cos()), px(r * angle.sin()))
}

fn sector(center: Point<Pixels>, slot: usize, radius: f32) -> Path<Pixels> {
    let a = (-112.5 + slot as f32 * 45.).to_radians();
    let b = a + 45_f32.to_radians();
    let mut path = PathBuilder::fill();
    path.move_to(polar(center, 86., a));
    for i in 0..=16 {
        path.line_to(polar(center, radius, a + (b - a) * i as f32 / 16.));
    }
    for i in (0..=16).rev() {
        path.line_to(polar(center, 86., a + (b - a) * i as f32 / 16.));
    }
    path.close();
    path.build().expect("valid wheel path")
}

impl Render for Overlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let mut root = div().size_full().relative();
        if let Some(wheel) = &model.wheel
            && wheel.display == self.display
        {
            let center = wheel.origin - self.bounds.origin;
            let hot = wheel.hot;
            root = root.child(
                canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        for slot in 0..8 {
                            let color = if hot == Some(slot) {
                                rgba(0x88826cdf)
                            } else {
                                rgba(0x242b32ed)
                            };
                            window.paint_path(sector(center + bounds.origin, slot, 188.), color);
                            let mut border = PathBuilder::stroke(px(1.));
                            let angle = (-112.5 + slot as f32 * 45.).to_radians();
                            border.move_to(polar(center + bounds.origin, 86., angle));
                            border.line_to(polar(center + bounds.origin, 188., angle));
                            if let Ok(path) = border.build() {
                                window.paint_path(path, rgba(0xc8aa6e88));
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            );
            for (slot, id) in model.settings.wheel.iter().enumerate() {
                let ping = &PINGS[index(id).unwrap_or(8)];
                let pos = polar(center, 137., (-90. + slot as f32 * 45.).to_radians());
                let s = if hot == Some(slot) { 58. } else { 48. };
                root = root.child(
                    img(model.assets.join(format!("textures/{}.png", ping.icon)))
                        .absolute()
                        .left(pos.x - px(s / 2.))
                        .top(pos.y - px(s / 2.))
                        .size(px(s)),
                );
            }
            let label = hot
                .map(|slot| PINGS[index(&model.settings.wheel[slot]).unwrap_or(8)].name)
                .unwrap_or("选择信号");
            root = root.child(
                div()
                    .absolute()
                    .left(center.x - px(86.))
                    .top(center.y - px(86.))
                    .size(px(172.))
                    .rounded_full()
                    .border_2()
                    .border_color(rgb(0xc8aa6e))
                    .bg(rgba(0x123033f5))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_3()
                    .text_color(rgb(0xf0dfb9))
                    .child(img(model.assets.join("textures/generic_ping.png")).size(px(34.)))
                    .child(label)
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x9caeae))
                            .child("中心松开 / Esc 取消"),
                    ),
            );
        }
        let mut animating = false;
        for ping in model.pings.iter().filter(|p| p.display == self.display) {
            let elapsed = ping.born.elapsed().as_secs_f32();
            if elapsed >= ping.duration {
                continue;
            }
            animating = true;
            let center = ping.origin - self.bounds.origin;
            let progress = elapsed / ping.duration;
            let opacity = ((1. - progress) / 0.18).min(1.);
            let def = &PINGS[ping.ping];
            let scale = (elapsed / 0.16).min(1.);
            let s = ping.size * (0.65 + 0.35 * scale);
            let bounce = if elapsed < 0.45 {
                (elapsed / 0.45 * std::f32::consts::PI).sin() * 16.
            } else {
                0.
            };
            for phase in [0., 0.3, 0.6] {
                let t = (elapsed * 1.2 + phase).fract();
                let r = ping.size * (0.35 + t * 0.75);
                root = root.child(
                    div()
                        .absolute()
                        .left(center.x - px(r))
                        .top(center.y - px(r * 0.4))
                        .w(px(r * 2.))
                        .h(px(r * 0.8))
                        .rounded_full()
                        .border_2()
                        .border_color(rgb(def.color))
                        .opacity((1. - t) * opacity * 0.7),
                );
            }
            for (i, art) in def.art.iter().enumerate() {
                let clash = if def.art.len() == 2 {
                    (1. - scale) * 30. * if i == 0 { -1. } else { 1. }
                } else {
                    0.
                };
                root = root.child(
                    img(model.assets.join(format!("textures/{art}.png")))
                        .absolute()
                        .left(center.x - px(s / 2.) + px(clash))
                        .top(center.y - px(s + bounce))
                        .size(px(s))
                        .opacity(opacity),
                );
            }
        }
        if animating {
            window.request_animation_frame();
        }
        root
    }
}

struct SettingsView {
    model: Entity<Model>,
    selected: usize,
    scroll: ScrollHandle,
}

fn button(
    label: impl Into<SharedString>,
    active: bool,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.into();
    div()
        .id(label.clone())
        .px_3()
        .py_2()
        .rounded_md()
        .border_1()
        .border_color(rgb(if active { 0xc8aa6e } else { 0x36404c }))
        .bg(rgb(if active { 0x3c382b } else { 0x222a34 }))
        .text_color(rgb(if active { 0xf0d79e } else { 0xcad2dc }))
        .text_sm()
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x404854)))
        .child(label)
        .on_click(move |_, window, cx| action(window, cx))
}

fn section(title: &'static str) -> Div {
    div()
        .p_5()
        .bg(rgb(0x1a212a))
        .rounded_lg()
        .flex()
        .flex_col()
        .gap_3()
        .child(div().text_color(rgb(0xe8d3a4)).text_lg().child(title))
}
fn row(label: impl Into<SharedString>, control: impl IntoElement) -> Stateful<Div> {
    let label = label.into();
    div()
        .id(label.clone())
        .flex()
        .justify_between()
        .items_center()
        .gap_3()
        .child(div().text_sm().child(label))
        .child(control)
}

impl SettingsView {
    fn toggle_row(
        &self,
        label: &'static str,
        value: bool,
        mutate: fn(&mut Settings),
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        let model = self.model.clone();
        let _ = cx;
        row(
            label,
            button(
                if value { "已开启" } else { "已关闭" },
                value,
                move |_, cx| {
                    model.update(cx, |model, cx| {
                        mutate(&mut model.settings);
                        model.save(cx);
                    });
                },
            ),
        )
    }

    fn number_row(
        &self,
        label: &'static str,
        value: String,
        adjust: fn(&mut Settings, f32),
    ) -> Stateful<Div> {
        let left = self.model.clone();
        let right = self.model.clone();
        row(
            label,
            div()
                .id(label)
                .flex()
                .items_center()
                .gap_3()
                .child(button("−", false, move |_, cx| {
                    left.update(cx, |model, cx| {
                        adjust(&mut model.settings, -1.);
                        model.save(cx);
                    })
                }))
                .child(div().w(px(80.)).text_center().text_sm().child(value))
                .child(button("+", false, move |_, cx| {
                    right.update(cx, |model, cx| {
                        adjust(&mut model.settings, 1.);
                        model.save(cx);
                    })
                })),
        )
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let s = model.settings.clone();
        let enabled = model.enabled;
        let status = model.status.clone();
        let notice = model.notice.clone();
        let assets = model.assets.clone();
        let pause_model = self.model.clone();
        let restart_model = self.model.clone();
        let mut trigger_buttons = div().flex().flex_wrap().gap_2();
        for &(id, label) in TRIGGERS {
            let model = self.model.clone();
            trigger_buttons =
                trigger_buttons.child(button(label, s.trigger == id, move |_, cx| {
                    model.update(cx, |model, cx| {
                        model.settings.trigger = id.into();
                        model.save(cx);
                    });
                }));
        }
        let mut palette = div().flex().flex_wrap().gap_2();
        for (i, ping) in PINGS.iter().enumerate() {
            palette = palette.child(
                div()
                    .id(("ping", i))
                    .w(px(120.))
                    .p_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(if i == self.selected {
                        0xc8aa6e
                    } else {
                        0x36404c
                    }))
                    .bg(rgb(if i == self.selected {
                        0x3c382b
                    } else {
                        0x222a34
                    }))
                    .cursor_pointer()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(img(assets.join(format!("textures/{}.png", ping.icon))).size(px(28.)))
                    .child(div().text_xs().child(ping.name))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.selected = i;
                        view.model.update(cx, |model, cx| model.preview(i, cx));
                        cx.notify();
                    })),
            );
        }
        let mut slots = div().flex().flex_wrap().gap_2();
        for (i, id) in s.wheel.iter().enumerate() {
            let model = self.model.clone();
            let selected = self.selected;
            let name = PINGS[index(id).unwrap_or(8)].name;
            let direction = ["↑", "↗", "→", "↘", "↓", "↙", "←", "↖"][i];
            slots = slots.child(button(
                format!("{direction} {name}"),
                false,
                move |_, cx| {
                    model.update(cx, |model, cx| {
                        model.settings.place(i, selected);
                        model.save(cx);
                    });
                },
            ));
        }
        let center_model = self.model.clone();
        let reset_model = self.model.clone();
        let selected = self.selected;
        let mut contents = div().flex().flex_col().gap_4().p_6()
            .child(div().flex().justify_between().items_center()
                .child(div().flex().flex_col().gap_1().child(div().text_2xl().text_color(rgb(0xf0dfb9)).child("lolPing"))
                    .child(div().text_sm().text_color(rgb(0x98a6b7)).child("把信号带到整个桌面")))
                .child(button(if enabled { "运行中 · 点击暂停" } else { "已暂停 · 点击恢复" }, enabled,
                    move |_, cx| pause_model.update(cx, |model, cx| model.toggle(cx)))))
            .child(section("输入与权限").child(div().text_sm().child(status))
                .child(div().flex().gap_2()
                    .child(button("辅助功能权限", false, |_, cx| cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")))
                    .child(button("重新连接", false, move |_, cx| restart_model.update(cx, |model, cx| model.restart_helper(cx)))))
                .child(div().text_xs().text_color(rgb(0x98a6b7)).child("按住触发键并拖动鼠标，松开发信号。⌃⌥P 暂停 / 恢复。"))
                .child(trigger_buttons)
                .child(self.toggle_row("触发键 + 单击发送信号", s.click_ping, |s| s.click_ping = !s.click_ping, cx))
                .child(self.toggle_row("启动时启用信号", s.enabled_on_start, |s| s.enabled_on_start = !s.enabled_on_start, cx))
                .child(self.number_row("拖动阈值", format!("{} pt", s.drag_threshold_px), |s, n|
                    s.drag_threshold_px = (s.drag_threshold_px as i32 + n as i32).clamp(2, 40) as u32)))
            .child(section("信号与声音")
                .child(self.number_row("信号大小", format!("{:.0} pt", s.ping_size_px), |s, n| s.ping_size_px = (s.ping_size_px + n * 10.).clamp(60., 220.)))
                .child(self.number_row("显示时长", format!("{:.1} 秒", s.ping_duration_s), |s, n| s.ping_duration_s = ((s.ping_duration_s + n * 0.2) * 10.).round().clamp(10., 80.) / 10.))
                .child(self.number_row("音量", format!("{}%", s.volume), |s, n| s.volume = (s.volume as i32 + n as i32 * 5).clamp(0, 100) as u32))
                .child(self.toggle_row("静音", s.muted, |s| s.muted = !s.muted, cx))
                .child(self.toggle_row("轮盘提示音", s.tick_sound, |s| s.tick_sound = !s.tick_sound, cx)))
            .child(section("轮盘布局")
                .child(div().text_sm().text_color(rgb(0x98a6b7)).child("先点信号试听并选中，再点下方方向来放置。已有信号会交换位置。"))
                .child(palette).child(slots)
                .child(div().flex().gap_2()
                    .child(button(format!("单击信号：{} · 点击替换", PINGS[index(&s.click_ping_id).unwrap_or(8)].name), false,
                        move |_, cx| center_model.update(cx, |model, cx| { model.settings.click_ping_id = PINGS[selected].id.into(); model.save(cx); })))
                    .child(button("恢复默认布局", false, move |_, cx| reset_model.update(cx, |model, cx| {
                        model.settings.wheel = Settings::default().wheel;
                        model.settings.click_ping_id = "generic".into(); model.save(cx);
                    })))));
        if !notice.is_empty() {
            contents = contents.child(div().text_color(rgb(0xf0bb7d)).child(notice));
        }
        contents = contents.child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x98a6b7))
                        .child("macOS 原生预览版 · 0.1.0"),
                )
                .child(button("退出 lolPing", false, |_, cx| cx.quit())),
        );
        div()
            .id("settings-scroll")
            .track_scroll(&self.scroll)
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(0x111820))
            .text_color(rgb(0xd4dce6))
            .font_family(".AppleSystemUIFont")
            .child(contents)
    }
}

pub fn run() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let executable = std::env::current_exe()?;
    let bundle = executable.parent().unwrap().join("../Resources");
    let bundled = bundle.join("assets").is_dir();
    let assets = if bundled {
        bundle.join("assets")
    } else {
        root.join("assets")
    };
    let helper_path = if bundled {
        executable.parent().unwrap().join("hook-helper")
    } else {
        root.join("native/hook-helper/build/hook-helper")
    };
    for asset in asset_files() {
        anyhow::ensure!(assets.join(&asset).is_file(), "缺少素材：{asset}");
    }
    let home = std::env::var_os("HOME").context("HOME 未设置")?;
    let data = PathBuf::from(home).join("Library/Application Support");
    let smoke = std::env::args().any(|a| a == "--smoke-test");
    let simulate = smoke || std::env::args().any(|a| a == "--preview");
    let settings_path = if simulate {
        std::env::temp_dir().join(format!(
            "lolping-preview-{}/settings.json",
            std::process::id()
        ))
    } else {
        data.join("lolPing-native/settings.json")
    };
    let (settings, notice) = if simulate {
        (Settings::default(), String::new())
    } else {
        match Settings::load(&settings_path, &data.join("lolPing/settings.json")) {
            Ok(s) => (s, String::new()),
            Err(e) => (Settings::default(), format!("使用默认设置：{e}")),
        }
    };
    fs::create_dir_all(settings_path.parent().unwrap())?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(settings_path.with_extension("lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock)
        .context("lolPing Native 已经在运行，请从菜单栏打开设置")?;
    let enabled = settings.enabled_on_start;
    let app = Application::new();
    app.on_reopen(|cx| cx.dispatch_action(&OpenSettings));
    app.run(move |cx| {
        let model = cx.new(|cx| {
            cx.on_app_quit(|model: &mut Model, _| {
                model.helper = None;
                async {}
            })
            .detach();
            Model {
                settings,
                enabled,
                status: String::new(),
                notice,
                settings_path,
                assets,
                helper_path,
                helper: None,
                generation: 0,
                simulate,
                wheel: None,
                pings: vec![],
                overlays: vec![],
                settings_window: None,
            }
        });
        macos::init_status();
        macos::set_enabled(enabled);
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-,", OpenSettings, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());
        let settings_model = model.clone();
        cx.on_action(move |_: &OpenSettings, cx| {
            settings_model.update(cx, |m, cx| m.show_settings(cx))
        });
        let toggle_model = model.clone();
        cx.on_action(move |_: &Toggle, cx| toggle_model.update(cx, |m, cx| m.toggle(cx)));
        cx.set_menus(vec![Menu {
            name: "lolPing".into(),
            items: vec![
                MenuItem::action("设置…", OpenSettings),
                MenuItem::action("暂停 / 恢复", Toggle),
                MenuItem::separator(),
                MenuItem::action("退出 lolPing", Quit),
            ],
        }]);
        model.update(cx, |m, cx| {
            m.sync_displays(cx);
            m.show_settings(cx);
            m.restart_helper(cx);
        });
        let ticker = model.clone();
        cx.spawn(async move |cx| {
            let _lock = lock;
            let mut ticks = 0;
            loop {
                Timer::after(Duration::from_millis(200)).await;
                if cx
                    .update(|cx| {
                        match macos::poll_action() {
                            1 => ticker.update(cx, |m, cx| m.show_settings(cx)),
                            2 => ticker.update(cx, |m, cx| m.toggle(cx)),
                            3 => cx.quit(),
                            _ => {}
                        }
                        ticks += 1;
                        if ticks % 10 == 0 {
                            ticker.update(cx, |m, cx| m.sync_displays(cx));
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        if simulate {
            cx.spawn(async move |cx| {
                Timer::after(Duration::from_secs(1)).await;
                let _ = model.update(cx, |m, cx| {
                    if let Some(d) = cx.primary_display() {
                        let c = macos::display_bounds(d.id()).center();
                        let x = f32::from(c.x) as i32;
                        let y = f32::from(c.y) as i32;
                        if let Some(helper) = &mut m.helper {
                            for command in [
                                json!({"type":"sim","ev":"kdown","vk":164}),
                                json!({"type":"sim","ev":"mdown","btn":"left","x":x,"y":y}),
                                json!({"type":"sim","ev":"move","x":x + 160,"y":y}),
                            ] {
                                let _ = helper.send(&command);
                            }
                        }
                    }
                });
                if smoke {
                    Timer::after(Duration::from_secs(2)).await;
                    let _ = model.update(cx, |m, _| {
                        if let Some(wheel) = &m.wheel {
                            let x = f32::from(wheel.origin.x) as i32 + 160;
                            let y = f32::from(wheel.origin.y) as i32;
                            if let Some(helper) = &mut m.helper {
                                let _ = helper.send(
                                    &json!({"type":"sim","ev":"mup","btn":"left","x":x,"y":y}),
                                );
                            }
                        }
                    });
                    Timer::after(Duration::from_secs(1)).await;
                    let _ = model.update(cx, |m, cx| {
                        assert_eq!(m.pings.len(), 1, "simulated drag must create a ping");
                        assert_eq!(m.pings[0].ping, 2, "east selects On My Way");
                        assert!(m.wheel.is_none(), "release must close the wheel");
                        assert!(m.notice.is_empty(), "sound and assets must load: {}", m.notice);
                        m.settings.volume = 35;
                        m.save(cx);
                        assert_eq!(Settings::load(&m.settings_path, &m.settings_path).unwrap().volume, 35);
                    });
                    let _ = cx.update(|cx| {
                        let overlays: Vec<_> = model.read(cx).overlays.iter().map(|(_, _, w)| *w).collect();
                        for overlay in overlays {
                            overlay.update(cx, |_, window, _| assert!(macos::overlay_valid(window),
                                "overlay must be transparent, click-through, nonactivating and on all Spaces")).unwrap();
                        }
                        let settings = model.read(cx).settings_window.unwrap();
                        settings.update(cx, |view, _, cx| {
                            view.scroll.set_offset(point(px(0.), px(-900.)));
                            cx.notify();
                        }).unwrap();
                    });
                    Timer::after(Duration::from_secs(2)).await;
                    let _ = cx.update(|cx| {
                        let settings = model.read(cx).settings_window.unwrap();
                        settings.update(cx, |view, _, _| assert!(view.scroll.offset().y < px(-100.),
                            "settings must scroll to the wheel editor")).unwrap();
                        eprintln!("Smoke test passed: helper → wheel → ping, native overlay flags, sound, settings persistence and scrolling");
                    });
                    Timer::after(Duration::from_secs(2)).await;
                    let _ = cx.update(|cx| cx.quit());
                }
            })
            .detach();
        }
    });
    Ok(())
}
