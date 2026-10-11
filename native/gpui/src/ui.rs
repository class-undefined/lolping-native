use crate::macos;
use anyhow::{Context as _, Result};
use gpui::{prelude::*, *};
use lolping_native::{
    catalog::{PINGS, asset_files, index},
    helper::{Event, Helper},
    pick_slice,
    settings::Settings,
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

mod editor;
mod wheel;
use editor::SettingsView;

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
                        highlight: wheel::Highlight::default(),
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
    let bounds = Bounds::centered(None, size(px(1080.), px(680.)), cx);
    let view_model = model.clone();
    match cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(1000.), px(600.))),
            titlebar: Some(TitlebarOptions {
                title: Some("lolPing · 设置".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            macos::configure_settings(window);
            window.on_window_should_close(cx, |window, _| {
                macos::hide(window);
                false
            });
            cx.new(|cx| {
                cx.observe(&view_model, |_, _, cx| cx.notify()).detach();
                SettingsView::new(view_model, window, cx)
            })
        },
    ) {
        Ok(window) => model.update(cx, |m, _| m.settings_window = Some(window)),
        Err(error) => eprintln!("设置窗口创建失败：{error}"),
    }
    cx.activate(true);
}

struct Overlay {
    highlight: wheel::Highlight,
    model: Entity<Model>,
    display: DisplayId,
    bounds: Bounds<Pixels>,
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
            let highlights = self.highlight.frame(hot, window);
            let label = hot
                .map(|slot| PINGS[index(&model.settings.wheel[slot]).unwrap_or(8)].name)
                .unwrap_or("选择信号");
            root = root.child(
                wheel::surface(
                    &model.assets,
                    &model.settings.wheel,
                    highlights,
                    None,
                    1.,
                    label,
                )
                .absolute()
                .left(center.x - px(wheel::SURFACE_SIZE / 2.))
                .top(center.y - px(wheel::SURFACE_SIZE / 2.))
                .with_animation(
                    "wheel-open",
                    Animation::new(Duration::from_millis(160)),
                    |element, t| element.opacity(wheel::settle(t)),
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
                    });
                    let settings = cx.update(|cx| model.read(cx).settings_window.unwrap()).unwrap();
                    editor::smoke_test(settings, cx).await;
                    eprintln!("Smoke test passed: helper → wheel → ping, native overlay flags, sound, settings persistence and editor gestures");
                    Timer::after(Duration::from_secs(2)).await;
                    let _ = cx.update(|cx| cx.quit());
                }
            })
            .detach();
        }
    });
    Ok(())
}
