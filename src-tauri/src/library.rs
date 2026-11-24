//! The library: every downloaded mod, kept on disk in a layout you can browse by hand.
//!
//! ```text
//! <library>/
//!   skyrimspecialedition/
//!     266 - SkyUI/
//!       nexus.json             what the mod is and which files are here
//!       SkyUI_5_2_SE-12604-5-2SE.7z
//!       SkyUI_5_2_SE-12604-5-2SE/      unpacked, when Settings > Unpack is on
//!   .incoming/                 downloads in progress (<id>.part)
//! ```

use crate::nexus::{FileInfo, ModInfo};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const INCOMING: &str = ".incoming";

/// Changes to nexus.json files take turns: two files of one mod finishing at the same moment
/// would otherwise each write the list without the other's file.
static WRITING: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    WRITING.lock().unwrap_or_else(|e| e.into_inner())
}
const META: &str = "nexus.json";

/// Mods added from your own files get numbers from here up, far above any Nexus Mods number.
pub const LOCAL_IDS: u64 = 1_000_000_000_000;

pub fn is_local(mod_id: u64) -> bool {
    mod_id >= LOCAL_IDS
}

/// Room to leave on a drive beyond what's needed, so it never fills to the last byte.
pub const SPARE: u64 = 64 * 1024 * 1024;

/// An error when `dir`'s drive hasn't room for `need` bytes (plus a little spare).
pub fn ensure_space(dir: &Path, need: u64, what: &str) -> Result<(), String> {
    match crate::settings::free_space(dir) {
        Some(free) if free < need.saturating_add(SPARE) => Err(format!(
            "There isn't enough space for {what}: it needs {}, and the drive has {} free. Make room, then try again.",
            crate::queue::size_words(need),
            crate::queue::size_words(free)
        )),
        _ => Ok(()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredFile {
    pub file_id: u64,
    pub name: String,
    pub file_name: String,
    pub version: Option<String>,
    pub category: Option<String>,
    pub size: u64,
    pub downloaded_at: u64,
    /// The archive's name in the mod folder, while it's kept.
    pub archive: Option<String>,
    /// The unpacked folder's name in the mod folder.
    pub unpacked: Option<String>,
    /// When the file was uploaded to Nexus Mods (Unix time), for files downloaded since this
    /// was kept.
    #[serde(default)]
    pub uploaded: Option<u64>,
    /// Whether the download matched Nexus Mods' checksum for this file (None: not checked).
    #[serde(default)]
    pub verified: Option<bool>,
    /// The choices made in the file's installer (FOMOD) and the folder they built.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<crate::fomod::Installed>,
    /// Whether the unpacked file has an installer (FOMOD). Filled in when the library is read.
    #[serde(default, skip_deserializing)]
    pub has_installer: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredMod {
    pub game: String,
    pub mod_id: u64,
    pub name: String,
    pub author: Option<String>,
    pub version: Option<String>,
    pub summary: Option<String>,
    pub picture_url: Option<String>,
    pub files: Vec<StoredFile>,
    /// Added from a file on this computer, not downloaded from Nexus Mods. Its number is made
    /// up (at or above LOCAL_IDS), so it's never looked up on Nexus Mods or put in a mod pack.
    #[serde(default)]
    pub local: bool,
    /// Marked as adult content on Nexus Mods (when it was downloaded).
    #[serde(default)]
    pub adult: bool,
    /// Where it came from when not Nexus Mods: "thunderstore:Owner-Name".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Filled in when the library is read, never saved.
    #[serde(default, skip_deserializing)]
    pub folder: String,
}

/// Longest name kept, in bytes (Linux allows 255 per name; this leaves room for
/// `.<name>.unpacking` and " files").
const NAME_BYTES: usize = 200;

/// A name that's safe as one folder or file name: no slashes, no leading dots, not too long.
/// A long name is shortened in the middle of its words, never at its ending (".zip" stays), so
/// a file and the folder it unpacks into never end up with the same name.
pub fn safe_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c == '/' || c == '\\' || c == '\0' || c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') { ' ' } else { c })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = cleaned.trim_start_matches(['.', ' ']).trim();
    let mut out = if trimmed.len() <= NAME_BYTES {
        trimmed.to_string()
    } else {
        let ext = trimmed.rfind('.').map(|i| &trimmed[i..]).filter(|e| e.len() <= 10 && e[1..].chars().all(|c| c.is_ascii_alphanumeric()) && e.len() > 1).unwrap_or("");
        let mut head = String::new();
        for c in trimmed.chars() {
            if head.len() + c.len_utf8() + ext.len() > NAME_BYTES {
                break;
            }
            head.push(c);
        }
        format!("{}{ext}", head.trim_end_matches(['.', ' ']))
    };
    out = out.trim_end_matches(['.', ' ']).to_string();
    if out.is_empty() { "mod".into() } else { out }
}

/// Games whose mods live under another folder than the library's (Settings › Storage): game →
/// that folder, which holds `<game>/<mod>` like the library does. Every library path goes
/// through `game_dir`, so everything follows it.
static ROOTS: std::sync::Mutex<std::collections::BTreeMap<String, PathBuf>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

pub fn set_roots(roots: &std::collections::BTreeMap<String, String>) {
    *ROOTS.lock().unwrap() = roots.iter().filter(|(_, v)| !v.trim().is_empty()).map(|(k, v)| (k.clone(), PathBuf::from(v))).collect();
}

/// The folder holding a game's `<game>` folder: its own, or the library.
pub fn root_for(lib: &Path, game: &str) -> PathBuf {
    ROOTS.lock().unwrap().get(game).cloned().unwrap_or_else(|| lib.to_path_buf())
}

/// A game's Nexus Mods name is already safe, but check before it becomes a folder.
pub fn game_dir(lib: &Path, game: &str) -> Result<PathBuf, String> {
    if game.is_empty() || !game.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("That isn't a game name Nexus Mods uses.".into());
    }
    Ok(root_for(lib, game).join(game))
}

/// Copy a folder's files and folders (links skipped) into `to`, which must not exist yet.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("Couldn't make {}: {e}", to.display()))?;
    for e in fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let Ok(t) = e.file_type() else { continue };
        let dest = to.join(e.file_name());
        if t.is_dir() {
            copy_tree(&e.path(), &dest)?;
        } else if t.is_file() {
            fs::copy(e.path(), &dest).map_err(|err| format!("Couldn't copy {}: {err}", e.path().display()))?;
        }
    }
    Ok(())
}

