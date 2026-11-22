//! Profiles: a named set of mods for one game, put into the game's folder when you play.
//!
//! Putting mods in ("applying" a profile):
//! - Each mod's unpacked files are hard-linked into the game's mod folder (copied when the
//!   library is on another drive), in the profile's order; a later mod wins a clash.
//! - A game file that's in the way is moved to `<game>/.zoms-nexus-backup/` first and put back
//!   when the mods are taken out again.
//! - Everything placed is listed in `deployed/<game>.json` in the app's data folder, so taking
//!   the mods out removes exactly those files and nothing of the game's own.
//! - Bethesda games: the profile's plugins (.esp, .esm, .esl) are switched on in the Proton
//!   prefix's plugins.txt, and switched off again when the mods are taken out.
//!
//! Mods with an installer (a `fomod` folder) are put in without their installer's choices; the
//! result says which ones need a look.

use crate::library::{self, StoredMod};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::{Component, Path, PathBuf},
};

pub const BACKUP: &str = ".zoms-nexus-backup";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMod {
    pub mod_id: u64,
    pub file_id: u64,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub game: String,
    /// In load order: later ones win when two mods have the same file.
    pub mods: Vec<ProfileMod>,
    #[serde(default)]
    pub collection: Option<String>,
    /// Leave requirements to the person: don't add, switch on or sort them by themselves.
    #[serde(default)]
    pub manual_requirements: bool,
    /// Finished downloads for this game go into this profile by themselves (one per game).
    #[serde(default)]
    pub new_downloads: bool,
}

/// Where a game's mods go, under its install folder. None: not known, so ask the person.
pub fn default_mod_folder(game: &str) -> Option<&'static str> {
    Some(match game {
        "skyrimspecialedition" | "skyrim" | "fallout4" | "newvegas" | "fallout3" | "oblivion" | "starfield" => "Data",
        "morrowind" => "Data Files",
        "stardewvalley" => "Mods",
        "cyberpunk2077" | "witcher3" | "valheim" | "eldenring" | "mountandblade2bannerlord" | "kingdomcomedeliverance" | "kingdomcomedeliverance2" | "nomanssky" | "subnautica" | "palworld" | "hogwartslegacy" | "monsterhunterworld" | "monsterhunterwilds" | "darksouls3" | "sekiro" | "bladeandsorcery" | "rimworld" | "xcom2" | "citiesskylines" | "stalker2heartofchornobyl" => "",
        _ => return None,
    })
}

/// The plugins.txt folder inside a Proton prefix, and whether its lines start with "*".
pub fn plugins_txt(game: &str) -> Option<(&'static str, bool)> {
    Some(match game {
        "skyrimspecialedition" => ("Skyrim Special Edition", true),
        "fallout4" => ("Fallout4", true),
        "starfield" => ("Starfield", true),
        "skyrim" => ("Skyrim", false),
        "newvegas" => ("FalloutNV", false),
        "fallout3" => ("Fallout3", false),
        "oblivion" => ("Oblivion", false),
        _ => return None,
    })
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deployed {
    pub profile_id: Option<String>,
    pub profile_name: Option<String>,
    /// Files placed, relative to the game folder.
    pub files: Vec<String>,
    /// Game files moved into the backup folder, relative to the game folder.
    pub backups: Vec<String>,
    /// Folders made for the mods (removed again when empty).
    pub dirs: Vec<String>,
    /// Plugins switched on in plugins.txt.
    pub plugins: Vec<String>,
    pub plugins_file: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub files: usize,
    pub mods: usize,
    pub backed_up: usize,
    pub plugins: usize,
    /// Mods with an installer whose choices weren't made.
    pub installers: Vec<String>,
    /// Mods in the profile that aren't in the library (not downloaded yet).
    pub missing: Vec<String>,
    /// Files more than one mod has (the later mod's copy is used).
    pub clashes: usize,
}

fn manifest_path(data: &Path, game: &str) -> PathBuf {
    data.join("deployed").join(format!("{}.json", library::safe_name(game)))
}

pub fn deployed(data: &Path, game: &str) -> Deployed {
    fs::read_to_string(manifest_path(data, game)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_manifest(data: &Path, game: &str, d: &Deployed) -> Result<(), String> {
    let p = manifest_path(data, game);
    fs::create_dir_all(p.parent().unwrap()).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&p, &serde_json::to_vec_pretty(d).map_err(|e| e.to_string())?)
}

/// A relative path from the manifest, checked to stay inside the game folder.
fn inside(root: &Path, rel: &str) -> Option<PathBuf> {
    let p = Path::new(rel);
    if p.components().all(|c| matches!(c, Component::Normal(_))) {
        Some(root.join(p))
    } else {
        None
    }
}

/// Remove `dir` and the folders in it, as far as they're empty.
fn prune_empty(dir: &Path) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                prune_empty(&e.path());
            }
        }
    }
    let _ = fs::remove_dir(dir);
}

