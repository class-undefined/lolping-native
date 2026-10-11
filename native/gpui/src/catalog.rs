pub struct Ping {
    pub id: &'static str,
    pub name: &'static str,
    pub icon: &'static str,
    pub art: &'static [&'static str],
    pub sound: &'static str,
    pub color: u32,
}

pub const PINGS: &[Ping] = &[
    Ping {
        id: "danger",
        name: "危险",
        icon: "pingwheel_retreatrender",
        art: &["pingwheel_retreatrender"],
        sound: "SRP_12",
        color: 0xff3b55,
    },
    Ping {
        id: "push",
        name: "推进",
        icon: "push",
        art: &["pingwheel_pushrender"],
        sound: "SRP_9",
        color: 0x35d27a,
    },
    Ping {
        id: "omw",
        name: "正在赶来",
        icon: "omw",
        art: &["omw"],
        sound: "OnMyWay",
        color: 0x2e8bff,
    },
    Ping {
        id: "allin",
        name: "全力进攻",
        icon: "all_in",
        art: &["pingwheel_allin_render_01", "pingwheel_allin_render_02"],
        sound: "SRP_4",
        color: 0xf2b320,
    },
    Ping {
        id: "assist",
        name: "请求协助",
        icon: "assist_me",
        art: &["assist_me"],
        sound: "ComeHere",
        color: 0x56d43c,
    },
    Ping {
        id: "needvision",
        name: "需要视野",
        icon: "need_vision",
        art: &["need_vision"],
        sound: "SRP_7",
        color: 0x56d43c,
    },
    Ping {
        id: "missing",
        name: "敌人消失",
        icon: "enemy_missing",
        art: &["enemy_missing"],
        sound: "MIA",
        color: 0xf2c518,
    },
    Ping {
        id: "enemyvision",
        name: "敌方视野",
        icon: "enemy_vision",
        art: &["pingwheel_enemyvisionrender"],
        sound: "AreaIsWarded",
        color: 0xff3b55,
    },
    Ping {
        id: "generic",
        name: "普通信号",
        icon: "generic_ping",
        art: &["pingwheel_basicpingrender"],
        sound: "Base",
        color: 0x2e8bff,
    },
    Ping {
        id: "bait",
        name: "诱饵",
        icon: "pingwheel_baitrender",
        art: &["pingwheel_baitrender"],
        sound: "SRP_11",
        color: 0xefbc12,
    },
    Ping {
        id: "visioncleared",
        name: "视野已清除",
        icon: "vision_cleared",
        art: &["pingwheel_visionclearedrender"],
        sound: "SRP_6",
        color: 0x2b80ff,
    },
];

pub fn index(id: &str) -> Option<usize> {
    PINGS.iter().position(|p| p.id == id)
}

pub fn asset_files() -> Vec<String> {
    let mut files = vec!["sounds/button.wav".to_owned()];
    for ping in PINGS {
        files.push(format!("textures/{}.png", ping.icon));
        for art in ping.art {
            files.push(format!("textures/{art}.png"));
        }
        files.push(format!("sounds/{}.wav", ping.sound));
    }
    files.sort();
    files.dedup();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assets_exist_and_exclude_emotes() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        for path in asset_files() {
            assert!(root.join(&path).is_file(), "missing {path}");
            assert!(!path.starts_with("emotes/"));
        }
    }
}