/// Move a game's mods from one root to another (`<from>/<game>` → `<to>/<game>`). Renamed when
/// both are on one drive; otherwise copied in full first and only then deleted, so a failure
/// leaves the original. Downloads wait their turn meanwhile. Gives the bytes moved.
pub fn move_game(from_root: &Path, to_root: &Path, game: &str) -> Result<u64, String> {
    let _turn = turn();
    if game.is_empty() || !game.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("That isn't a game name Nexus Mods uses.".into());
    }
    let (from, to) = (from_root.join(game), to_root.join(game));
    if !from.is_dir() {
        // No mods yet: the folder is made now, for the first ones.
        fs::create_dir_all(&to).map_err(|e| format!("Couldn't use {}: {e}", to_root.display()))?;
        return Ok(0);
    }
    if to.exists() && fs::read_dir(&to).map(|mut d| d.next().is_some()).unwrap_or(true) {
        return Err(format!("{} already has files in it. Pick an empty folder, or clear it first.", to.display()));
    }
    let size = dir_size(&from);
    fs::create_dir_all(to_root).map_err(|e| format!("Couldn't use {}: {e}", to_root.display()))?;
    let _ = fs::remove_dir(&to);
    if fs::rename(&from, &to).is_ok() {
        return Ok(size);
    }
    let free = crate::settings::free_space(to_root).unwrap_or(u64::MAX);
    if free < size + SPARE {
        return Err(format!("{} has {} MB free; the mods need {} MB.", to_root.display(), free / 1_000_000, size / 1_000_000));
    }
    let tmp = to_root.join(format!(".{game}.moving"));
    let _ = fs::remove_dir_all(&tmp);
    if let Err(e) = copy_tree(&from, &tmp) {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }
    fs::rename(&tmp, &to).map_err(|e| e.to_string())?;
    fs::remove_dir_all(&from).map_err(|e| format!("Copied, but couldn't delete the old folder {}: {e}", from.display()))?;
    Ok(size)
}

/// The mod's folder: an existing one for this mod number (even if the mod was renamed since), or
/// a new `<id> - <name>`.
pub fn mod_dir(lib: &Path, game: &str, mod_id: u64, name: &str) -> Result<PathBuf, String> {
    let gdir = game_dir(lib, game)?;
    let prefix = format!("{mod_id} - ");
    if let Ok(rd) = fs::read_dir(&gdir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            if (n == mod_id.to_string() || n.starts_with(&prefix)) && e.path().is_dir() {
                return Ok(e.path());
            }
        }
    }
    Ok(gdir.join(format!("{prefix}{}", safe_name(name))))
}

pub fn read_meta(dir: &Path) -> Option<StoredMod> {
    let mut m: StoredMod = serde_json::from_str(&fs::read_to_string(dir.join(META)).ok()?).ok()?;
    m.folder = dir.to_string_lossy().into_owned();
    Some(m)
}

pub fn write_meta(dir: &Path, m: &StoredMod) -> Result<(), String> {
    crate::settings::write_atomic(&dir.join(META), &serde_json::to_vec_pretty(m).map_err(|e| e.to_string())?)
}

/// A name for `want` in `dir` that doesn't clash with a different file already there:
/// "file.7z", then "file (2).7z".
fn free_name(dir: &Path, want: &str) -> String {
    if !dir.join(want).exists() {
        return want.to_string();
    }
    let (stem, ext) = match want.rfind('.') {
        Some(i) if i > 0 => (&want[..i], &want[i..]),
        _ => (want, ""),
    };
    (2..).map(|n| format!("{stem} ({n}){ext}")).find(|n| !dir.join(n).exists()).unwrap()
}

/// Move a finished download into its mod's folder and note it in nexus.json. Gives the folder
/// and the archive's path.
pub fn store(lib: &Path, part: &Path, game: &str, info: &ModInfo, file: &FileInfo, now: u64) -> Result<(PathBuf, PathBuf), String> {
    put(lib, Some(part), game, info, file, now)
}

