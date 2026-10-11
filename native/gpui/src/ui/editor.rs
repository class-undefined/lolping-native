use super::*;
use lolping_native::{pick_editor_slot, settings::TRIGGERS};
use std::{cell::Cell, rc::Rc};

mod smoke;
pub(super) use smoke::run as smoke_test;

const BG: u32 = 0x10161e;
const PANEL: u32 = 0x18212c;
const EDGE: u32 = 0x303e4e;
const TEXT: u32 = 0xdce6ee;
const MUTED: u32 = 0x91a2b4;
const ACCENT: u32 = 0x6bc9f3;

#[derive(Clone)]
struct DragSignal {
    ping: usize,
    source: Option<usize>,
    assets: PathBuf,
    offset: Point<Pixels>,
}

impl Render for DragSignal {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let ping = &PINGS[self.ping];
        div()
            .pl(self.offset.x + px(12.))
            .pt(self.offset.y + px(12.))
            .child(
                div()
                    .w(px(150.))
                    .h(px(44.))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .bg(rgb(0x263b4b))
                    .border_1()
                    .border_color(rgb(ACCENT))
                    .shadow_lg()
                    .text_size(px(12.))
                    .text_color(rgb(TEXT))
                    .child(
                        img(self.assets.join(format!("textures/{}.png", ping.icon))).size(px(28.)),
                    )
                    .child(ping.name),
            )
    }
}

#[derive(Clone, PartialEq)]
struct Layout {
    wheel: [String; 8],
    click: String,
}

impl Layout {
    fn read(settings: &Settings) -> Self {
        Self {
            wheel: settings.wheel.clone(),
            click: settings.click_ping_id.clone(),
        }
    }
}

pub(super) struct SettingsView {
    model: Entity<Model>,
    selected: Option<usize>,
    active_slot: usize,
    hot: Option<usize>,
    pressed_slot: Option<usize>,
    highlight: wheel::Highlight,
    transition: Option<(Layout, Instant)>,
    undo: Vec<Layout>,
    wheel_bounds: Rc<Cell<Bounds<Pixels>>>,
    palette_bounds: Rc<Cell<Bounds<Pixels>>>,
    click_bounds: Rc<Cell<Bounds<Pixels>>>,
    scroll: ScrollHandle,
    focus: FocusHandle,
}

fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    active: bool,
    action: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.))
        .px(px(10.))
        .flex()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(rgb(if active { 0x52849e } else { EDGE }))
        .bg(rgb(if active { 0x243e50 } else { 0x202c39 }))
        .text_color(rgb(if active { 0xc0eaff } else { TEXT }))
        .text_size(px(12.))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0x304458)).border_color(rgb(0x5b829d)))
        .active(|s| s.bg(rgb(0x172633)))
        .child(label.into())
        .on_click(move |_, window, cx| action(window, cx))
}

fn heading(title: &'static str) -> Div {
    div()
        .h(px(38.))
        .px_3()
        .flex()
        .items_center()
        .border_b_1()
        .border_color(rgb(EDGE))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(title)
}

fn panel() -> Div {
    div()
        .flex()
        .flex_col()
        .bg(rgb(PANEL))
        .border_1()
        .border_color(rgb(EDGE))
}