/// Every file under `dir`, as paths relative to it. Links are skipped.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let Ok(rd) = fs::read_dir(dir.join(&rel)) else { continue };
        for e in rd.flatten() {
            let Ok(t) = e.file_type() else { continue };
            let r = rel.join(e.file_name());
            if t.is_dir() {
                stack.push(r);
            } else if t.is_file() {
                out.push(r);
            }
        }
    }
    out.sort();
    out
}

/// The folder whose contents go into the mod folder: skip a single wrapping folder
/// ("SkyUI_5_2/..."), and say whether the mod brings the mod folder itself ("Data/...").
fn content_root(unpacked: &Path, mod_folder: &str) -> (PathBuf, bool) {
    let target = Path::new(mod_folder).file_name().map(|n| n.to_string_lossy().to_lowercase());
    let mut src = unpacked.to_path_buf();
    for _ in 0..3 {
        let entries: Vec<_> = fs::read_dir(&src).map(|r| r.flatten().collect()).unwrap_or_default();
        let has_target = entries.iter().any(|e| Some(e.file_name().to_string_lossy().to_lowercase()) == target);
        if has_target {
            return (src, true);
        }
        match entries.as_slice() {
            [only] if only.path().is_dir() && !matches!(only.file_name().to_string_lossy().to_lowercase().as_str(), "fomod" | "meshes" | "textures" | "scripts" | "interface" | "sound" | "seq" | "skse" | "archive" | "bin" | "r6" | "red4ext" | "bepinex" | "content") => {
                src = only.path();
            }
            _ => break,
        }
    }
    (src, false)
}

/// A library file's unpacked folder, unpacking it first if it hasn't been.
pub fn unpacked_dir(m: &StoredMod, f: &library::StoredFile) -> Result<PathBuf, String> {
    let dir = PathBuf::from(&m.folder);
    match &f.unpacked {
        Some(u) => Ok(dir.join(library::safe_name(u))),
        None => {
            let archive = f.archive.as_ref().ok_or_else(|| format!("“{}” has no files to put in. Download it again.", m.name))?;
            let name = library::unpack(&dir.join(library::safe_name(archive))).map_err(|e| format!("“{}”: {e}", m.name))?;
            library::set_unpacked(&dir, f.file_id, Some(name.clone()), false)?;
            Ok(dir.join(name))
        }
    }
}

/// Where each of a mod's files goes, relative to the game folder ("Data/textures/sky.dds"),
/// and where it comes from. A mod's installer folder (fomod) isn't put in.
pub fn entries(unpacked: &Path, mod_folder: &str) -> Vec<(String, PathBuf)> {
    let (src, brings_folder) = content_root(unpacked, mod_folder);
    // "Data/..." in the mod goes to <game>/Data/...; anything else goes into the mod folder.
    let base = if brings_folder { Path::new(mod_folder).parent().map(Path::to_path_buf).unwrap_or_default() } else { PathBuf::from(mod_folder) };
    walk(&src)
        .into_iter()
        .filter(|rel| !rel.components().next().map(|c| c.as_os_str().eq_ignore_ascii_case("fomod")).unwrap_or(false))
        .map(|rel| (base.join(&rel).to_string_lossy().replace('\\', "/"), src.join(rel)))
        .collect()
}

