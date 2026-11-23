//! A plain-text log for troubleshooting: `<data>/zoms-nexus.log`, kept under about 2 MB (the
//! older half moves to `zoms-nexus.old.log`). Errors from Nexus Mods, downloads, unpacking,
//! Play and the page itself go here. API keys and download keys are never written.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

static FILE: OnceLock<PathBuf> = OnceLock::new();
static TURN: Mutex<()> = Mutex::new(());
const MAX: u64 = 2 * 1024 * 1024;

pub fn init(data: &std::path::Path) {
    let _ = fs::create_dir_all(data);
    let _ = FILE.set(data.join("zoms-nexus.log"));
}

pub fn path() -> Option<PathBuf> {
    FILE.get().cloned()
}

/// A Unix time moved to this computer's time zone (so the log reads like the clock).
pub fn local(t: u64) -> u64 {
    #[cfg(unix)]
    {
        let tt = t as libc::time_t;
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        // SAFETY: tt and tm are valid for localtime_r, which only writes into tm.
        if !unsafe { libc::localtime_r(&tt, &mut tm) }.is_null() {
            return (t as i64 + tm.tm_gmtoff as i64).max(0) as u64;
        }
    }
    t
}

/// The hour of the day here (0 to 23).
pub fn hour_now() -> u8 {
    ((local(crate::queue::now()) % 86_400) / 3600) as u8
}

/// "2026-11-22 10:04:05" from a Unix time (as given: UTC, or already moved by `local`).
pub fn stamp(t: u64) -> String {
    let days = (t / 86400) as i64;
    let secs = t % 86400;
    // Civil date from days since 1970 (Howard Hinnant's algorithm).
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60)
}

/// Blank out anything that looks like a secret: `key=...` in nxm links and long tokens.
pub fn scrub(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split_inclusive(|c: char| c.is_whitespace()) {
        let w = word.trim_end();
        let tail = &word[w.len()..];
        if let Some(i) = w.find("key=") {
            let end = w[i..].find('&').map(|e| i + e).unwrap_or(w.len());
            out.push_str(&w[..i + 4]);
            out.push('…');
            out.push_str(&w[end..]);
        } else if w.len() >= 40 && w.chars().all(|c| c.is_ascii_alphanumeric() || "-_=+/".contains(c)) {
            out.push_str("[hidden]");
        } else {
            out.push_str(w);
        }
        out.push_str(tail);
    }
    out
}

pub fn write(level: &str, msg: &str) {
    let Some(path) = FILE.get() else { return };
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    if fs::metadata(path).map(|m| m.len() > MAX).unwrap_or(false) {
        let _ = fs::rename(path, path.with_file_name("zoms-nexus.old.log"));
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} [{level}] {}", stamp(local(crate::queue::now())), scrub(msg).replace('\n', " | "));
    }
}

pub fn info(msg: &str) {
    write("info", msg)
}
pub fn warn(msg: &str) {
    write("warn", msg)
}
pub fn error(msg: &str) {
    write("error", msg)
}

/// The last `lines` lines of the log.
pub fn tail(lines: usize) -> String {
    let Some(path) = FILE.get() else { return String::new() };
    let text = fs::read_to_string(path).unwrap_or_default();
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_and_scrubs() {
        assert_eq!(stamp(0), "1970-01-01 00:00:00");
        assert_eq!(stamp(1_794_560_497), "2026-11-13 09:01:37");
        assert_eq!(scrub("nxm://g/mods/1/files/2?key=SECRET123&expires=9 ok"), "nxm://g/mods/1/files/2?key=…&expires=9 ok");
        assert_eq!(scrub("token abcdefghijABCDEFGHIJ0123456789abcdefghij end"), "token [hidden] end");
    }
}
