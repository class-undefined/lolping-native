use crate::catalog::{PINGS, index};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub enabled_on_start: bool,
    pub trigger: String,
    pub click_ping: bool,
    pub click_ping_id: String,
    pub wheel: [String; 8],
    pub drag_threshold_px: u32,
    pub ping_size_px: f32,
    pub ping_duration_s: f32,
    pub volume: u32,
    pub muted: bool,
    pub tick_sound: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled_on_start: true,
            trigger: "alt".into(),
            click_ping: false,
            click_ping_id: "generic".into(),
            wheel: std::array::from_fn(|i| PINGS[i].id.into()),
            drag_threshold_px: 8,
            ping_size_px: 110.,
            ping_duration_s: 3.2,
            volume: 70,
            muted: false,
            tick_sound: true,
        }
    }
}

#[cfg(target_os = "macos")]
pub const TRIGGERS: &[(&str, &str)] = &[
    ("alt", "⌥ Option"),
    ("ctrl", "⌃ Control"),
    ("shift", "⇧ Shift"),
    ("win", "⌘ Command"),
    ("mouse4", "鼠标侧键 4"),
    ("mouse5", "鼠标侧键 5"),
];

#[cfg(not(target_os = "macos"))]
pub const TRIGGERS: &[(&str, &str)] = &[
    ("alt", "Alt"),
    ("ctrl", "Ctrl"),
    ("shift", "Shift"),
    ("win", "Windows"),
    ("mouse4", "鼠标侧键 4"),
    ("mouse5", "鼠标侧键 5"),
];