/// Take out whatever mods are in the game now, putting its own files back.
pub fn clear(data: &Path, game: &str, root: &Path) -> Result<(), String> {
    let d = deployed(data, game);
    for rel in &d.files {
        if let Some(p) = inside(root, rel) {
            let _ = fs::remove_file(p);
        }
    }
    let backup = root.join(BACKUP);
    for rel in &d.backups {
        if let (Some(from), Some(to)) = (inside(&backup, rel), inside(root, rel)) {
            if let Some(parent) = to.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::rename(&from, &to).map_err(|e| format!("Couldn't put the game's own {rel} back: {e}. It's still in {}.", backup.display()))?;
        }
    }
    let mut dirs: Vec<&String> = d.dirs.iter().collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
    for rel in dirs {
        if let Some(p) = inside(root, rel) {
            let _ = fs::remove_dir(p); // Only when empty.
        }
    }
    prune_empty(&backup);
    if let (Some(file), false) = (&d.plugins_file, d.plugins.is_empty()) {
        let _ = set_plugins(Path::new(file), &d.plugins, &[], false);
    }
    save_manifest(data, game, &Deployed::default())
}

/// Put plugins.txt's lines for `old` out and `new` in, keeping every other line as it was.
fn set_plugins(file: &Path, old: &[String], new: &[String], star: bool) -> Result<(), String> {
    let text = fs::read_to_string(file).unwrap_or_default();
    let ours: BTreeSet<String> = old.iter().chain(new.iter()).map(|p| p.to_lowercase()).collect();
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| {
            let name = l.trim().trim_start_matches('*').to_lowercase();
            !ours.contains(&name)
        })
        .map(String::from)
        .collect();
    for p in new {
        lines.push(if star { format!("*{p}") } else { p.clone() });
    }
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    // plugins.txt is read by a Windows game: CRLF line ends.
    fs::write(file, lines.join("\r\n") + "\r\n").map_err(|e| e.to_string())
}

pub struct Target<'a> {
    pub game: &'a str,
    pub root: &'a Path,
    pub mod_folder: &'a str,
    pub prefix: Option<&'a Path>,
}

/// Apply a profile: take out what's there, then put this profile's mods in.
/// Whether two folders are on the same drive (so files can be hard-linked between them).
fn same_drive(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev(),
        _ => true,
    }
}