fn row(label: &'static str, control: impl IntoElement) -> Stateful<Div> {
    div()
        .id(label)
        .h(px(38.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .border_b_1()
        .border_color(rgb(0x273341))
        .text_size(px(12.))
        .child(label)
        .child(control)
}

impl SettingsView {
    pub(super) fn new(model: Entity<Model>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window);
        Self {
            model,
            selected: None,
            active_slot: 0,
            hot: None,
            pressed_slot: None,
            highlight: wheel::Highlight::default(),
            transition: None,
            undo: vec![],
            wheel_bounds: Rc::new(Cell::new(Bounds::default())),
            palette_bounds: Rc::new(Cell::new(Bounds::default())),
            click_bounds: Rc::new(Cell::new(Bounds::default())),
            scroll: ScrollHandle::new(),
            focus,
        }
    }

    fn slot_at(&self, position: Point<Pixels>) -> Option<usize> {
        let delta = position - self.wheel_bounds.get().center();
        pick_editor_slot(delta.x.into(), delta.y.into())
    }

    fn finish_edit(&mut self, before: Layout, cx: &mut Context<Self>) {
        self.selected = None;
        self.pressed_slot = None;
        cx.notify();
        if before == Layout::read(&self.model.read(cx).settings) {
            return;
        }
        self.undo.push(before.clone());
        if self.undo.len() > 32 {
            self.undo.remove(0);
        }
        self.transition = Some((before, Instant::now()));
        self.model.update(cx, |model, cx| model.save(cx));
        cx.notify();
    }

    fn drop_signal(&mut self, drag: &DragSignal, slot: usize, cx: &mut Context<Self>) {
        let before = Layout::read(&self.model.read(cx).settings);
        self.model.update(cx, |model, _| {
            if let Some(from) = drag.source {
                model.settings.move_slot(from, slot);
            } else {
                model.settings.place(slot, drag.ping);
            }
        });
        self.active_slot = slot;
        self.pressed_slot = None;
        self.finish_edit(before, cx);
    }

    fn undo_layout(&mut self, cx: &mut Context<Self>) {
        if let Some(previous) = self.undo.pop() {
            let before = Layout::read(&self.model.read(cx).settings);
            self.model.update(cx, |model, cx| {
                model.settings.wheel = previous.wheel;
                model.settings.click_ping_id = previous.click;
                model.save(cx);
            });
            self.transition = Some((before, Instant::now()));
            self.selected = None;
            cx.notify();
        }
    }

    fn toggle_row(
        &self,
        label: &'static str,
        value: bool,
        mutate: fn(&mut Settings),
    ) -> Stateful<Div> {
        let model = self.model.clone();
        row(
            label,
            div()
                .id((ElementId::Name("toggle".into()), label))
                .w(px(58.))
                .h(px(24.))
                .px(px(5.))
                .flex()
                .items_center()
                .justify_between()
                .cursor_pointer()
                .bg(rgb(if value { 0x253f50 } else { 0x202a35 }))
                .border_1()
                .border_color(rgb(if value { 0x4b8dac } else { EDGE }))
                .hover(|s| s.border_color(rgb(ACCENT)))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(if value { 0xb2e4fb } else { MUTED }))
                        .child(if value { "开启" } else { "关闭" }),
                )
                .child(
                    div()
                        .size(px(8.))
                        .bg(rgb(if value { ACCENT } else { 0x546475 })),
                )
                .on_click(move |_, _, cx| {
                    model.update(cx, |model, cx| {
                        mutate(&mut model.settings);
                        model.save(cx);
                    })
                }),
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
                .flex()
                .items_center()
                .child(
                    button(
                        (ElementId::Name("minus".into()), label),
                        "−",
                        false,
                        move |_, cx| {
                            left.update(cx, |model, cx| {
                                adjust(&mut model.settings, -1.);
                                model.save(cx);
                            })
                        },
                    )
                    .w(px(26.))
                    .h(px(24.))
                    .px_0(),
                )
                .child(
                    div()
                        .w(px(62.))
                        .text_center()
                        .text_size(px(12.))
                        .child(value),
                )
                .child(
                    button(
                        (ElementId::Name("plus".into()), label),
                        "+",
                        false,
                        move |_, cx| {
                            right.update(cx, |model, cx| {
                                adjust(&mut model.settings, 1.);
                                model.save(cx);
                            })
                        },
                    )
                    .w(px(26.))
                    .h(px(24.))
                    .px_0(),
                ),
        )
    }

    fn palette(&self, settings: &Settings, assets: &std::path::Path, cx: &Context<Self>) -> Div {
        let bounds = self.palette_bounds.clone();
        let mut list = panel()
            .relative()
            .child(
                canvas(move |rect, _, _| bounds.set(rect), |_, (), _, _| {})
                    .absolute()
                    .size_full(),
            )
            .w(px(174.))
            .flex_shrink_0()
            .child(heading("信号库"));
        for (ping_index, ping) in PINGS.iter().enumerate() {
            let slot = settings.wheel.iter().position(|id| id == ping.id);
            let selected = self.selected == Some(ping_index);
            let drag = DragSignal {
                ping: ping_index,
                source: None,
                assets: assets.to_path_buf(),
                offset: Point::default(),
            };
            list = list.child(
                div()
                    .id(("signal", ping_index))
                    .h(px(43.))
                    .px_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .border_b_1()
                    .border_color(rgb(0x293542))
                    .border_l_2()
                    .border_color(rgb(if selected { ACCENT } else { 0x293542 }))
                    .bg(rgb(if selected { 0x263f50 } else { PANEL }))
                    .cursor_grab()
                    .hover(|s| s.bg(rgb(0x263544)))
                    .child(
                        img(assets.join(format!("textures/{}.png", ping.icon)))
                            .size(px(27.))
                            .flex_shrink_0(),
                    )
                    .child(div().flex_1().text_size(px(12.)).child(ping.name))
                    .child(
                        div().text_size(px(11.)).text_color(rgb(MUTED)).child(
                            slot.map(|i| format!("{:02}", i + 1))
                                .unwrap_or_else(|| "+".into()),
                        ),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.selected = if view.selected == Some(ping_index) {
                            None
                        } else {
                            Some(ping_index)
                        };
                        view.model
                            .update(cx, |model, _| model.sound(PINGS[ping_index].sound));
                        cx.notify();
                    }))
                    .on_drag(drag, |drag, offset, _, cx| {
                        cx.new(|_| DragSignal {
                            offset,
                            ..drag.clone()
                        })
                    }),
            );
        }
        list
    }

    fn wheel_editor(
        &mut self,
        settings: &Settings,
        assets: &std::path::Path,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let dragging = cx.has_active_drag();
        let active = if dragging {
            self.hot
        } else {
            self.hot.or(Some(self.active_slot))
        };
        let highlights = self.highlight.frame(active, window);
        let slot = active.unwrap_or(self.active_slot);
        let ping_index = self
            .selected
            .unwrap_or_else(|| index(&settings.wheel[slot]).unwrap_or(8));
        let label = PINGS[ping_index].name;
        let sublabel = if self.selected.is_some() {
            "待放置".to_owned()
        } else {
            format!("{:02} / {}", slot + 1, wheel::DIRECTIONS[slot])
        };
        let progress = self
            .transition
            .as_ref()
            .map(|(_, start)| start.elapsed().as_secs_f32() / 0.24)
            .unwrap_or(1.);
        if progress >= 1. {
            self.transition = None;
        } else {
            window.request_animation_frame();
        }
        let previous = self.transition.as_ref().map(|(layout, _)| &layout.wheel);
        let bounds = self.wheel_bounds.clone();
        let source = self.pressed_slot.or(self.hot);
        let mut surface = div()
            .id("wheel-editor")
            .relative()
            .size(px(wheel::SURFACE_SIZE))
            .flex_shrink_0()
            .cursor_grab()
            .child(
                canvas(move |rect, _, _| bounds.set(rect), |_, (), _, _| {})
                    .absolute()
                    .size_full(),
            )
            .child(wheel::surface(
                assets,
                &settings.wheel,
                highlights,
                previous,
                wheel::settle(progress),
                label,
                &sublabel,
            ))
            .on_mouse_move(cx.listener(|view, event: &MouseMoveEvent, _, cx| {
                if event.pressed_button.is_none() {
                    let hot = view.slot_at(event.position);
                    if hot != view.hot {
                        view.hot = hot;
                        cx.notify();
                    }
                }
            }))
            .on_hover(cx.listener(|view, hovered, _, cx| {
                if !hovered {
                    view.hot = None;
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, event: &MouseDownEvent, _, cx| {
                    view.pressed_slot = view.slot_at(event.position);
                    cx.notify();
                }),
            )
            .on_drag_move(
                cx.listener(|view, event: &DragMoveEvent<DragSignal>, _, cx| {
                    let hot = view.slot_at(event.event.position);
                    if hot != view.hot {
                        view.hot = hot;
                        cx.notify();
                    }
                }),
            )
            .on_drop(cx.listener(|view, drag: &DragSignal, window, cx| {
                if let Some(slot) = view.slot_at(window.mouse_position()) {
                    view.drop_signal(drag, slot, cx);
                }
                view.pressed_slot = None;
                cx.notify();
            }))
            .on_click(cx.listener(|view, _, window, cx| {
                if let Some(slot) = view.slot_at(window.mouse_position()) {
                    view.active_slot = slot;
                    if let Some(ping) = view.selected {
                        let before = Layout::read(&view.model.read(cx).settings);
                        view.model
                            .update(cx, |model, _| model.settings.place(slot, ping));
                        view.finish_edit(before, cx);
                    }
                }
                view.pressed_slot = None;
                cx.notify();
            }));
        if let Some(slot) = source {
            let drag = DragSignal {
                ping: index(&settings.wheel[slot]).unwrap_or(8),
                source: Some(slot),
                assets: assets.to_path_buf(),
                offset: Point::default(),
            };
            surface = surface.on_drag(drag, |drag, offset, _, cx| {
                cx.new(|_| DragSignal {
                    offset,
                    ..drag.clone()
                })
            });
        }
        let toolbar = div()
            .h(px(38.))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(EDGE))
            .child(
                div()
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("轮盘布局"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        button("undo-layout", "撤销", false, {
                            let view = cx.entity();
                            move |_, cx| view.update(cx, |view, cx| view.undo_layout(cx))
                        })
                        .h(px(24.))
                        .when(self.undo.is_empty(), |s| s.opacity(0.35).cursor_default()),
                    )
                    .child(
                        button("reset-layout", "恢复默认", false, {
                            let view = cx.entity();
                            move |_, cx| {
                                view.update(cx, |view, cx| {
                                    let before = Layout::read(&view.model.read(cx).settings);
                                    view.model.update(cx, |model, _| {
                                        model.settings.wheel = Settings::default().wheel;
                                        model.settings.click_ping_id = "generic".into();
                                    });
                                    view.finish_edit(before, cx);
                                })
                            }
                        })
                        .h(px(24.)),
                    ),
            );
        let click_ping = &PINGS[index(&settings.click_ping_id).unwrap_or(8)];
        let click_bounds = self.click_bounds.clone();
        let center = div()
            .id("click-signal")
            .relative()
            .child(
                canvas(move |rect, _, _| click_bounds.set(rect), |_, (), _, _| {})
                    .absolute()
                    .size_full(),
            )
            .h(px(42.))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .border_1()
            .border_color(rgb(EDGE))
            .bg(rgb(0x15202b))
            .cursor_pointer()
            .hover(|s| s.border_color(rgb(0x5685a1)))
            .drag_over::<DragSignal>(|s, _, _, _| s.border_color(rgb(ACCENT)).bg(rgb(0x284556)))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child("单击信号"),
            )
            .child(img(assets.join(format!("textures/{}.png", click_ping.icon))).size(px(26.)))
            .child(div().flex_1().text_size(px(12.)).child(click_ping.name))
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(rgb(if settings.click_ping { ACCENT } else { MUTED }))
                    .child(if settings.click_ping {
                        "已启用"
                    } else {
                        "未启用"
                    }),
            )
            .on_drop(cx.listener(|view, drag: &DragSignal, _, cx| {
                let before = Layout::read(&view.model.read(cx).settings);
                view.model.update(cx, |model, _| {
                    model.settings.click_ping_id = PINGS[drag.ping].id.into()
                });
                view.finish_edit(before, cx);
            }))
            .on_click(cx.listener(|view, _, _, cx| {
                if let Some(ping) = view.selected {
                    let before = Layout::read(&view.model.read(cx).settings);
                    view.model.update(cx, |model, _| {
                        model.settings.click_ping_id = PINGS[ping].id.into()
                    });
                    view.finish_edit(before, cx);
                }
            }));
        panel()
            .flex_1()
            .min_w(px(428.))
            .child(toolbar)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(442.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(linear_gradient(
                        180.,
                        linear_color_stop(rgb(0x202c39), 0.),
                        linear_color_stop(rgb(0x111923), 1.),
                    ))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .m_3()
                            .border_1()
                            .border_color(rgb(0x2a3745)),
                    )
                    .child(surface),
            )
            .child(
                div()
                    .p_3()
                    .border_t_1()
                    .border_color(rgb(EDGE))
                    .child(center),
            )
    }

    fn controls(&self, settings: &Settings) -> Div {
        let mut triggers = div().flex().flex_wrap().gap(px(4.)).py_2();
        for &(id, label) in TRIGGERS {
            let model = self.model.clone();
            triggers = triggers.child(
                button(
                    (ElementId::Name("trigger".into()), id),
                    label,
                    settings.trigger == id,
                    move |_, cx| {
                        model.update(cx, |model, cx| {
                            model.settings.trigger = id.into();
                            model.save(cx);
                        });
                    },
                )
                .w(px(124.))
                .px_0(),
            );
        }
        let preview = self
            .selected
            .unwrap_or_else(|| index(&settings.wheel[self.active_slot]).unwrap_or(8));
        let model = self.model.clone();
        div()
            .w(px(280.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                panel().child(heading("触发设置")).child(
                    div()
                        .px_3()
                        .pb_2()
                        .child(triggers)
                        .child(self.toggle_row("触发键 + 单击", settings.click_ping, |s| {
                            s.click_ping = !s.click_ping
                        }))
                        .child(
                            self.toggle_row("启动时启用", settings.enabled_on_start, |s| {
                                s.enabled_on_start = !s.enabled_on_start
                            }),
                        )
                        .child(self.number_row(
                            "拖动阈值",
                            format!("{} pt", settings.drag_threshold_px),
                            |s, n| {
                                s.drag_threshold_px =
                                    (s.drag_threshold_px as i32 + n as i32).clamp(2, 40) as u32
                            },
                        )),
                ),
            )
            .child(
                panel().child(heading("信号与声音")).child(
                    div()
                        .px_3()
                        .pb_2()
                        .child(self.number_row(
                            "信号大小",
                            format!("{:.0} pt", settings.ping_size_px),
                            |s, n| s.ping_size_px = (s.ping_size_px + n * 10.).clamp(60., 220.),
                        ))
                        .child(self.number_row(
                            "显示时长",
                            format!("{:.1} 秒", settings.ping_duration_s),
                            |s, n| {
                                s.ping_duration_s = ((s.ping_duration_s + n * 0.2) * 10.)
                                    .round()
                                    .clamp(10., 80.)
                                    / 10.
                            },
                        ))
                        .child(self.number_row(
                            "音量",
                            format!("{}%", settings.volume),
                            |s, n| s.volume = (s.volume as i32 + n as i32 * 5).clamp(0, 100) as u32,
                        ))
                        .child(self.toggle_row("静音", settings.muted, |s| s.muted = !s.muted))
                        .child(self.toggle_row("轮盘提示音", settings.tick_sound, |s| {
                            s.tick_sound = !s.tick_sound
                        }))
                        .child(
                            button(
                                "preview-signal",
                                format!("预览 · {}", PINGS[preview].name),
                                false,
                                move |_, cx| {
                                    model.update(cx, |model, cx| model.preview(preview, cx))
                                },
                            )
                            .w_full()
                            .mt_2(),
                        ),
                ),
            )
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = self.model.read(cx);
        let settings = model.settings.clone();
        let enabled = model.enabled;
        let assets = model.assets.clone();
        let status = model.status.clone();
        let notice = model.notice.clone();
        let simulate = model.simulate;
        let preview_ready = simulate && status.starts_with("预览模式");
        let connected = status.contains("已连接") || preview_ready;
        let pause_model = self.model.clone();
        let restart_model = self.model.clone();
        let palette = self.palette(&settings, &assets, cx);
        let wheel = self.wheel_editor(&settings, &assets, window, cx);
        let controls = self.controls(&settings);
        div().id("settings-root").track_focus(&self.focus).size_full().flex().flex_col()
            .bg(rgb(BG)).text_color(rgb(TEXT)).font_family(".AppleSystemUIFont")
            .on_mouse_down(MouseButton::Left, cx.listener(|view, _, window, _| view.focus.focus(window)))
            .on_mouse_up(MouseButton::Left, cx.listener(|view, _, _, cx| { view.pressed_slot = None; cx.notify(); }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|view, _, _, cx| { view.pressed_slot = None; cx.notify(); }))
            .on_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_active_drag(window);
                    view.selected = None; view.pressed_slot = None; view.hot = None; cx.notify(); cx.stop_propagation();
                } else if event.keystroke.key == "z" && event.keystroke.modifiers.platform && !cx.has_active_drag() {
                    view.undo_layout(cx);
                    cx.stop_propagation();
                }
            }))
            .child(div().h(px(54.)).flex_shrink_0().px_4().flex().items_center().justify_between()
                .border_b_1().border_color(rgb(EDGE)).bg(rgb(0x18212b))
                .child(div().flex().items_center().gap_3()
                    .child(img(assets.join("textures/generic_ping.png")).size(px(27.)))
                    .child(div().text_size(px(20.)).font_weight(FontWeight::BOLD).child("lolPing"))
                    .child(div().h(px(18.)).w(px(1.)).bg(rgb(0x405064)))
                    .child(div().text_size(px(13.)).text_color(rgb(MUTED)).child("信号控制台")))
                .child(button("toggle-enabled", if enabled { "● 运行中" } else { "Ⅱ 已暂停" }, enabled,
                    move |_, cx| pause_model.update(cx, |model, cx| model.toggle(cx))).w(px(98.))))
            .child(div().id("settings-scroll").track_scroll(&self.scroll).flex_1().min_h_0().overflow_y_scroll()
                .child(div().p_3().flex().gap_3().child(palette).child(wheel).child(controls)))
            .when(!notice.is_empty(), |root| root.child(div().px_4().py_2().text_size(px(12.)).text_color(rgb(0xf0bb7d)).child(notice)))
            .child(div().h(px(40.)).flex_shrink_0().px_3().flex().items_center().justify_between()
                .border_t_1().border_color(rgb(EDGE)).bg(rgb(0x151e28))
                .child(div().flex().items_center().gap_2().flex_1().min_w_0()
                    .child(div().size(px(6.)).bg(rgb(if connected { 0x72cfa7 } else { 0xe1ad68 })))
                    .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(if preview_ready { "预览模式".to_owned() } else { status })))
                .child(div().flex().items_center().gap_2()
                    .child(button("permissions", "辅助功能权限", false, |_, cx| cx.open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")).h(px(24.)))
                    .child(button("reconnect", "重新连接", false, move |_, cx| restart_model.update(cx, |model, cx| model.restart_helper(cx))).h(px(24.)))
                    .child(button("quit", "退出", false, |_, cx| cx.quit()).h(px(24.)))))
    }
}
