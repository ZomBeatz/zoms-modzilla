//! Settings in `settings.json` in the app's config folder (~/.config/com.zombeatz.zoms-nexus).
//! The API key is kept apart in `api-key`, readable only by you (mode 600), so it never ends
//! up in a copy of the settings.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// One file at a time, in the order of the list.
    Sequential,
    /// Several files at once, up to `max_concurrent`.
    Concurrent,
    /// Decided by the account: Premium downloads several at once (up to `max_concurrent`),
    /// free accounts one at a time, since Nexus Mods slows free downloads down and running
    /// several mostly splits the same speed.
    Auto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedGame {
    pub domain: String,
    pub name: String,
    pub path: Option<String>,
    /// Where mods go, under the install folder ("Data", "Mods", "" for the folder itself).
    #[serde(default)]
    pub mod_folder: Option<String>,
    /// How to start the game when it isn't a Steam game: a command, run in the install folder.
    #[serde(default)]
    pub launch: Option<String>,
    /// Its Steam app number, when it was added from the Steam games found (for Play, art and
    /// its Proton folder).
    #[serde(default)]
    pub steam_app_id: Option<u32>,
    /// Where the game keeps its saves, when set by the person (for saves per profile).
    #[serde(default)]
    pub save_folder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Where downloaded mods are kept: `<library>/<game>/<mod id> - <name>/`.
    pub library_dir: String,
    pub mode: Mode,
    /// How many files download at once in concurrent mode (1 to 8).
    pub max_concurrent: u32,
    /// Unpack zip and 7z files into a folder next to the archive when they finish.
    pub extract: bool,
    /// Keep the archive after unpacking it.
    pub keep_archives: bool,
    /// Try a failed download again by itself (up to 3 times), picking up where it stopped.
    pub auto_retry: bool,
    /// The most all downloads together may use, in bytes a second. 0 means no limit.
    pub speed_limit: u64,
    /// Games added by hand (ones that aren't found in Steam, or other stores).
    pub games: Vec<AddedGame>,
    /// Open in the background when you sign in to carry on with unfinished downloads (an entry
    /// in ~/.config/autostart). Off at first.
    pub start_with_computer: bool,
    /// Keep the API key in the desktop's keyring (KDE Wallet, GNOME Keyring) through
    /// secret-tool, instead of the file. Off at first: the keyring may ask to be unlocked.
    pub key_in_keyring: bool,
    /// Check each finished download against Nexus Mods' checksum (one request per file).
    pub verify_downloads: bool,
    /// Mods marked adult on Nexus Mods: "show" them like any other, "blur" their pictures, or
    /// "hide" them in Browse (and blur them elsewhere). The library always lists what's there.
    pub adult_content: String,
    /// Request saver: pages that would ask Nexus Mods by themselves (Browse) wait for a click.
    pub request_saver: bool,
    /// After the quick tries, keep trying a download that lost its connection later on (in 5
    /// minutes, 30 minutes, then 2 hours) instead of marking it failed.
    pub retry_later: bool,
    /// Only start downloads between these hours (local time, 0 to 23). Equal means any time.
    pub hours_from: u8,
    pub hours_to: u8,
    /// While a game is running: "carry" on, "hold" new downloads, or "pause" all of them.
    pub while_playing: String,
    /// A second speed limit for some hours of the day (bytes a second, 0 = off).
    pub timed_limit: u64,
    pub timed_from: u8,
    pub timed_to: u8,
    /// Open in the tray instead of a window when started.
    pub start_hidden: bool,
    /// Carry on with unfinished downloads when the app starts ("yes"), or keep them paused ("no").
    pub resume_on_start: String,
    /// Games whose mods are always copied in, never hard-linked (for tools that change files in
    /// place, which would change the library's copy too).
    pub copy_games: Vec<String>,
    /// Games whose mods are kept under another folder than the library: game → folder (which
    /// gets a `<game>` folder inside). Changed only by moving them (`library_move_game`).
    pub game_libraries: std::collections::BTreeMap<String, String>,
    /// Ask GitHub once a day whether a newer Zom's Nexus is out (not in the Flatpak).
    pub check_app_updates: bool,
    /// Snapshot a game's saves by itself before each Play (the last 10 are kept).
    pub snapshot_before_play: bool,
    /// Optional AI help: which service and model (the key is kept apart, in `ai-key`).
    pub ai: Option<crate::ai::AiSettings>,
    /// On battery, new downloads wait below this charge (percent; 0 = never).
    pub battery_floor: u8,
    /// On battery, finished downloads are unpacked later (when a profile needs them, or from
    /// the Library) instead of right away.
    pub unpack_on_power: bool,
    /// Activity kinds not to note ("played", "updates"...).
    pub activity_off: Vec<String>,
    /// Which defaults this file was written with; missing means the first ones.
    #[serde(default)]
    pub version: u32,
}

