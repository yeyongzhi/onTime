use log::{LevelFilter, Log, Metadata, Record};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
struct FileLogger(Mutex<Option<PathBuf>>);
static LOGGER: FileLogger = FileLogger(Mutex::new(None));
const MAX_BYTES: u64 = 5 * 1024 * 1024;
pub fn install(root: Option<&Path>) {
    set_root(root);
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(LevelFilter::Warn);
    }
}
pub fn set_root(root: Option<&Path>) {
    if let Ok(mut target) = LOGGER.0.lock() {
        *target = root.map(|p| p.join("logs"));
    }
}
impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn && metadata.target().starts_with("app_lib")
    }
    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let Ok(guard) = self.0.lock() else {
            return;
        };
        let Some(dir) = guard.as_ref() else {
            return;
        };
        if !dir.is_dir() {
            return;
        }
        let path = dir.join("app.log");
        let line = format!(
            "{} {} {}\n",
            chrono::Utc::now().to_rfc3339(),
            record.level(),
            record.args()
        );
        if fs::metadata(&path).is_ok_and(|m| m.len() + line.len() as u64 > MAX_BYTES) {
            let oldest = dir.join("app.4.log");
            if oldest.exists() && fs::remove_file(&oldest).is_err() {
                return;
            }
            for n in (1..4).rev() {
                let source = dir.join(format!("app.{n}.log"));
                if source.exists()
                    && fs::rename(&source, dir.join(format!("app.{}.log", n + 1))).is_err()
                {
                    return;
                }
            }
            if fs::rename(&path, dir.join("app.1.log")).is_err() {
                return;
            }
        }
        if let Ok(mut file) = fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = file.write_all(line.as_bytes());
        }
    }
    fn flush(&self) {}
}