impl Settings {
    /// Normalize fields independently so one malformed legacy field cannot discard all settings.
    pub fn from_value(raw: Value) -> Self {
        let defaults = serde_json::to_value(Self::default()).unwrap();
        let mut normalized = defaults.clone();
        if let (Some(out), Some(input), Some(defaults)) = (
            normalized.as_object_mut(),
            raw.as_object(),
            defaults.as_object(),
        ) {
            for (key, fallback) in defaults {
                if let Some(value) = input.get(key) {
                    let valid = if fallback.is_boolean() {
                        value.is_boolean()
                    } else if fallback.is_string() {
                        value.is_string()
                    } else if fallback.is_number() {
                        value.is_number()
                    } else {
                        value.is_array()
                    };
                    if valid {
                        out.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        // Individually decode fields using a one-field object and serde defaults.
        let mut result = defaults.as_object().unwrap().clone();
        for (key, value) in normalized.as_object().unwrap() {
            let one = json!({key: value});
            if serde_json::from_value::<Self>(one).is_ok() {
                result.insert(key.clone(), value.clone());
            }
        }
        let mut s: Self = serde_json::from_value(Value::Object(result)).unwrap_or_default();
        if !TRIGGERS.iter().any(|(id, _)| *id == s.trigger) {
            s.trigger = "alt".into();
        }
        if index(&s.click_ping_id).is_none() {
            s.click_ping_id = "generic".into();
        }
        if s.wheel.iter().any(|id| index(id).is_none())
            || (0..8).any(|i| s.wheel[..i].contains(&s.wheel[i]))
        {
            s.wheel = Self::default().wheel;
        }
        s.drag_threshold_px = s.drag_threshold_px.clamp(2, 40);
        s.ping_size_px = s.ping_size_px.clamp(60., 220.);
        s.ping_duration_s = s.ping_duration_s.clamp(1., 8.);
        s.volume = s.volume.min(100);
        s
    }

    pub fn load(path: &Path, legacy: &Path) -> Result<Self> {
        let source = if path.exists() { path } else { legacy };
        if !source.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(source).with_context(|| format!("读取 {} 失败", source.display()))?;
        Ok(Self::from_value(
            serde_json::from_slice(&bytes).context("设置文件不是有效 JSON")?,
        ))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let dir = path.parent().context("设置目录不存在")?;
        fs::create_dir_all(dir)?;
        let temporary = path.with_extension("json.tmp");
        let mut f = fs::File::create(&temporary)?;
        f.write_all(&serde_json::to_vec_pretty(self)?)?;
        f.sync_all()?;
        fs::rename(temporary, path)?;
        Ok(())
    }

    pub fn place(&mut self, slot: usize, ping: usize) {
        if slot >= 8 || ping >= PINGS.len() {
            return;
        }
        let id = PINGS[ping].id;
        if let Some(from) = self.wheel.iter().position(|p| p == id) {
            self.wheel.swap(from, slot);
        } else {
            self.wheel[slot] = id.into();
        }
    }

    /// Insert a wheel slot at a new position, preserving the order of the other signals.
    pub fn move_slot(&mut self, from: usize, to: usize) {
        if from >= self.wheel.len() || to >= self.wheel.len() || from == to {
            return;
        }
        if from < to {
            self.wheel[from..=to].rotate_left(1);
        } else {
            self.wheel[to..=from].rotate_right(1);
        }
    }

    pub fn helper_config(&self, enabled: bool) -> Value {
        json!({"type":"config", "trigger":self.trigger, "triggerVk":0,
            "clickPing":self.click_ping, "emoteTrigger":"off", "emoteTriggerVk":0,
            "emoteClick":false, "dragThresholdPx":self.drag_threshold_px,
            "toggleMods":3, "toggleVk":80, "enabled":enabled})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corrupt_fields_are_independent_and_ranges_are_bounded() {
        let s = Settings::from_value(json!({"wheel":["bogus"],"volume":400,"trigger":{"vk":65},
            "pingSizePx":999,"pingDurationS":-2,"muted":true,"dragThresholdPx":-1}));
        assert_eq!(s.volume, 100);
        assert_eq!(s.ping_size_px, 220.);
        assert_eq!(s.ping_duration_s, 1.);
        assert_eq!(s.drag_threshold_px, 8);
        assert_eq!(s.trigger, "alt");
        assert!(s.muted);
        assert_eq!(s.wheel, Settings::default().wheel);
    }
    #[test]
    fn saving_twice_replaces_existing_settings_in_unicode_directory() {
        let dir = std::env::temp_dir().join(format!("lolping-设置-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut settings = Settings::default();
        settings.save(&path).unwrap();
        settings.volume = 23;
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path, &path).unwrap(), settings);
        assert!(!path.with_extension("json.tmp").exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn placement_swaps_and_legacy_roundtrip_preserves_supported_fields() {
        let mut s = Settings::default();
        s.place(0, 6);
        assert_eq!(s.wheel[0], "missing");
        assert_eq!(s.wheel[6], "danger");
        s.place(3, 10);
        assert_eq!(s.wheel[3], "visioncleared");
        assert_eq!(Settings::from_value(serde_json::to_value(&s).unwrap()), s);
        assert_eq!(s.helper_config(true)["emoteTrigger"], "off");
    }

    #[test]
    fn moving_slots_preserves_order_and_survives_reload() {
        let mut s = Settings::default();
        s.move_slot(0, 3);
        assert_eq!(
            s.wheel,
            [
                "push",
                "omw",
                "allin",
                "danger",
                "assist",
                "needvision",
                "missing",
                "enemyvision"
            ]
        );
        assert_eq!(Settings::from_value(serde_json::to_value(&s).unwrap()), s);
        s.move_slot(3, 0);
        assert_eq!(s.wheel, Settings::default().wheel);
        s.move_slot(7, 0);
        assert_eq!(s.wheel[0], "enemyvision");
        assert_eq!(s.wheel[7], "missing");
    }

    #[test]
    fn no_op_or_invalid_edits_leave_layout_unchanged() {
        let mut s = Settings::default();
        for (from, to) in [(2, 2), (8, 0), (0, 8), (usize::MAX, 1)] {
            s.move_slot(from, to);
        }
        s.place(0, 0);
        s.place(8, 0);
        s.place(0, PINGS.len());
        assert_eq!(s, Settings::default());
    }

    #[test]
    fn replacements_and_reordering_keep_all_eight_slots_unique() {
        for (ping, definition) in PINGS.iter().enumerate() {
            for target in 0..8 {
                let mut s = Settings::default();
                s.place(target, ping);
                assert_eq!(s.wheel[target], definition.id);
                s.move_slot(target, (target + 3) % 8);
                assert_eq!(Settings::from_value(serde_json::to_value(&s).unwrap()), s);
            }
        }
    }
}
