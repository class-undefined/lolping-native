use lolping_native::{
    helper::{Event, Helper},
    settings::Settings,
};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

fn simulate(commands: Vec<Value>) -> Vec<Event> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../hook-helper/build/hook-helper");
    assert!(
        path.is_file(),
        "Run sh native/hook-helper/build.sh before cargo test"
    );
    let (mut helper, receiver) = Helper::start(&path, true).unwrap();
    helper
        .send(&Settings::default().helper_config(true))
        .unwrap();
    for command in commands {
        helper.send(&command).unwrap();
    }
    helper.send(&json!({"type":"shutdown"})).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "input helper did not finish");
        if let Ok(event) = receiver.try_recv() {
            if matches!(event, Event::Disconnected) {
                break;
            }
            events.push(event);
        } else {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    events
}

#[test]
fn real_helper_opens_moves_releases_in_global_points() {
    let events = simulate(vec![
        json!({"type":"sim","ev":"kdown","vk":164}),
        json!({"type":"sim","ev":"mdown","btn":"left","x":-200,"y":50}),
        json!({"type":"sim","ev":"move","x":-40,"y":50}),
        json!({"type":"sim","ev":"mup","btn":"left","x":-40,"y":50}),
    ]);
    assert!(matches!(events[0], Event::Ready { version: 2 }));
    assert!(matches!(events[1], Event::WheelOpen { x: -200, y: 50, .. }));
    assert!(matches!(events[2], Event::WheelMove { x: -40, y: 50 }));
    assert!(matches!(events[3], Event::WheelRelease { x: -40, y: 50 }));
    assert_eq!(events.len(), 4);
}

#[test]
fn emote_trigger_is_disabled_and_pause_shortcut_is_preserved() {
    let events = simulate(vec![
        json!({"type":"sim","ev":"kdown","vk":162}),
        json!({"type":"sim","ev":"mdown","btn":"left","x":100,"y":100}),
        json!({"type":"sim","ev":"move","x":300,"y":100}),
        json!({"type":"sim","ev":"mup","btn":"left","x":300,"y":100}),
        json!({"type":"sim","ev":"kdown","vk":164}),
        json!({"type":"sim","ev":"kdown","vk":80}),
    ]);
    assert_eq!(events.len(), 2);
    assert!(matches!(events[1], Event::Toggled { enabled: false }));
}

#[test]
fn escape_cancels_without_emitting_a_signal() {
    let events = simulate(vec![
        json!({"type":"sim","ev":"kdown","vk":164}),
        json!({"type":"sim","ev":"mdown","btn":"left","x":100,"y":100}),
        json!({"type":"sim","ev":"move","x":300,"y":100}),
        json!({"type":"sim","ev":"kdown","vk":27}),
        json!({"type":"sim","ev":"mup","btn":"left","x":300,"y":100}),
    ]);
    assert!(events.iter().any(|e| matches!(e, Event::Cancel)));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::WheelRelease { .. } | Event::Click { .. }))
    );
}