/// A file whose download finished and was moved into its mod's folder, but the app closed
/// before noting it in nexus.json: note it now, instead of downloading it again. Nothing
/// happens unless the archive is there at exactly the size Nexus Mods gives.
pub fn adopt(lib: &Path, game: &str, info: &ModInfo, file: &FileInfo, now: u64) -> Option<(PathBuf, PathBuf)> {
    let dir = mod_dir(lib, game, info.mod_id, &info.name).ok()?;
    let archive = dir.join(safe_name(&file.file_name));
    if file.size == 0 || fs::metadata(&archive).ok()?.len() != file.size {
        return None;
    }
    put(lib, None, game, info, file, now).ok()
}

/// Note a file in its mod's nexus.json, moving it in from `part` first (or, with no `part`,
/// using the archive already in the folder).
fn put(lib: &Path, part: Option<&Path>, game: &str, info: &ModInfo, file: &FileInfo, now: u64) -> Result<(PathBuf, PathBuf), String> {
    let _turn = turn();
    let dir = mod_dir(lib, game, info.mod_id, &info.name)?;
    fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the mod's folder {}: {e}", dir.display()))?;
    let mut meta = read_meta(&dir).unwrap_or(StoredMod {
        game: game.to_string(),
        mod_id: info.mod_id,
        name: info.name.clone(),
        author: None,
        version: None,
        summary: None,
        picture_url: None,
        files: vec![],
        local: false, adult: false,
        folder: String::new(),
        source: None,
    });
    meta.name = info.name.clone();
    meta.author = info.author.clone();
    meta.version = info.version.clone();
    meta.summary = info.summary.clone();
    meta.picture_url = info.picture_url.clone();
    meta.adult = info.adult;

    // Downloading the same file again replaces the old copy.
    let old = meta.files.iter().position(|f| f.file_id == file.file_id).map(|i| meta.files.remove(i));
    let name = match part {
        Some(part) => {
            if let Some(a) = old.as_ref().and_then(|o| o.archive.as_ref()) {
                let _ = fs::remove_file(dir.join(a));
            }
            let name = free_name(&dir, &safe_name(&file.file_name));
            move_file(part, &dir.join(&name))?;
            name
        }
        None => safe_name(&file.file_name),
    };
    let dest = dir.join(&name);
    meta.files.push(StoredFile {
        file_id: file.file_id,
        name: file.name.clone(),
        file_name: file.file_name.clone(),
        version: file.version.clone(),
        category: file.category.clone(),
        size: file.size,
        downloaded_at: now,
        archive: Some(name),
        unpacked: old.as_ref().and_then(|o| o.unpacked.clone()),
        uploaded: file.uploaded,
        verified: None,
        installed: old.and_then(|o| o.installed),
        has_installer: false,
    });
    write_meta(&dir, &meta)?;
    Ok((dir, dest))
}

/// Rename when it's on the same drive, copy and delete when it isn't.
fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    fs::copy(from, to).map_err(|e| format!("Couldn't move the download into the library: {e}"))?;
    let _ = fs::remove_file(from);
    Ok(())
}

pub fn set_unpacked(dir: &Path, file_id: u64, unpacked: Option<String>, drop_archive: bool) -> Result<(), String> {
    let _turn = turn();
    let mut meta = read_meta(dir).ok_or("This mod's nexus.json is missing.")?;
    if let Some(f) = meta.files.iter_mut().find(|f| f.file_id == file_id) {
        f.unpacked = unpacked;
        if drop_archive {
            if let Some(a) = f.archive.take() {
                let _ = fs::remove_file(dir.join(a));
            }
        }
    }
    write_meta(dir, &meta)
}

/// Keep (or forget, with None) the installer choices for one file. A forgotten built folder is deleted.
pub fn set_installed(dir: &Path, file_id: u64, installed: Option<crate::fomod::Installed>) -> Result<(), String> {
    let _turn = turn();
    let mut meta = read_meta(dir).ok_or("This mod's nexus.json is missing.")?;
    let f = meta.files.iter_mut().find(|f| f.file_id == file_id).ok_or("That file isn't in the library any more.")?;
    if let Some(old) = f.installed.take() {
        if installed.as_ref().map(|i| &i.folder) != Some(&old.folder) {
            let _ = fs::remove_dir_all(dir.join(safe_name(&old.folder)));
        }
    }
    f.installed = installed;
    write_meta(dir, &meta)
}

/// How much space one file of a mod takes on disk: its archive and its unpacked folder.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileUsage {
    pub game: String,
    pub mod_id: u64,
    pub file_id: u64,
    pub archive: u64,
    pub unpacked: u64,
}

