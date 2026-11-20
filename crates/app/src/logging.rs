//! 日志写到 `%LOCALAPPDATA%\yume-rhythm\logs\yume.log`。启动时超过 1 MB 就清空重写。

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_BYTES: u64 = 1024 * 1024;

struct FileLogger {
    file: Mutex<File>,
    level: log::LevelFilter,
}

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(
                file,
                "{} {:<5} {}: {}",
                timestamp(),
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

/// 初始化日志。失败（目录不可写等）时不记录日志，程序照常运行。
pub fn init(dir: Option<&Path>) {
    let Some(dir) = dir else { return };
    let path = dir.join("yume.log");
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let too_big = std::fs::metadata(&path).is_ok_and(|m| m.len() > MAX_BYTES);
    let file = OpenOptions::new()
        .create(true)
        .append(!too_big)
        .write(true)
        .truncate(too_big)
        .open(&path);
    let Ok(file) = file else { return };
    let level = if cfg!(debug_assertions) {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };
    let logger = Box::new(FileLogger {
        file: Mutex::new(file),
        level,
    });
    if log::set_boxed_logger(logger).is_ok() {
        log::set_max_level(level);
    }
    std::panic::set_hook(Box::new(|info| {
        log::error!("程序崩溃：{info}");
        log::logger().flush();
    }));
}

/// UTC 时间 `YYYY-MM-DD hh:mm:ss.mmm`。
fn timestamp() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    let secs = i64::try_from(ms / 1000).unwrap_or(0);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}.{:03}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60,
        ms % 1000
    )
}

/// 公历日期（Howard Hinnant 的算法）。
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_412), (2025, 11, 20));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
    }
}
