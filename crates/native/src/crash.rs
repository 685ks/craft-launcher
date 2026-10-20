//! Opt-in local crash reports.
//!
//! When the user has switched them on (Settings, "Crash reports"), the shell's panic hook calls
//! [`on_panic`], which writes a folder `crash-<unix time>-<pid>` under the launcher's data folder
//! (`crash-reports/`) holding `report.txt` (version, system, the panic and its backtrace) and
//! `log.txt` (the end of this run's log). Nothing is ever sent anywhere: the user opens the
//! folder from Settings and decides what to share. Only the newest [`KEEP`] reports are kept.
//!
//! Everything here runs inside a panic hook, so it never panics itself and gives up quietly on
//! any error.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

/// How many reports are kept.
pub const KEEP: usize = 20;
/// The most of the log copied into a report.
const LOG_TAIL: u64 = 256 * 1024;

static ENABLED: AtomicBool = AtomicBool::new(false);
static WHERE: Mutex<Option<Where>> = Mutex::new(None);

#[derive(Clone)]
struct Where {
    dir: PathBuf,
    log: Option<PathBuf>,
}

/// The folder reports go in.
pub fn reports_dir(paths: &crate::paths::Paths) -> PathBuf {
    paths.data.join("crash-reports")
}

/// Where reports go and which log to copy into them.
pub fn configure(dir: PathBuf, log: Option<PathBuf>) {
    *WHERE.lock().unwrap_or_else(PoisonError::into_inner) = Some(Where { dir, log });
}

/// Switches reports on or off (the user's setting).
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// From the panic hook: writes a report when they're switched on. Returns the report's folder.
pub fn on_panic(report: &str) -> Option<PathBuf> {
    if !enabled() {
        return None;
    }
    // `try_lock`: a panic while the lock is held must not deadlock the hook.
    let place = WHERE.try_lock().ok().and_then(|w| w.clone())?;
    let log = place.log.as_deref().and_then(|p| read_tail(p, LOG_TAIL));
    write_report(&place.dir, crate::now(), std::process::id(), report, log.as_deref()).ok()
}

/// The last `max` bytes of `path`, as text.
fn read_tail(path: &Path, max: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(max))).ok()?;
    let mut bytes = Vec::new();
    file.take(max).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Writes one report folder and drops the oldest beyond [`KEEP`].
pub fn write_report(dir: &Path, now: i64, pid: u32, report: &str, log: Option<&str>) -> Result<PathBuf, String> {
    let folder = dir.join(format!("crash-{now}-{pid}"));
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let text = format!(
        "Arthavan Projects {version}\nSystem: {os} {arch}\nTime: {now} (Unix seconds)\n\n{report}\n",
        version = env!("CARGO_PKG_VERSION"),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
    );
    std::fs::write(folder.join("report.txt"), text).map_err(|e| e.to_string())?;
    if let Some(log) = log {
        std::fs::write(folder.join("log.txt"), log).map_err(|e| e.to_string())?;
    }
    prune(dir, KEEP);
    Ok(folder)
}

/// Removes all but the newest `keep` report folders (by the time in their names).
fn prune(dir: &Path, keep: usize) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut reports: Vec<(i64, PathBuf)> = entries
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let time = name.strip_prefix("crash-")?.split('-').next()?.parse::<i64>().ok()?;
            e.file_type().ok().filter(std::fs::FileType::is_dir).map(|_| (time, e.path()))
        })
        .collect();
    reports.sort();
    let extra = reports.len().saturating_sub(keep);
    for (_, path) in reports.into_iter().take(extra) {
        let _ = std::fs::remove_dir_all(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_holds_the_panic_and_the_log() {
        let dir = tempfile::tempdir().unwrap();
        let folder = write_report(dir.path(), 1_792_500_000, 42, "internal error: boom", Some("line 1\nline 2\n")).unwrap();
        assert!(folder.ends_with("crash-1792500000-42"));
        let report = std::fs::read_to_string(folder.join("report.txt")).unwrap();
        assert!(report.contains("internal error: boom"));
        assert!(report.contains(env!("CARGO_PKG_VERSION")));
        assert_eq!(std::fs::read_to_string(folder.join("log.txt")).unwrap(), "line 1\nline 2\n");
    }

    #[test]
    fn only_the_newest_reports_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"not a report").unwrap();
        for t in 0..(KEEP as i64 + 5) {
            write_report(dir.path(), 1000 + t, 1, "x", None).unwrap();
        }
        let left: Vec<String> = std::fs::read_dir(dir.path()).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left.iter().filter(|n| n.starts_with("crash-")).count(), KEEP);
        assert!(!left.contains(&"crash-1000-1".to_string()));
        assert!(left.contains(&format!("crash-{}-1", 1000 + KEEP as i64 + 4)));
        assert!(left.contains(&"notes.txt".to_string()));
    }

    #[test]
    fn the_log_tail_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("a.log");
        std::fs::write(&log, "abcdefghij").unwrap();
        assert_eq!(read_tail(&log, 4).as_deref(), Some("ghij"));
        assert_eq!(read_tail(&log, 100).as_deref(), Some("abcdefghij"));
        assert_eq!(read_tail(&dir.path().join("missing"), 4), None);
    }

    #[test]
    fn nothing_is_written_while_switched_off() {
        let dir = tempfile::tempdir().unwrap();
        configure(dir.path().join("reports"), None);
        set_enabled(false);
        assert_eq!(on_panic("boom"), None);
        assert!(!dir.path().join("reports").exists());
        set_enabled(true);
        let folder = on_panic("boom").unwrap();
        assert!(folder.join("report.txt").exists());
        set_enabled(false);
    }
}