/// Bytes under a folder (links aren't followed).
pub fn dir_size(dir: &Path) -> u64 {
    let mut total = 0;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() {
                stack.push(e.path());
            } else if t.is_file() {
                total += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

/// The space each file in the library takes, measured on disk.
pub fn usage(lib: &Path) -> Vec<FileUsage> {
    list(lib)
        .into_iter()
        .flat_map(|m| {
            let dir = PathBuf::from(&m.folder);
            m.files
                .iter()
                .map(|f| FileUsage {
                    game: m.game.clone(),
                    mod_id: m.mod_id,
                    file_id: f.file_id,
                    archive: f.archive.as_ref().and_then(|a| fs::metadata(dir.join(safe_name(a))).ok()).map(|x| x.len()).unwrap_or(0),
                    unpacked: f.unpacked.as_ref().map(|u| dir_size(&dir.join(safe_name(u)))).unwrap_or(0),
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Delete a file's archive, keeping its unpacked folder (which is what profiles use).
pub fn drop_archive(lib: &Path, game: &str, mod_id: u64, file_id: u64) -> Result<u64, String> {
    let _turn = turn();
    let dir = find(lib, game, mod_id)?;
    let mut meta = read_meta(&dir).ok_or("This mod's nexus.json is missing.")?;
    let f = meta.files.iter_mut().find(|f| f.file_id == file_id).ok_or("That file isn't in the library any more.")?;
    let unpacked = f.unpacked.as_ref().map(|u| dir.join(safe_name(u))).filter(|p| p.is_dir());
    if unpacked.is_none() {
        return Err(format!("“{}” isn't unpacked, so its archive is the only copy.", f.name));
    }
    let Some(a) = f.archive.take() else { return Ok(0) };
    let path = dir.join(safe_name(&a));
    let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let _ = fs::remove_file(&path);
    write_meta(&dir, &meta)?;
    Ok(size)
}

/// The MD5 checksum of a file, as Nexus Mods writes it (lowercase hex).
pub fn md5_of(path: &Path) -> Result<String, String> {
    use md5::{Digest, Md5};
    use std::io::Read;
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut h = Md5::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Note whether a file matched Nexus Mods' checksum.
pub fn set_verified(dir: &Path, file_id: u64, ok: bool) -> Result<(), String> {
    let _turn = turn();
    let mut meta = read_meta(dir).ok_or("This mod's nexus.json is missing.")?;
    if let Some(f) = meta.files.iter_mut().find(|f| f.file_id == file_id) {
        f.verified = Some(ok);
    }
    write_meta(dir, &meta)
}

/// Every mod in the library, newest download first.
pub fn list(lib: &Path) -> Vec<StoredMod> {
    let roots = ROOTS.lock().unwrap().clone();
    list_with(lib, &roots)
}

fn list_with(lib: &Path, roots: &std::collections::BTreeMap<String, PathBuf>) -> Vec<StoredMod> {
    let mut out = vec![];
    // The library's own games (not ones kept elsewhere), then each game kept elsewhere.
    let main = fs::read_dir(lib).into_iter().flatten().flatten().filter(|g| !roots.contains_key(&*g.file_name().to_string_lossy())).map(|g| g.path());
    let elsewhere = roots.iter().map(|(game, root)| root.join(game));
    for g in main.chain(elsewhere).collect::<Vec<_>>() {
        if g.file_name().map(|n| n.to_string_lossy().starts_with('.')).unwrap_or(true) || !g.is_dir() {
            continue;
        }
        let Ok(mods) = fs::read_dir(&g) else { continue };
        for m in mods.flatten() {
            if let Some(mut meta) = read_meta(&m.path()) {
                for f in &mut meta.files {
                    f.has_installer = f.unpacked.as_ref().is_some_and(|u| crate::fomod::root(&m.path().join(safe_name(u))).is_some());
                }
                out.push(meta);
            }
        }
    }
    let latest = |m: &StoredMod| m.files.iter().map(|f| f.downloaded_at).max().unwrap_or(0);
    out.sort_by_key(|m| std::cmp::Reverse(latest(m)));
    out
}

/// The folder of a mod in the library, checked to really be one (so a command can't be pointed
/// anywhere else on the computer).
pub fn find(lib: &Path, game: &str, mod_id: u64) -> Result<PathBuf, String> {
    let gdir = game_dir(lib, game)?;
    let dir = mod_dir(lib, game, mod_id, "")?;
    if dir.parent() != Some(gdir.as_path()) || read_meta(&dir).is_none() {
        return Err("That mod isn't in the library any more.".into());
    }
    Ok(dir)
}

/// Delete one file of a mod (its archive and unpacked folder), and the mod's folder when it was
/// the last one.
pub fn remove_file(lib: &Path, game: &str, mod_id: u64, file_id: u64) -> Result<(), String> {
    let _turn = turn();
    let dir = find(lib, game, mod_id)?;
    let mut meta = read_meta(&dir).ok_or("This mod's nexus.json is missing.")?;
    if let Some(i) = meta.files.iter().position(|f| f.file_id == file_id) {
        let f = meta.files.remove(i);
        if let Some(a) = f.archive {
            let _ = fs::remove_file(dir.join(safe_name(&a)));
        }
        if let Some(u) = f.unpacked {
            let _ = fs::remove_dir_all(dir.join(safe_name(&u)));
        }
        if let Some(i) = f.installed {
            let _ = fs::remove_dir_all(dir.join(safe_name(&i.folder)));
        }
    }
    if meta.files.is_empty() {
        return fs::remove_dir_all(&dir).map_err(|e| format!("Couldn't delete {}: {e}", dir.display()));
    }
    write_meta(&dir, &meta)
}

/// Delete every mod of a game from the library (its whole folder there). Gives the bytes freed.
pub fn remove_game(lib: &Path, game: &str) -> Result<u64, String> {
    let _turn = turn();
    let dir = game_dir(lib, game)?;
    if !dir.is_dir() {
        return Ok(0);
    }
    let size = dir_size(&dir);
    fs::remove_dir_all(&dir).map_err(|e| format!("Couldn't delete {}: {e}", dir.display()))?;
    Ok(size)
}

pub fn remove_mod(lib: &Path, game: &str, mod_id: u64) -> Result<(), String> {
    let _turn = turn();
    let dir = find(lib, game, mod_id)?;
    fs::remove_dir_all(&dir).map_err(|e| format!("Couldn't delete {}: {e}", dir.display()))
}

#[derive(Debug, PartialEq)]
pub enum Kind {
    Zip,
    SevenZ,
    Rar,
    Other,
}

/// What kind of archive a file is, by its first bytes (the name can be wrong).
pub fn kind(path: &Path) -> Kind {
    use std::io::Read;
    let mut b = [0u8; 8];
    let n = fs::File::open(path).and_then(|mut f| f.read(&mut b)).unwrap_or(0);
    match &b[..n.min(8)] {
        [b'P', b'K', 3, 4, ..] | [b'P', b'K', 5, 6, ..] => Kind::Zip,
        [b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C, ..] => Kind::SevenZ,
        [b'R', b'a', b'r', b'!', ..] => Kind::Rar,
        _ => Kind::Other,
    }
}

/// Unpack an archive into a new folder named after it, next to it. Gives the folder's name.
/// Zip and 7z are built in; rar uses `unrar` or `bsdtar` when one is installed.
pub fn unpack(archive: &Path) -> Result<String, String> {
    let dir = archive.parent().ok_or("The archive has no folder.")?;
    let file = archive.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = match file.rfind('.') {
        Some(i) if i > 0 => file[..i].to_string(),
        _ => format!("{file} files"),
    };
    let mut name = safe_name(&stem);
    // Never the archive itself (an archive saved without its ending, say).
    if dir.join(&name) == archive || dir.join(&name).is_file() {
        name = safe_name(&format!("{stem} files"));
    }
    let dest = dir.join(&name);
    check_bomb(archive)?;
    let need = unpacked_size(archive).unwrap_or_else(|| fs::metadata(archive).map(|m| m.len() * 3).unwrap_or(0));
    ensure_space(dir, need, &format!("unpacking {file}"))?;
    // Unpack into a temporary folder first, so a failure never leaves half a folder behind.
    let tmp = dir.join(format!(".{name}.unpacking"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let result = match kind(archive) {
        Kind::Zip => unzip(archive, &tmp),
        Kind::SevenZ => sevenz_rust2::decompress_file(archive, &tmp).map_err(|e| format!("Couldn't unpack the 7z file: {e}")),
        Kind::Rar => unrar(archive, &tmp),
        Kind::Other => Err("This file isn't a zip, 7z or rar archive, so it's kept as it is.".into()),
    };
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let _ = fs::remove_dir_all(&dest);
    fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(name)
}

/// Most files one archive may hold, and how much bigger than itself it may unpack to (past
/// 4 GB). Real mods stay far below both; an archive past them is refused, not unpacked.
const MAX_FILES: usize = 250_000;
const MAX_RATIO: u64 = 100;

fn entry_count(archive: &Path) -> Option<usize> {
    match kind(archive) {
        Kind::Zip => Some(zip::ZipArchive::new(fs::File::open(archive).ok()?).ok()?.len()),
        Kind::SevenZ => Some(sevenz_rust2::Archive::open(archive).ok()?.files.len()),
        _ => None,
    }
}

/// Refuse archives that would unpack into far more than any mod needs (a "zip bomb").
pub fn check_bomb(archive: &Path) -> Result<(), String> {
    let file = archive.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if entry_count(archive).is_some_and(|n| n > MAX_FILES) {
        return Err(format!("{file} holds more than {MAX_FILES} files, far more than a mod has, so it wasn't unpacked."));
    }
    let packed = fs::metadata(archive).map(|m| m.len()).unwrap_or(0).max(1);
    if let Some(u) = unpacked_size(archive) {
        if u > 4_000_000_000 && u / packed > MAX_RATIO {
            return Err(format!("{file} says it unpacks to {} GB from {} MB, which no mod does, so it wasn't unpacked.", u / 1_000_000_000, packed / 1_000_000));
        }
    }
    Ok(())
}

/// How big an archive's files are once unpacked, when the archive says (zip and 7z do).
pub fn unpacked_size(archive: &Path) -> Option<u64> {
    match kind(archive) {
        Kind::Zip => {
            let mut z = zip::ZipArchive::new(fs::File::open(archive).ok()?).ok()?;
            let mut total = 0u64;
            for i in 0..z.len() {
                total = total.saturating_add(z.by_index_raw(i).ok()?.size());
            }
            Some(total)
        }
        Kind::SevenZ => Some(sevenz_rust2::Archive::open(archive).ok()?.files.iter().map(|f| f.size).sum()),
        _ => None,
    }
}

/// Add a mod from an archive on this computer (a mod from another site, or your own): it's
/// copied into the library under a made-up number and used like any other mod.
pub fn import(lib: &Path, game: &str, src: &Path, now: u64) -> Result<StoredMod, String> {
    if !src.is_file() {
        return Err("Pick a zip, 7z or rar file.".into());
    }
    if kind(src) == Kind::Other {
        return Err("That file isn't a zip, 7z or rar archive. Mods have to be packed in one of those.".into());
    }
    let size = fs::metadata(src).map_err(|e| e.to_string())?.len();
    let file = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stem = file.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_else(|| file.clone());
    let stem = stem.strip_suffix(".tar").unwrap_or(&stem).to_string();
    let name = stem.replace(['_', '-'], " ").split_whitespace().collect::<Vec<_>>().join(" ");
    let gdir = game_dir(lib, game)?;
    fs::create_dir_all(&gdir).map_err(|e| e.to_string())?;
    ensure_space(&gdir, size, &format!("copying {file} into the library"))?;
    let _turn = turn();
    let mut id = LOCAL_IDS + now;
    while list_ids(&gdir).contains(&id) {
        id += 1;
    }
    let dir = gdir.join(format!("{id} - {}", safe_name(&name)));
    fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the mod's folder {}: {e}", dir.display()))?;
    let archive = safe_name(&file);
    if let Err(e) = fs::copy(src, dir.join(&archive)) {
        let _ = fs::remove_dir_all(&dir);
        return Err(format!("Couldn't copy {file} into the library: {e}"));
    }
    let mut meta = StoredMod {
        game: game.to_string(),
        mod_id: id,
        name: if name.is_empty() { "My mod".into() } else { name },
        author: None,
        version: None,
        summary: None,
        picture_url: None,
        files: vec![StoredFile { file_id: id, name: file.clone(), file_name: file, version: None, category: None, size, downloaded_at: now, archive: Some(archive), unpacked: None, uploaded: None, verified: None, installed: None, has_installer: false }],
        local: true,
        adult: false,
        folder: String::new(),
        source: None,
    };
    write_meta(&dir, &meta)?;
    meta.folder = dir.to_string_lossy().into_owned();
    Ok(meta)
}

/// Make a mod of your own from files on this computer (what a modding tool made in the game
/// folder): they're moved into the library, under `unpacked/` at the same place under the mod
/// folder. Each `(rel, from)` is the file's place under the mod folder and where it is now.
pub fn import_files(lib: &Path, game: &str, name: &str, files: &[(String, PathBuf)], now: u64) -> Result<StoredMod, String> {
    if files.is_empty() {
        return Err("There are no files to keep.".into());
    }
    let gdir = game_dir(lib, game)?;
    fs::create_dir_all(&gdir).map_err(|e| e.to_string())?;
    let _turn = turn();
    let mut id = LOCAL_IDS + now;
    while list_ids(&gdir).contains(&id) {
        id += 1;
    }
    let name = if name.trim().is_empty() { "Tool output".to_string() } else { name.trim().chars().take(120).collect() };
    let dir = gdir.join(format!("{id} - {}", safe_name(&name)));
    let unpacked = dir.join("unpacked");
    let mut size = 0;
    for (rel, from) in files {
        let rel = Path::new(rel);
        if rel.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
            continue;
        }
        let to = unpacked.join(rel);
        fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
        if fs::rename(from, &to).is_err() {
            fs::copy(from, &to).map_err(|e| format!("Couldn't keep {}: {e}", rel.display()))?;
            fs::remove_file(from).map_err(|e| e.to_string())?;
        }
        size += fs::metadata(&to).map(|m| m.len()).unwrap_or(0);
    }
    let mut meta = StoredMod {
        game: game.to_string(),
        mod_id: id,
        name: name.clone(),
        author: None,
        version: None,
        summary: Some("Made on this computer, kept as a mod of your own.".into()),
        picture_url: None,
        files: vec![StoredFile { file_id: id, name, file_name: String::new(), version: None, category: None, size, downloaded_at: now, archive: None, unpacked: Some("unpacked".into()), uploaded: None, verified: None, installed: None, has_installer: false }],
        local: true,
        adult: false,
        folder: String::new(),
        source: None,
    };
    write_meta(&dir, &meta)?;
    meta.folder = dir.to_string_lossy().into_owned();
    Ok(meta)
}

/// The mod numbers already used in a game's folder.
fn list_ids(gdir: &Path) -> Vec<u64> {
    fs::read_dir(gdir)
        .map(|rd| rd.flatten().filter_map(|e| e.file_name().to_string_lossy().split(' ').next()?.parse().ok()).collect())
        .unwrap_or_default()
}

fn unzip(archive: &Path, dest: &Path) -> Result<(), String> {
    let f = fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("Couldn't read the zip file: {e}"))?;
    for i in 0..z.len() {
        let mut entry = z.by_index(i).map_err(|e| format!("Couldn't read the zip file: {e}"))?;
        // enclosed_name refuses names that would land outside the folder ("../", "/etc/...").
        let Some(rel) = entry.enclosed_name() else {
            return Err(format!("The zip file has an unsafe path in it ({}), so it wasn't unpacked.", entry.name()));
        };
        let out = dest.join(&rel);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if entry.is_symlink() {
            continue; // Mods don't need links, and a link could point outside the folder.
        }
        if let Some(p) = out.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut w = fs::File::create(&out).map_err(|e| e.to_string())?;
        // Never more than the entry says it holds (a lying header can't fill the drive).
        let most = entry.size();
        let n = std::io::copy(&mut std::io::Read::take(&mut entry, most + 1), &mut w).map_err(|e| format!("Couldn't unpack {}: {e}", rel.display()))?;
        if n > most {
            return Err(format!("{} in the zip file is bigger than it says, so the zip wasn't unpacked.", rel.display()));
        }
    }
    Ok(())
}

fn unrar(archive: &Path, dest: &Path) -> Result<(), String> {
    use std::process::Command;
    // unrar refuses paths outside the folder by itself; bsdtar does too unless told otherwise.
    let tries: [(&str, Vec<std::ffi::OsString>); 2] = [
        ("unrar", vec!["x".into(), "-o+".into(), "-idq".into(), archive.into(), format!("{}/", dest.display()).into()]),
        ("bsdtar", vec!["-xf".into(), archive.into(), "-C".into(), dest.into()]),
    ];
    for (tool, args) in tries {
        match Command::new(tool).args(&args).output() {
            Ok(o) if o.status.success() => return Ok(()),
            Ok(o) => return Err(format!("{tool} couldn't unpack the rar file: {}", String::from_utf8_lossy(&o.stderr).trim())),
            Err(_) => continue,
        }
    }
    Err("Unpacking rar files needs unrar or bsdtar. Install one (for example the unrar or libarchive package), then try again.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn games_can_live_elsewhere_and_move() {
        let d = tempfile::tempdir().unwrap();
        let (lib, other) = (d.path().join("lib"), d.path().join("other"));
        let m = lib.join("testgameroots/5 - Five");
        fs::create_dir_all(&m).unwrap();
        fs::write(m.join(META), r#"{"game":"testgameroots","modId":5,"name":"Five","files":[]}"#).unwrap();
        assert!(list(&lib).iter().any(|x| x.game == "testgameroots"));
        assert!(move_game(&lib, &other, "testgameroots").unwrap() > 0);
        let mut roots = std::collections::BTreeMap::new();
        roots.insert("testgameroots".to_string(), other.clone());
        let found = list_with(&lib, &roots);
        assert_eq!(found.iter().filter(|x| x.game == "testgameroots").count(), 1);
        assert!(found[0].folder.starts_with(&*other.to_string_lossy()));
        // A folder that already has files is never written over.
        fs::create_dir_all(lib.join("testgameroots/x")).unwrap();
        assert!(move_game(&other, &lib, "testgameroots").is_err());
    }
    use std::io::Write;

    #[test]
    fn imports_your_own_mod_file() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("Better_Lights-v2.zip");
        {
            let mut w = zip::ZipWriter::new(fs::File::create(&src).unwrap());
            w.start_file("Data/lights.esp", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(&[7u8; 5000]).unwrap();
            w.finish().unwrap();
        }
        assert_eq!(unpacked_size(&src), Some(5000));
        let lib = tmp.path().join("lib");
        let a = import(&lib, "skyrimspecialedition", &src, 100).unwrap();
        let b = import(&lib, "skyrimspecialedition", &src, 100).unwrap();
        assert!(a.local && is_local(a.mod_id));
        assert_ne!(a.mod_id, b.mod_id, "two imports in the same second get their own numbers");
        assert_eq!(a.name, "Better Lights v2");
        assert!(find(&lib, "skyrimspecialedition", a.mod_id).is_ok());
        let dir = PathBuf::from(&a.folder);
        assert!(dir.join("Better_Lights-v2.zip").is_file());
        let name = unpack(&dir.join("Better_Lights-v2.zip")).unwrap();
        assert!(dir.join(name).join("Data/lights.esp").is_file());
        assert!(import(&lib, "skyrimspecialedition", &tmp.path().join("nope.txt"), 1).is_err());
    }

    #[test]
    fn safe_names() {
        assert_eq!(safe_name("SkyUI"), "SkyUI");
        assert_eq!(safe_name("../../etc/passwd"), "etc passwd");
        assert_eq!(safe_name("  ..hidden  "), "hidden");
        assert_eq!(safe_name("a:b*c?"), "a b c");
        assert_eq!(safe_name(""), "mod");
        assert_eq!(safe_name("..."), "mod");
        // Long names keep their ending, so a file never clashes with the folder it unpacks into.
        let long = format!("{} KRvfRxIYU.zip", "Atomiic x Hyst - Dia Angels Garb - Archive XL 33142 1 2026-08-25T18-32Z ".repeat(4));
        let short = safe_name(&long);
        assert!(short.ends_with(".zip") && short.len() <= 200, "{short}");
        assert_eq!(safe_name(&short), short, "already safe names don't change");
        let old = "Atomiic x Hyst - Dia Angels Garb - Archive XL 33142 1 2026-08-25T18-32Z KRvfRxIY";
        assert_eq!(safe_name(old), old);
    }

    fn info(id: u64, name: &str) -> ModInfo {
        ModInfo { mod_id: id, game: "skyrim".into(), name: name.into(), summary: None, version: Some("1.0".into()), author: Some("Alex".into()), picture_url: None, adult: false, endorsements: None, downloads: None, updated: None }
    }

    fn file(id: u64, name: &str) -> FileInfo {
        FileInfo { file_id: id, name: name.into(), file_name: name.into(), version: None, category: Some("MAIN".into()), size: 3, uploaded: None, is_primary: true, description: None }
    }

    #[test]
    fn adopts_a_file_moved_in_before_a_close() {
        let lib = tempfile::tempdir().unwrap();
        let f = file(70, "cool.zip");
        // Nothing there yet, or the wrong size: not adopted.
        assert!(adopt(lib.path(), "skyrim", &info(7, "Cool Mod"), &f, 1).is_none());
        let dir = lib.path().join("skyrim/7 - Cool Mod");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("cool.zip"), b"ab").unwrap();
        assert!(adopt(lib.path(), "skyrim", &info(7, "Cool Mod"), &f, 1).is_none());
        // The finished file, moved in but never noted.
        fs::write(dir.join("cool.zip"), b"abc").unwrap();
        let (d, archive) = adopt(lib.path(), "skyrim", &info(7, "Cool Mod"), &f, 1).unwrap();
        assert_eq!(d, dir);
        assert_eq!(fs::read(archive).unwrap(), b"abc");
        assert_eq!(list(lib.path())[0].files[0].file_id, 70);
    }

    #[test]
    fn checksums_like_nexus_mods() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.bin");
        fs::write(&p, b"abc").unwrap();
        assert_eq!(md5_of(&p).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn measures_space_and_drops_archives() {
        let lib = tempfile::tempdir().unwrap();
        let part = lib.path().join("a.part");
        fs::write(&part, b"abc").unwrap();
        let (dir, _) = store(lib.path(), &part, "skyrim", &info(7, "Cool Mod"), &file(70, "cool.zip"), 100).unwrap();
        // Not unpacked: the archive is the only copy, so it stays.
        assert!(drop_archive(lib.path(), "skyrim", 7, 70).is_err());
        fs::create_dir_all(dir.join("cool/Data")).unwrap();
        fs::write(dir.join("cool/Data/a.esp"), b"12345").unwrap();
        set_unpacked(&dir, 70, Some("cool".into()), false).unwrap();
        assert_eq!(usage(lib.path()), vec![FileUsage { game: "skyrim".into(), mod_id: 7, file_id: 70, archive: 3, unpacked: 5 }]);
        assert_eq!(drop_archive(lib.path(), "skyrim", 7, 70).unwrap(), 3);
        assert!(!dir.join("cool.zip").exists());
        assert_eq!(usage(lib.path())[0].archive, 0);
        assert_eq!(list(lib.path())[0].files[0].unpacked.as_deref(), Some("cool"));
    }

    #[test]
    fn stores_lists_and_removes() {
        let lib = tempfile::tempdir().unwrap();
        let part = lib.path().join("a.part");
        fs::write(&part, b"abc").unwrap();
        let (dir, path) = store(lib.path(), &part, "skyrim", &info(7, "Cool Mod"), &file(70, "cool.zip"), 100).unwrap();
        assert!(dir.ends_with("skyrim/7 - Cool Mod"));
        assert_eq!(fs::read(&path).unwrap(), b"abc");
        // Renamed on Nexus Mods: the same folder is used.
        fs::write(&part, b"xyz").unwrap();
        let (dir2, _) = store(lib.path(), &part, "skyrim", &info(7, "Renamed"), &file(71, "update.zip"), 200).unwrap();
        assert_eq!(dir, dir2);
        let all = list(lib.path());
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Renamed");
        assert_eq!(all[0].files.len(), 2);
        remove_file(lib.path(), "skyrim", 7, 70).unwrap();
        assert_eq!(list(lib.path())[0].files.len(), 1);
        remove_file(lib.path(), "skyrim", 7, 71).unwrap();
        assert!(list(lib.path()).is_empty());
        assert!(!dir.exists());
        assert!(find(lib.path(), "../x", 1).is_err());
    }

    #[test]
    fn unpacks_zip_and_refuses_escapes() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.zip");
        {
            let mut w = zip::ZipWriter::new(fs::File::create(&good).unwrap());
            w.start_file("Data/readme.txt", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"hi").unwrap();
            w.finish().unwrap();
        }
        assert_eq!(kind(&good), Kind::Zip);
        let name = unpack(&good).unwrap();
        assert_eq!(fs::read(dir.path().join(name).join("Data/readme.txt")).unwrap(), b"hi");

        let bad = dir.path().join("bad.zip");
        {
            let mut w = zip::ZipWriter::new(fs::File::create(&bad).unwrap());
            w.start_file("../escaped.txt", zip::write::SimpleFileOptions::default()).unwrap();
            w.write_all(b"no").unwrap();
            w.finish().unwrap();
        }
        assert!(unpack(&bad).is_err());
        assert!(!dir.path().join("escaped.txt").exists());
        assert!(!dir.path().join("bad").exists());
    }
}
