#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "windows.rs"]
mod platform;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod ui;

fn main() -> anyhow::Result<()> {
    struct Logger;
    impl log::Log for Logger {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Warn
        }
        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                eprintln!("{}: {}", record.level(), record.args());
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: Logger = Logger;
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Warn);
    if std::env::args().any(|a| a == "--list-assets") {
        for path in lolping_native::catalog::asset_files() {
            println!("{path}");
        }
        return Ok(());
    }
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    return ui::run();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    anyhow::bail!("The native UI supports macOS and Windows.");
}
