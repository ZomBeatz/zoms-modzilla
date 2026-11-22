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
    /// Filled in when the library is read, never saved.
    #[serde(default, skip_deserializing)]
    pub folder: String,
}

/// A name that's safe as one folder or file name: no slashes, no leading dots, not too long.
pub fn safe_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if c == '/' || c == '\\' || c == '\0' || c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|') { ' ' } else { c })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = cleaned.trim_start_matches(['.', ' ']).trim();
    let mut out: String = trimmed.chars().take(80).collect();
    out = out.trim_end_matches(['.', ' ']).to_string();
    if out.is_empty() { "mod".into() } else { out }
}

/// A game's Nexus Mods name is already safe, but check before it becomes a folder.
fn game_dir(lib: &Path, game: &str) -> Result<PathBuf, String> {
    if game.is_empty() || !game.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("That isn't a game name Nexus Mods uses.".into());
    }
    Ok(lib.join(game))
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

fn write_meta(dir: &Path, m: &StoredMod) -> Result<(), String> {
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
        local: false,
        folder: String::new(),
    });
    meta.name = info.name.clone();
    meta.author = info.author.clone();
    meta.version = info.version.clone();
    meta.summary = info.summary.clone();
    meta.picture_url = info.picture_url.clone();

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
        unpacked: old.and_then(|o| o.unpacked),
        uploaded: file.uploaded,
        verified: None,
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
    let mut out = vec![];
    let Ok(games) = fs::read_dir(lib) else { return out };
    for g in games.flatten() {
        if g.file_name().to_string_lossy().starts_with('.') || !g.path().is_dir() {
            continue;
        }
        let Ok(mods) = fs::read_dir(g.path()) else { continue };
        for m in mods.flatten() {
            if let Some(meta) = read_meta(&m.path()) {
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
    }
    if meta.files.is_empty() {
        return fs::remove_dir_all(&dir).map_err(|e| format!("Couldn't delete {}: {e}", dir.display()));
    }
    write_meta(&dir, &meta)
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
    let name = safe_name(&stem);
    let dest = dir.join(&name);
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
        files: vec![StoredFile { file_id: id, name: file.clone(), file_name: file, version: None, category: None, size, downloaded_at: now, archive: Some(archive), unpacked: None, uploaded: None, verified: None }],
        local: true,
        folder: String::new(),
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
        std::io::copy(&mut entry, &mut w).map_err(|e| format!("Couldn't unpack {}: {e}", rel.display()))?;
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