/// The settings version written now. 2: unpacking after download became the default.
pub const VERSION: u32 = 2;

pub const MAX_CONCURRENT: u32 = 8;

impl Default for Settings {
    fn default() -> Self {
        Settings {
            library_dir: default_library().to_string_lossy().into_owned(),
            mode: Mode::Auto,
            max_concurrent: 3,
            extract: true,
            keep_archives: true,
            auto_retry: true,
            speed_limit: 0,
            start_with_computer: false,
            key_in_keyring: false,
            verify_downloads: true,
            adult_content: "blur".into(),
            request_saver: false,
            games: vec![],
            retry_later: true,
            hours_from: 0,
            hours_to: 0,
            while_playing: "carry".into(),
            timed_limit: 0,
            timed_from: 0,
            timed_to: 0,
            start_hidden: false,
            resume_on_start: "yes".into(),
            activity_off: vec![],
            copy_games: vec![],
            game_libraries: Default::default(),
            ai: None,
            snapshot_before_play: false,
            battery_floor: 0,
            unpack_on_power: false,
            check_app_updates: true,
            version: VERSION,
        }
    }
}

/// ~/Games/Nexus Mods, or the data folder when there's no home folder.
fn default_library() -> PathBuf {
    match std::env::var_os("HOME") {
        Some(h) => PathBuf::from(h).join("Games").join("Nexus Mods"),
        None => PathBuf::from("Nexus Mods"),
    }
}

impl Settings {
    pub fn clamp(mut self) -> Self {
        self.max_concurrent = self.max_concurrent.clamp(1, MAX_CONCURRENT);
        if !matches!(self.adult_content.as_str(), "show" | "blur" | "hide") {
            self.adult_content = "blur".into();
        }
        for h in [&mut self.hours_from, &mut self.hours_to, &mut self.timed_from, &mut self.timed_to] {
            *h = (*h).min(23);
        }
        if !matches!(self.while_playing.as_str(), "carry" | "hold" | "pause") {
            self.while_playing = "carry".into();
        }
        if !matches!(self.resume_on_start.as_str(), "yes" | "no") {
            self.resume_on_start = "yes".into();
        }
        self.activity_off.truncate(20);
        if self.library_dir.trim().is_empty() {
            self.library_dir = default_library().to_string_lossy().into_owned();
        }
        self
    }

    /// Whether `hour` is inside from..to (wrapping past midnight). Equal ends mean always.
    pub fn in_hours(hour: u8, from: u8, to: u8) -> bool {
        from == to || if from < to { hour >= from && hour < to } else { hour >= from || hour < to }
    }

    /// Whether downloads may start at `hour` (local).
    pub fn may_start(&self, hour: u8) -> bool {
        Self::in_hours(hour, self.hours_from, self.hours_to)
    }

    /// The speed limit at `hour` (local): the timed one inside its hours, else the usual one.
    pub fn limit_at(&self, hour: u8) -> u64 {
        if self.timed_limit > 0 && self.timed_from != self.timed_to && Self::in_hours(hour, self.timed_from, self.timed_to) {
            self.timed_limit
        } else {
            self.speed_limit
        }
    }

    /// How many downloads may run at the same time.
    pub fn slots(&self, premium: bool) -> usize {
        match self.mode {
            Mode::Sequential => 1,
            Mode::Auto if !premium => 1,
            Mode::Concurrent | Mode::Auto => self.max_concurrent.clamp(1, MAX_CONCURRENT) as usize,
        }
    }

    pub fn library(&self) -> PathBuf {
        PathBuf::from(&self.library_dir)
    }
}

pub fn load(dir: &Path) -> Settings {
    let mut s = fs::read_to_string(dir.join("settings.json")).ok().and_then(|s| serde_json::from_str::<Settings>(&s).ok()).unwrap_or_default();
    upgrade(&mut s);
    s.clamp()
}

