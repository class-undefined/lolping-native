#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
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
    #[cfg(target_os = "macos")]
    return ui::run();
    #[cfg(not(target_os = "macos"))]
    anyhow::bail!("The native UI currently supports macOS only.");
}
