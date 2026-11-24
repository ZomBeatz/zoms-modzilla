//! The Activity page: things worth knowing about later (a download that failed, updates found,
//! a mod gone from Nexus Mods, a profile whose check got worse, saves switched, time played),
//! each pointing at where to deal with it. Kept in `<data>/activity.json`, newest first, the
//! last 300. The detailed log for troubleshooting stays in `log.rs`.
//!
//! Also play time: after Play, the app watches for the game running and notes how long.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tauri::{AppHandle, Emitter};

const KEEP: usize = 300;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: u64,
    pub at: u64,
    /// "failed", "updates", "gone", "worse", "saves", "played", "quickExit", "done".
    pub kind: String,
    pub title: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub game: Option<String>,
    /// The page that deals with it: "downloads", "library", "profiles".
    #[serde(default)]
    pub page: Option<String>,
    /// A profile to open there.
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub read: bool,
}

static PLACE: OnceLock<(PathBuf, AppHandle)> = OnceLock::new();
static TURN: Mutex<()> = Mutex::new(());

pub fn init(data: &Path, handle: AppHandle) {
    let _ = PLACE.set((data.to_path_buf(), handle));
}

fn file(data: &Path) -> PathBuf {
    data.join("activity.json")
}

pub fn list_in(data: &Path) -> Vec<Event> {
    fs::read_to_string(file(data)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_in(data: &Path, all: &[Event]) {
    if let Ok(b) = serde_json::to_vec(all) {
        let _ = crate::settings::write_atomic(&file(data), &b);
    }
}

/// Tell the page something changed (an event name it listens for).
pub fn tell(event: &str) {
    if let Some((_, handle)) = PLACE.get() {
        let _ = handle.emit(event, 0);
    }
}

/// Problems the person said to ignore on the Activity page ("profile id|problem").
pub fn ignored(data: &Path) -> Vec<String> {
    fs::read_to_string(data.join("ignored.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn set_ignored(data: &Path, key: &str, on: bool) -> Result<Vec<String>, String> {
    let _turn = TURN.lock().unwrap();
    let mut all = ignored(data);
    all.retain(|k| k != key);
    if on {
        all.push(key.chars().take(400).collect());
    }
    // Old ones go first when the list gets long.
    let extra = all.len().saturating_sub(2000);
    all.drain(..extra);
    crate::settings::write_atomic(&data.join("ignored.json"), &serde_json::to_vec(&all).map_err(|e| e.to_string())?)?;
    Ok(all)
}

pub fn list() -> Vec<Event> {
    PLACE.get().map(|(d, _)| list_in(d)).unwrap_or_default()
}

/// Change the list (under a lock) and tell the page.
pub fn change(f: impl FnOnce(&mut Vec<Event>)) {
    let Some((data, handle)) = PLACE.get() else { return };
    let _turn = TURN.lock().unwrap();
    let mut all = list_in(data);
    f(&mut all);
    all.truncate(KEEP);
    save_in(data, &all);
    let _ = handle.emit("activity:changed", all.iter().filter(|e| !e.read).count());
}

/// Note something. An unread event of the same kind and title is replaced, not repeated.
static OFF: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Kinds of entry the person doesn't want noted (Settings).
pub fn set_off(kinds: &[String]) {
    *OFF.lock().unwrap() = kinds.to_vec();
}

pub fn add(kind: &str, title: impl Into<String>, detail: Option<String>, game: Option<&str>, page: Option<&str>, profile: Option<&str>) {
    if OFF.lock().unwrap().iter().any(|k| k == kind) {
        return;
    }
    let title = title.into();
    let now = crate::queue::now();
    change(|all| {
        all.retain(|e| e.read || !(e.kind == kind && e.title == title));
        let id = all.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        all.insert(0, Event { id, at: now, kind: kind.into(), title, detail, game: game.map(String::from), page: page.map(String::from), profile: profile.map(String::from), read: false });
    });
}

/// A download that failed: grouped into one unread entry while they keep coming.
pub fn failed(name: &str, why: &str) {
    if OFF.lock().unwrap().iter().any(|k| k == "failed") {
        return;
    }
    let now = crate::queue::now();
    change(|all| {
        if let Some(e) = all.iter_mut().find(|e| !e.read && e.kind == "failed") {
            let mut names: Vec<String> = e.detail.as_deref().and_then(|d| d.split_once("page: ")).map(|(_, rest)| rest.trim_end_matches('.').split(", ").map(String::from).collect()).unwrap_or_default();
            if !names.iter().any(|n| n == name) {
                names.push(name.to_string());
            }
            e.title = format!("{} downloads didn't finish", names.len());
            e.detail = Some(format!("Try them again on the Downloads page: {}.", names.join(", ")));
            e.at = now;
            return;
        }
        let id = all.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        all.insert(0, Event { id, at: now, kind: "failed".into(), title: format!("“{name}” didn't download"), detail: Some(format!("{why} Try it again on the Downloads page: {name}.")), game: None, page: Some("downloads".into()), profile: None, read: false });
    });
}

// ---- Play time ----

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlayTime {
    pub seconds: u64,
    pub sessions: u32,
    pub last_played: Option<u64>,
}

fn play_file(data: &Path) -> PathBuf {
    data.join("playtime.json")
}

pub fn play_times(data: &Path) -> HashMap<String, PlayTime> {
    fs::read_to_string(play_file(data)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn note_play(data: &Path, profile: &str, start: u64, seconds: u64) {
    let _turn = TURN.lock().unwrap();
    let mut all = play_times(data);
    let p = all.entry(profile.to_string()).or_default();
    p.seconds += seconds;
    p.sessions += 1;
    p.last_played = Some(start);
    if let Ok(b) = serde_json::to_vec(&all) {
        let _ = crate::settings::write_atomic(&play_file(data), &b);
    }
}

/// How a game's folder can show up in a process's command line: its Linux path, and for a
/// Steam game the `steamapps/common/<game>` part, which Proton puts after whatever drive letter
/// it gave the library (Z:, S:, …). Lowercase, with forward slashes.
fn patterns(game: &Path) -> Vec<String> {
    let g = game.to_string_lossy().trim_end_matches('/').to_lowercase();
    let mut out = vec![g.clone()];
    if let Some(i) = g.find("/steamapps/common/") {
        out.push(g[i..].to_string());
    }
    out
}

/// Whether a process's command line or program is inside the game's folder (for the tests,
/// with a made-up /proc entry).
#[cfg(test)]
fn runs_from(pid_dir: &Path, game: &Path, pats: &[String]) -> bool {
    let cmd = fs::read(pid_dir.join("cmdline")).map(|b| String::from_utf8_lossy(&b).replace('\0', " ")).unwrap_or_default().replace('\\', "/").to_lowercase();
    cmd_in(&cmd, pats) || fs::read_link(pid_dir.join("exe")).map(|p| p.starts_with(game)).unwrap_or(false)
}

/// Whether a command line is from inside the game's folder.
fn cmd_in(cmd: &str, pats: &[String]) -> bool {
    // A folder name, not a longer one starting the same ("Game" vs "Game 2").
    pats.iter().any(|p| cmd.match_indices(p.as_str()).any(|(i, _)| cmd[i + p.len()..].starts_with('/')))
}

/// Is anything running from the game's folder?
pub fn running(game: &Path) -> bool {
    !processes(game).is_empty()
}

/// The processes running from the game's folder (never this app). Outside a Flatpak, a
/// program's own path counts too (for games started without their folder on the command line).
pub fn processes(game: &Path) -> Vec<i32> {
    if game.to_string_lossy().len() < 5 {
        return vec![];
    }
    let pats = patterns(game);
    let flatpak = crate::host::in_flatpak();
    crate::host::processes()
        .into_iter()
        .filter(|p| cmd_in(&p.cmd, &pats) || (!flatpak && fs::read_link(format!("/proc/{}/exe", p.pid)).map(|e| e.starts_with(game)).unwrap_or(false)))
        .map(|p| p.pid)
        .collect()
}

static CANCEL: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn cancelled(game: &str) -> bool {
    CANCEL.lock().unwrap().iter().any(|g| g == game)
}

/// Stop waiting for a game (Zom's Modzilla stops watching it), and with `end`, close the processes
/// running from its folder: asked to close first, made to after 8 seconds. Gives how many
/// processes were asked to close.
pub fn stop(game: &str, path: &Path, end: bool) -> usize {
    if WATCHING.lock().unwrap().iter().any(|g| g == game) {
        CANCEL.lock().unwrap().push(game.to_string());
    } else {
        set_state(game, "stopped", None, Some("Stopped from Zom's Modzilla".into()));
    }
    if !end {
        return 0;
    }
    let pids = processes(path);
    for pid in &pids {
        crate::host::signal(*pid, false);
    }
    if !pids.is_empty() {
        let path = path.to_path_buf();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(8));
            for pid in processes(&path) {
                // Only processes still running from the game's folder.
                crate::host::signal(pid, true);
            }
        });
    }
    crate::log::info(&format!("Stopped {game} from Zom's Modzilla ({} processes asked to close)", pids.len()));
    pids.len()
}

/// Where a game is, for the page: "applying" (mods going in), "starting" (asked to start,
/// not seen running yet), "running", "notStarted" (nothing ran within three minutes),
/// "stopped" (it ran and closed), "failed" (couldn't put mods in or start it).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    pub game: String,
    pub state: String,
    /// When this state began.
    pub since: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    /// Why it failed, or how long it ran.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

static STATES: Mutex<Vec<GameState>> = Mutex::new(Vec::new());

pub fn game_states() -> Vec<GameState> {
    STATES.lock().unwrap().clone()
}

/// Note where a game is and tell the page.
pub fn set_state(game: &str, state: &str, profile: Option<&str>, detail: Option<String>) {
    let s = GameState { game: game.into(), state: state.into(), since: crate::queue::now(), profile: profile.map(String::from), detail };
    {
        let mut all = STATES.lock().unwrap();
        all.retain(|x| x.game != game);
        all.push(s.clone());
    }
    if let Some((_, handle)) = PLACE.get() {
        let _ = handle.emit("game:state", s);
    }
}

static WATCHING: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// After Play: wait up to three minutes for the game to start, then note how long it runs.
pub fn watch(data: PathBuf, profile_id: String, profile_name: String, game: String, game_name: String, path: PathBuf) {
    {
        let mut w = WATCHING.lock().unwrap();
        if w.contains(&game) {
            return;
        }
        w.push(game.clone());
    }
    let named = (!profile_name.is_empty()).then(|| profile_name.clone());
    set_state(&game, "starting", named.as_deref(), None);
    tauri::async_runtime::spawn(async move {
        let p = path.clone();
        let mut started = None;
        for _ in 0..36 {
            if cancelled(&game) {
                break;
            }
            let p2 = p.clone();
            if tokio::task::spawn_blocking(move || running(&p2)).await.unwrap_or(false) {
                started = Some(crate::queue::now());
                set_state(&game, "running", named.as_deref(), None);
                break;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        if let Some(start) = started {
            loop {
                // Checked every few seconds, so Stop answers quickly.
                for _ in 0..5 {
                    if cancelled(&game) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
                if cancelled(&game) {
                    break;
                }
                let p2 = p.clone();
                if !tokio::task::spawn_blocking(move || running(&p2)).await.unwrap_or(false) {
                    break;
                }
            }
            let secs = crate::queue::now().saturating_sub(start);
            let stopped = cancelled(&game);
            set_state(&game, "stopped", named.as_deref(), Some(length(secs)));
            if !profile_id.is_empty() {
                note_play(&data, &profile_id, start, secs);
                let run = crate::runs::finish(&data, &profile_id, start, secs, stopped);
                if run.is_some_and(|r| r.quick()) {
                    let with = named.as_ref().map(|n| format!("With the profile “{n}”. A mod most likely stopped it: the profile shows what changed since it last ran fine."));
                    add("quickExit", format!("{game_name} closed {secs} seconds after starting"), with, Some(&game), Some("profiles"), Some(profile_id.as_str()));
                }
            }
            if secs >= 60 {
                let with = named.as_ref().map(|n| format!("With the profile “{n}”."));
                add("played", format!("Played {game_name} for {}", length(secs)), with, Some(&game), Some(if profile_id.is_empty() { "games" } else { "profiles" }), (!profile_id.is_empty()).then_some(profile_id.as_str()));
            }
            crate::log::info(&format!("{game} ran for {secs} s ({profile_name})"));
        } else if cancelled(&game) {
            set_state(&game, "stopped", named.as_deref(), Some("Stopped from Zom's Modzilla".into()));
        } else {
            crate::log::warn(&format!("{game} didn't show up as running within three minutes of Play"));
            set_state(&game, "notStarted", named.as_deref(), None);
        }
        WATCHING.lock().unwrap().retain(|g| g != &game);
        CANCEL.lock().unwrap().retain(|g| g != &game);
    });
}

/// "1 h 20 min", "12 min".
pub fn length(secs: u64) -> String {
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    if h > 0 { format!("{h} h {m} min") } else { format!("{} min", m.max(1)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_time_adds_up_and_reads_running_processes() {
        let dir = tempfile::tempdir().unwrap();
        note_play(dir.path(), "p1", 100, 600);
        note_play(dir.path(), "p1", 5000, 60);
        let t = &play_times(dir.path())["p1"];
        assert_eq!((t.seconds, t.sessions, t.last_played), (660, 2, Some(5000)));
        assert_eq!(length(4800), "1 h 20 min");
        assert_eq!(length(20), "1 min");

        // A made-up /proc entry: a Proton command line naming the game's exe.
        let proc_dir = dir.path().join("123");
        fs::create_dir_all(&proc_dir).unwrap();
        fs::write(proc_dir.join("cmdline"), "C:\\windows\\system32\\steam.exe\0S:\\steamapps\\common\\Cool Game\\bin\\game.exe\0").unwrap();
        let game = Path::new("/mnt/Steam Games/steamapps/common/Cool Game");
        assert!(runs_from(&proc_dir, game, &patterns(game)), "any drive letter Proton picked");
        let other = Path::new("/mnt/Steam Games/steamapps/common/Cool");
        assert!(!runs_from(&proc_dir, other, &patterns(other)), "a folder name, not the start of one");
        fs::write(proc_dir.join("cmdline"), "/mnt/Steam Games/steamapps/common/Cool Game/run.sh\0").unwrap();
        assert!(runs_from(&proc_dir, game, &patterns(game)));
        assert!(!running(Path::new("/")), "too short a path never matches");
    }
}