/// Bring settings written with older defaults up to date.
fn upgrade(s: &mut Settings) {
    if s.version < 2 {
        // Unpacking used to be off at first; mods are easier to use unpacked.
        s.extract = true;
    }
    s.version = VERSION;
}

pub fn save(dir: &Path, s: &Settings) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?;
    write_atomic(&dir.join("settings.json"), &json)
}

pub fn load_key(dir: &Path) -> Option<String> {
    keyring_get().or_else(|| fs::read_to_string(dir.join("api-key")).ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty()))
}

const SECRET: [&str; 4] = ["application", "zoms-nexus", "kind", "api-key"];

/// The key from the desktop keyring, when one is there (secret-tool installed, key stored).
fn keyring_get() -> Option<String> {
    if !crate::system::has_program("secret-tool") {
        return None;
    }
    let out = std::process::Command::new("secret-tool").arg("lookup").args(SECRET).output().ok()?;
    let k = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !k.is_empty()).then_some(k)
}

/// Put the key in the desktop keyring, or take it out (None).
pub fn keyring_set(key: Option<&str>) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    if !crate::system::has_program("secret-tool") {
        return Err("The keyring needs secret-tool (the libsecret package). Settings > This computer has the command to install it.".into());
    }
    match key {
        None => {
            let _ = Command::new("secret-tool").arg("clear").args(SECRET).status();
            Ok(())
        }
        Some(k) => {
            let mut child = Command::new("secret-tool")
                .args(["store", "--label=Zom's Nexus: Nexus Mods API key"])
                .args(SECRET)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            child.stdin.take().ok_or("Couldn't talk to secret-tool.")?.write_all(k.as_bytes()).map_err(|e| e.to_string())?;
            let out = child.wait_with_output().map_err(|e| e.to_string())?;
            if out.status.success() {
                Ok(())
            } else {
                Err(format!("The keyring didn't take the key ({}). Is it unlocked?", String::from_utf8_lossy(&out.stderr).trim()))
            }
        }
    }
}

/// Store the key where the settings say: the keyring (and no file), or the file (and not the
/// keyring). None forgets it in both.
pub fn store_key(dir: &Path, key: Option<&str>, keyring: bool) -> Result<(), String> {
    match (key, keyring) {
        (Some(k), true) => {
            keyring_set(Some(k))?;
            save_key(dir, None)
        }
        (k, _) => {
            if crate::system::has_program("secret-tool") {
                let _ = keyring_set(None);
            }
            save_key(dir, k)
        }
    }
}

pub fn save_key(dir: &Path, key: Option<&str>) -> Result<(), String> {
    let path = dir.join("api-key");
    match key {
        None => match fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        },
        Some(k) => {
            use std::io::Write;
            #[cfg(unix)]
            use std::os::unix::fs::OpenOptionsExt;
            let mut o = fs::OpenOptions::new();
            o.write(true).create(true).truncate(true);
            #[cfg(unix)]
            o.mode(0o600);
            let mut f = o.open(&path).map_err(|e| e.to_string())?;
            f.write_all(k.as_bytes()).map_err(|e| e.to_string())
        }
    }
}

/// Write by way of a temporary file, so a crash or power cut never leaves half a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    // Each save gets its own temporary file, so two saves never trip over each other's.
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("saving-{}-{n}", std::process::id()));
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Free space on the drive that holds `path` (or the nearest folder above it that exists).
pub fn free_space(path: &Path) -> Option<u64> {
    disk(path).map(|(free, _)| free)
}

/// Free space and size of the drive that holds `path` (or the nearest folder above it that
/// exists), in bytes.
pub fn disk(path: &Path) -> Option<(u64, u64)> {
    let mut p = path.to_path_buf();
    while !p.exists() {
        p = p.parent()?.to_path_buf();
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
        let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: c is a valid C string and st is a properly sized, writable statvfs.
        if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
            return None;
        }
        let unit = st.f_frsize as u64;
        Some((st.f_bavail as u64 * unit, st.f_blocks as u64 * unit))
    }
    #[cfg(not(unix))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_unpack_after_download() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("settings.json"), r#"{"libraryDir":"/tmp/x","extract":false}"#).unwrap();
        let s = load(dir.path());
        assert!(s.extract);
        assert_eq!(s.version, VERSION);
        // Once saved with the new version, turning it off sticks.
        save(dir.path(), &Settings { extract: false, ..s }).unwrap();
        assert!(!load(dir.path()).extract);
    }
}
