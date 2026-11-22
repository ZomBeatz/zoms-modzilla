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
            games: vec![],
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
        if self.library_dir.trim().is_empty() {
            self.library_dir = default_library().to_string_lossy().into_owned();
        }
        self
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
