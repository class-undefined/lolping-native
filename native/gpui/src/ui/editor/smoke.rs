use super::*;

async fn send(handle: WindowHandle<SettingsView>, event: PlatformInput, cx: &mut AsyncApp) {
    let (kind, position) = match event {
        PlatformInput::MouseMove(event) => (
            if event.pressed_button.is_some() { 2 } else { 0 },
            event.position,
        ),
        PlatformInput::MouseDown(event) => (1, event.position),
        PlatformInput::MouseUp(event) => (3, event.position),
        PlatformInput::KeyDown(event) => (
            if event.keystroke.key == "escape" {
                4
            } else {
                5
            },
            Point::default(),
        ),
        _ => unreachable!("unsupported smoke input"),
    };
    AnyWindowHandle::from(handle)
        .update(cx, |_, window, _| {
            macos::simulate_settings_input(window, kind, position.x.into(), position.y.into());
        })
        .unwrap();
    Timer::after(Duration::from_millis(45)).await;
}

fn motion(position: Point<Pixels>, dragging: bool) -> PlatformInput {
    PlatformInput::MouseMove(MouseMoveEvent {
        position,
        pressed_button: dragging.then_some(MouseButton::Left),
        ..Default::default()
    })
}

async fn start_drag(
    handle: WindowHandle<SettingsView>,
    from: Point<Pixels>,
    to: Point<Pixels>,
    cx: &mut AsyncApp,
) {
    send(handle, motion(from, false), cx).await;
    send(
        handle,
        PlatformInput::MouseDown(MouseDownEvent {
            position: from,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    )
    .await;
    send(handle, motion(from + point(px(12.), px(0.)), true), cx).await;
    send(handle, motion(to, true), cx).await;
    cx.update(|cx| {
        assert!(
            cx.has_active_drag(),
            "pointer gesture must start a GPUI drag"
        )
    })
    .unwrap();
}

async fn release(handle: WindowHandle<SettingsView>, position: Point<Pixels>, cx: &mut AsyncApp) {
    send(
        handle,
        PlatformInput::MouseUp(MouseUpEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    )
    .await;
    cx.update(|cx| assert!(!cx.has_active_drag(), "release must end the drag"))
        .unwrap();
}

async fn drag(
    handle: WindowHandle<SettingsView>,
    from: Point<Pixels>,
    to: Point<Pixels>,
    cx: &mut AsyncApp,
) {
    start_drag(handle, from, to, cx).await;
    release(handle, to, cx).await;
    Timer::after(Duration::from_millis(260)).await;
}

async fn click(handle: WindowHandle<SettingsView>, position: Point<Pixels>, cx: &mut AsyncApp) {
    send(handle, motion(position, false), cx).await;
    send(
        handle,
        PlatformInput::MouseDown(MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }),
        cx,
    )
    .await;
    release(handle, position, cx).await;
}

fn layout(handle: WindowHandle<SettingsView>, cx: &mut AsyncApp) -> Layout {
    handle
        .update(cx, |view, _, cx| {
            Layout::read(&view.model.read(cx).settings)
        })
        .unwrap()
}

/// Exercises real GPUI event dispatch and rendered hitboxes using temporary preview settings.
pub(in crate::ui) async fn run(handle: WindowHandle<SettingsView>, cx: &mut AsyncApp) {
    let (center, library, click_target) = handle
        .update(cx, |view, window, _| {
            window.activate_window();
            (
                view.wheel_bounds.get().center(),
                view.palette_bounds.get(),
                view.click_bounds.get().center(),
            )
        })
        .unwrap();
    let signal = |ping: usize| {
        point(
            library.center().x,
            library.top() + px(38. + 43. * ping as f32 + 22.),
        )
    };
    let slot = |slot: usize| wheel::polar(center, 137., (-90. + slot as f32 * 45.).to_radians());

    // Replace an absent signal, then insert/reorder it across multiple slots.
    drag(handle, signal(9), slot(0), cx).await;
    assert_eq!(
        layout(handle, cx).wheel[0],
        "bait",
        "library drop replaces the target slot"
    );
    drag(handle, slot(0), slot(3), cx).await;
    assert_eq!(
        layout(handle, cx).wheel,
        [
            "push",
            "omw",
            "allin",
            "bait",
            "assist",
            "needvision",
            "missing",
            "enemyvision"
        ]
    );

    // A library signal already on the wheel swaps, so the layout remains unique.
    drag(handle, signal(6), slot(1), cx).await;
    assert_eq!(layout(handle, cx).wheel[1], "missing");
    assert_eq!(layout(handle, cx).wheel[6], "omw");
    send(
        handle,
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse("cmd-z").unwrap(),
            is_held: false,
        }),
        cx,
    )
    .await;
    assert_eq!(
        layout(handle, cx).wheel[1],
        "omw",
        "undo restores the previous order"
    );

    // Invalid destinations and Escape cancel without changing the persisted layout.
    let before = layout(handle, cx);
    for destination in [
        center,
        center + point(px(195.), px(195.)),
        point(px(6.), px(6.)),
    ] {
        drag(handle, signal(10), destination, cx).await;
        assert!(
            layout(handle, cx) == before,
            "dead zone and outside drops must cancel"
        );
    }
    start_drag(handle, slot(3), slot(5), cx).await;
    send(
        handle,
        PlatformInput::KeyDown(KeyDownEvent {
            keystroke: Keystroke::parse("escape").unwrap(),
            is_held: false,
        }),
        cx,
    )
    .await;
    release(handle, slot(5), cx).await;
    assert!(
        layout(handle, cx) == before,
        "Escape must cancel reordering"
    );

    drag(handle, signal(10), click_target, cx).await;
    assert_eq!(layout(handle, cx).click, "visioncleared");
    assert_eq!(
        layout(handle, cx).wheel,
        before.wheel,
        "changing click signal must not change the wheel"
    );

    // Click-to-place remains available alongside drag and drop.
    click(handle, signal(8), cx).await;
    click(handle, slot(4), cx).await;
    assert_eq!(layout(handle, cx).wheel[4], "generic");
    handle
        .update(cx, |view, _, cx| {
            let model = view.model.read(cx);
            let saved = Settings::load(&model.settings_path, &model.settings_path).unwrap();
            assert_eq!(saved.wheel, model.settings.wheel);
            assert_eq!(saved.click_ping_id, "visioncleared");
        })
        .unwrap();

    // At the minimum window size, the controls remain reachable by scrolling.
    handle
        .update(cx, |_, window, _| window.resize(size(px(1000.), px(600.))))
        .unwrap();
    Timer::after(Duration::from_millis(250)).await;
    handle
        .update(cx, |view, _, cx| {
            view.scroll.set_offset(point(px(0.), px(-900.)));
            cx.notify();
        })
        .unwrap();
    Timer::after(Duration::from_millis(250)).await;
    handle
        .update(cx, |view, _, _| {
            assert!(
                view.scroll.offset().y < px(-20.),
                "compact settings must scroll at minimum height"
            )
        })
        .unwrap();
    handle
        .update(cx, |view, window, cx| {
            window.resize(size(px(1080.), px(680.)));
            view.scroll.set_offset(Point::default());
            cx.notify();
        })
        .unwrap();
    eprintln!(
        "Editor smoke passed: library replacement, slot reordering, swaps, undo, invalid drops, Escape, click signal, click-to-place, persistence and minimum-size scrolling"
    );
}
