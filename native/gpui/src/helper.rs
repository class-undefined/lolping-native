use anyhow::{Context, Result};
use async_channel::{Receiver, bounded};
use serde::Deserialize;
use serde_json::Value;
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
};

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event {
    Ready {
        version: u32,
    },
    WheelOpen {
        x: i32,
        y: i32,
        #[serde(default)]
        wheel: String,
    },
    WheelMove {
        x: i32,
        y: i32,
    },
    WheelRelease {
        x: i32,
        y: i32,
    },
    Click {
        x: i32,
        y: i32,
        #[serde(default)]
        wheel: String,
    },
    Cancel,
    Toggled {
        enabled: bool,
    },
    Error {
        message: String,
        code: Option<String>,
    },
    #[serde(skip)]
    Disconnected,
}

pub struct Helper {
    child: Child,
    stdin: ChildStdin,
}

impl Helper {
    pub fn start(path: &Path, simulate: bool) -> Result<(Self, Receiver<Event>)> {
        let mut command = Command::new(path);
        if simulate {
            command.arg("--simulate");
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .with_context(|| format!("无法启动输入助手：{}", path.display()))?;
        let stdin = child.stdin.take().context("输入助手 stdin 不可用")?;
        let stdout = child.stdout.take().context("输入助手 stdout 不可用")?;
        let (tx, rx) = bounded(256);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Ok(event) = serde_json::from_str(&line)
                    && tx.send_blocking(event).is_err()
                {
                    return;
                }
            }
            let _ = tx.send_blocking(Event::Disconnected);
        });
        Ok((Self { child, stdin }, rx))
    }

    pub fn send(&mut self, value: &Value) -> Result<()> {
        writeln!(self.stdin, "{value}")?;
        self.stdin.flush()?;
        Ok(())
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.send(&serde_json::json!({"type":"shutdown"}));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