pub fn apply(data: &Path, t: &Target, profile: &Profile, mods: &[StoredMod]) -> Result<Applied, String> {
    if !t.root.is_dir() {
        return Err(format!("The game's folder {} isn't there. Check it on the Games page.", t.root.display()));
    }
    // Unpack everything first (each unpack checks its own space), then make sure the game's
    // drive has room: files are linked when the library is on the same drive, which takes no
    // space, and copied when it isn't.
    let mut ready: Vec<(&StoredMod, PathBuf)> = vec![];
    let mut out = Applied::default();
    for pm in profile.mods.iter().filter(|m| m.enabled) {
        let Some(m) = mods.iter().find(|m| m.game == t.game && m.mod_id == pm.mod_id) else {
            out.missing.push(format!("Mod {}", pm.mod_id));
            continue;
        };
        let Some(f) = m.files.iter().find(|f| f.file_id == pm.file_id) else {
            out.missing.push(m.name.clone());
            continue;
        };
        ready.push((m, unpacked_dir(m, f)?));
    }
    if let Some((_, first)) = ready.first() {
        if !same_drive(first, t.root) {
            let need: u64 = ready.iter().flat_map(|(_, u)| entries(u, t.mod_folder)).map(|(_, from)| fs::metadata(from).map(|m| m.len()).unwrap_or(0)).sum();
            library::ensure_space(t.root, need, "copying the mods into the game (the library is on another drive)")?;
        }
    }
    clear(data, t.game, t.root)?;
    let mut d = Deployed { profile_id: Some(profile.id.clone()), profile_name: Some(profile.name.clone()), ..Default::default() };
    let mut placed: HashMap<String, usize> = HashMap::new(); // lowercased path -> index in d.files
    let mut made_dirs: BTreeSet<String> = BTreeSet::new();
    let backup = t.root.join(BACKUP);

    for (m, unpacked) in &ready {
        let unpacked = unpacked.as_path();
        if unpacked.join("fomod").is_dir() || walk(&unpacked).iter().any(|p| p.components().next().map(|c| c.as_os_str().eq_ignore_ascii_case("fomod")).unwrap_or(false)) {
            out.installers.push(m.name.clone());
        }
        out.mods += 1;
        for (rel_s, from) in entries(&unpacked, t.mod_folder) {
            let rel_game = PathBuf::from(&rel_s);
            let Some(to) = inside(t.root, &rel_s) else { continue };
            let key = rel_s.to_lowercase();
            if placed.contains_key(&key) {
                out.clashes += 1;
                let _ = fs::remove_file(&to);
            } else if to.exists() || to.is_symlink() {
                // The game's own file: keep it safe.
                let b = backup.join(&rel_game);
                fs::create_dir_all(b.parent().unwrap()).map_err(|e| e.to_string())?;
                fs::rename(&to, &b).map_err(|e| format!("Couldn't move the game's {rel_s} aside: {e}"))?;
                d.backups.push(rel_s.clone());
                out.backed_up += 1;
            }
            // Note each folder made, from the top down.
            let mut acc = PathBuf::new();
            for c in rel_game.parent().map(|p| p.components().collect::<Vec<_>>()).unwrap_or_default() {
                acc.push(c);
                if !t.root.join(&acc).exists() {
                    made_dirs.insert(acc.to_string_lossy().replace('\\', "/"));
                }
            }
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("Couldn't make {}: {e}", parent.display()))?;
            }
            if fs::hard_link(&from, &to).is_err() {
                fs::copy(&from, &to).map_err(|e| format!("Couldn't put {rel_s} in the game folder: {e}"))?;
            }
            if !placed.contains_key(&key) {
                placed.insert(key, d.files.len());
                d.files.push(rel_s);
            }
            // Save as we go, so a failure halfway can still be undone.
            if d.files.len() % 500 == 0 {
                d.dirs = made_dirs.iter().cloned().collect();
                save_manifest(data, t.game, &d)?;
            }
        }
    }
    d.dirs = made_dirs.into_iter().collect();
    out.files = d.files.len();

    // Bethesda plugins, in the profile's order.
    if let (Some((folder, star)), Some(prefix)) = (plugins_txt(t.game), t.prefix) {
        let data_dir = t.mod_folder.to_lowercase();
        let plugins: Vec<String> = d
            .files
            .iter()
            .filter_map(|f| {
                let (dir, name) = f.rsplit_once('/')?;
                let ext = name.rsplit('.').next()?.to_lowercase();
                (dir.to_lowercase() == data_dir && matches!(ext.as_str(), "esp" | "esm" | "esl")).then(|| name.to_string())
            })
            .collect();
        if !plugins.is_empty() {
            let file = prefix.join("drive_c/users/steamuser/AppData/Local").join(folder).join("plugins.txt");
            set_plugins(&file, &[], &plugins, star)?;
            out.plugins = plugins.len();
            d.plugins = plugins;
            d.plugins_file = Some(file.to_string_lossy().into_owned());
        }
    }
    save_manifest(data, t.game, &d)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::StoredFile;

    fn lib_mod(lib: &Path, id: u64, files: &[(&str, &str)]) -> StoredMod {
        let folder = lib.join(format!("skyrimspecialedition/{id} - Mod {id}"));
        let unpacked = folder.join("unpacked");
        for (rel, body) in files {
            let p = unpacked.join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }
        StoredMod {
            game: "skyrimspecialedition".into(),
            mod_id: id,
            name: format!("Mod {id}"),
            author: None,
            version: None,
            summary: None,
            picture_url: None,
            local: false,
            folder: folder.to_string_lossy().into_owned(),
            files: vec![StoredFile { file_id: id * 10, name: "Main".into(), file_name: "m.7z".into(), version: None, category: None, size: 1, downloaded_at: 0, archive: None, unpacked: Some("unpacked".into()), uploaded: None, verified: None }],
        }
    }

    fn profile(ids: &[u64]) -> Profile {
        Profile { id: "p1".into(), name: "Winter".into(), game: "skyrimspecialedition".into(), mods: ids.iter().map(|i| ProfileMod { mod_id: *i, file_id: i * 10, enabled: true }).collect(), collection: None, manual_requirements: false, new_downloads: false }
    }

    #[test]
    fn applies_and_clears_without_losing_game_files() {
        let tmp = tempfile::tempdir().unwrap();
        let (data, lib, game, pfx) = (tmp.path().join("data"), tmp.path().join("lib"), tmp.path().join("game"), tmp.path().join("pfx"));
        fs::create_dir_all(game.join("Data/textures")).unwrap();
        fs::write(game.join("Data/textures/sky.dds"), "original").unwrap();
        fs::write(game.join("SkyrimSE.exe"), "exe").unwrap();
        let plugins = pfx.join("drive_c/users/steamuser/AppData/Local/Skyrim Special Edition/plugins.txt");
        fs::create_dir_all(plugins.parent().unwrap()).unwrap();
        fs::write(&plugins, "*Existing.esp\r\n").unwrap();

        // Mod 1 wraps its files in a folder and brings Data/; mod 2 has loose files and wins the clash.
        let m1 = lib_mod(&lib, 1, &[("Wrapper/Data/textures/sky.dds", "mod1"), ("Wrapper/Data/Lanterns.esp", "p")]);
        let m2 = lib_mod(&lib, 2, &[("textures/sky.dds", "mod2"), ("meshes/new.nif", "n"), ("fomod/ModuleConfig.xml", "x")]);
        let t = Target { game: "skyrimspecialedition", root: &game, mod_folder: "Data", prefix: Some(&pfx) };
        let r = apply(&data, &t, &profile(&[1, 2, 3]), &[m1, m2]).unwrap();
        assert_eq!(fs::read_to_string(game.join("Data/textures/sky.dds")).unwrap(), "mod2");
        assert_eq!(fs::read_to_string(game.join(BACKUP).join("Data/textures/sky.dds")).unwrap(), "original");
        assert!(game.join("Data/meshes/new.nif").exists());
        assert!(!game.join("Data/fomod").exists());
        assert_eq!((r.mods, r.backed_up, r.clashes, r.plugins), (2, 1, 1, 1));
        assert_eq!(r.installers, vec!["Mod 2".to_string()]);
        assert_eq!(r.missing, vec!["Mod 3".to_string()]);
        assert_eq!(fs::read_to_string(&plugins).unwrap(), "*Existing.esp\r\n*Lanterns.esp\r\n");

        clear(&data, "skyrimspecialedition", &game).unwrap();
        assert_eq!(fs::read_to_string(game.join("Data/textures/sky.dds")).unwrap(), "original");
        assert!(!game.join("Data/meshes").exists(), "folders made for mods are removed");
        assert!(!game.join("Data/Lanterns.esp").exists());
        assert!(!game.join(BACKUP).exists());
        assert!(game.join("SkyrimSE.exe").exists());
        assert_eq!(fs::read_to_string(&plugins).unwrap(), "*Existing.esp\r\n");
        // The library's own copies are untouched.
        assert_eq!(fs::read_to_string(lib.join("skyrimspecialedition/2 - Mod 2/unpacked/textures/sky.dds")).unwrap(), "mod2");
    }

    #[test]
    fn switching_profiles_takes_the_old_one_out() {
        let tmp = tempfile::tempdir().unwrap();
        let (data, lib, game) = (tmp.path().join("data"), tmp.path().join("lib"), tmp.path().join("game"));
        fs::create_dir_all(&game).unwrap();
        let m1 = lib_mod(&lib, 1, &[("a.txt", "1")]);
        let m2 = lib_mod(&lib, 2, &[("b.txt", "2")]);
        let t = Target { game: "skyrimspecialedition", root: &game, mod_folder: "Data", prefix: None };
        apply(&data, &t, &profile(&[1]), &[m1.clone(), m2.clone()]).unwrap();
        assert!(game.join("Data/a.txt").exists());
        apply(&data, &t, &profile(&[2]), &[m1, m2]).unwrap();
        assert!(!game.join("Data/a.txt").exists());
        assert!(game.join("Data/b.txt").exists());
        assert_eq!(deployed(&data, "skyrimspecialedition").profile_name.as_deref(), Some("Winter"));
    }

    #[test]
    fn manifest_paths_cannot_leave_the_game_folder() {
        assert!(inside(Path::new("/g"), "../etc/passwd").is_none());
        assert!(inside(Path::new("/g"), "/etc/passwd").is_none());
        assert!(inside(Path::new("/g"), "Data/a.esp").is_some());
    }
}
