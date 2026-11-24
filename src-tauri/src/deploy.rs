//! Profiles: a named set of mods for one game, put into the game's folder when you play.
//!
//! Putting mods in ("applying" a profile):
//! - Each mod's unpacked files are hard-linked into the game's mod folder (copied when the
//!   library is on another drive), in the profile's order; a later mod wins a clash.
//! - A game file that's in the way is moved to `<game>/.zoms-modzilla-backup/` first and put back
//!   when the mods are taken out again.
//! - Everything placed is listed in `deployed/<game>.json` in the app's data folder, so taking
//!   the mods out removes exactly those files and nothing of the game's own.
//! - Bethesda games: the profile's plugins (.esp, .esm, .esl) are switched on in the Proton
//!   prefix's plugins.txt, and switched off again when the mods are taken out.
//!
//! Mods with an installer (a `fomod` folder) use the folder built from their installer's choices
//! (`fomod`); until those are made they're put in as they are, and the result says which ones.

use crate::library::{self, StoredMod};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};

pub const BACKUP: &str = ".zoms-modzilla-backup";

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
    /// Keeps its own saves (see `saves`); otherwise it shares the usual ones.
    #[serde(default)]
    pub own_saves: bool,
    /// The collection as it was at its revision, kept apart from your changes (see `layers`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<crate::layers::Base>,
    /// Named sets of changes on top of the collection, each switched on or off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addons: Vec<crate::layers::Addon>,
    /// "Load this after that" rules, kept whenever the order changes (see `order`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<crate::order::Rule>,
    /// Your notes on the profile (what it's for, what to remember).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Made from a Thunderstore mod pack: "Owner-Name" and the version, for its updates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts_pack: Option<(String, String)>,
}

/// Games whose mods go in through their own tool, not by putting files in a folder: what to
/// tell the person instead of asking for a mod folder.
pub fn mod_tool(game: &str) -> Option<&'static str> {
    Some(match game {
        "masseffectlegendaryedition" => "Mass Effect Legendary Edition mods are installed with ME3Tweaks Mod Manager, not by copying files.",
        "terraria" => "Terraria mods are installed through tModLoader (a free Steam app), in its own Mods folder.",
        _ => return None,
    })
}

/// Where a game's mods go, under its install folder. None: not known, so ask the person.
pub fn default_mod_folder(game: &str) -> Option<&'static str> {
    Some(match game {
        "skyrimspecialedition" | "skyrim" | "fallout4" | "newvegas" | "fallout3" | "oblivion" | "starfield" => "Data",
        "morrowind" => "Data Files",
        "stardewvalley" => "Mods",
        // Unreal games load extra .pak files from a ~mods folder.
        "finalfantasy7rebirth" => "End/Content/Paks/~mods",
        "clairobscurexpedition33" => "Sandfall/Content/Paks/~mods",
        "helldivers2" => "data",
        // Baldur's Gate 3: Script Extender and loose files go in the game folder; its .pak mods
        // go in its Mods folder in the prefix (bg3.rs).
        "baldursgate3" => "",
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
    /// Settings files placed, with (size, modified) then, to tell which you changed.
    #[serde(default)]
    pub settings: std::collections::BTreeMap<String, (u64, u64)>,
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
    // A folder built from installer choices is already laid out like the mod folder.
    if unpacked.join(crate::fomod::MARK).is_file() {
        return (unpacked.to_path_buf(), false);
    }
    let target = Path::new(mod_folder).file_name().map(|n| n.to_string_lossy().to_lowercase());
    let mut src = unpacked.to_path_buf();
    for _ in 0..3 {
        let entries: Vec<_> = fs::read_dir(&src).map(|r| r.flatten().collect()).unwrap_or_default();
        let has_target = entries.iter().any(|e| Some(e.file_name().to_string_lossy().to_lowercase()) == target);
        if has_target {
            return (src, true);
        }
        // One folder, maybe with a readme or pictures beside it: the mod is in the folder.
        let dirs: Vec<_> = entries.iter().filter(|e| e.path().is_dir()).collect();
        let loose_docs = entries.iter().filter(|e| !e.path().is_dir()).all(|e| crate::check::is_doc(&e.file_name().to_string_lossy()));
        match dirs.as_slice() {
            [only] if loose_docs && !matches!(only.file_name().to_string_lossy().to_lowercase().as_str(), "fomod" | "meshes" | "textures" | "scripts" | "interface" | "sound" | "seq" | "skse" | "archive" | "bin" | "r6" | "red4ext" | "bepinex" | "content" | "mods" | "plugins" | "data" | "engine" | "reframework" | "ue4ss" | "melonloader" | "userlibs" | "paks" | "binaries") => {
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
    if let Some(i) = &f.installed {
        let built = dir.join(library::safe_name(&i.folder));
        if built.is_dir() {
            return Ok(built);
        }
    }
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
    // A built folder for the game's own folder (a mod loader like SMAPI).
    if unpacked.join(crate::fomod::MARK).is_file() && unpacked.join(GAME_ROOT_MARK).is_file() {
        return walk(unpacked)
            .into_iter()
            .filter(|rel| rel.as_os_str() != crate::fomod::MARK && rel.as_os_str() != GAME_ROOT_MARK)
            .map(|rel| (rel.to_string_lossy().replace('\\', "/"), unpacked.join(rel)))
            .collect();
    }
    // SMAPI mods (Stardew Valley): each folder with a manifest is one mod and keeps its folder.
    if !unpacked.join(crate::fomod::MARK).is_file() {
        let dirs = smapi_mod_dirs(unpacked);
        if !dirs.is_empty() {
            let mut out = vec![];
            for d in dirs {
                let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "mod".into());
                for rel in walk(&d) {
                    out.push((Path::new(mod_folder).join(&name).join(&rel).to_string_lossy().replace('\\', "/"), d.join(rel)));
                }
            }
            return out;
        }
    }
    let (src, brings_folder) = content_root(unpacked, mod_folder);
    // "Data/..." in the mod goes to <game>/Data/...; anything else goes into the mod folder.
    let base = if brings_folder { Path::new(mod_folder).parent().map(Path::to_path_buf).unwrap_or_default() } else { PathBuf::from(mod_folder) };
    walk(&src)
        .into_iter()
        .filter(|rel| !rel.components().next().map(|c| c.as_os_str().eq_ignore_ascii_case("fomod")).unwrap_or(false) && rel.as_os_str() != crate::fomod::MARK)
        .map(|rel| (base.join(&rel).to_string_lossy().replace('\\', "/"), src.join(rel)))
        .collect()
}

/// Marks a built folder whose files go into the game's own folder, not its mod folder.
pub const GAME_ROOT_MARK: &str = ".zoms-modzilla-game-folder";

/// Folders holding a SMAPI mod (a manifest.json with a UniqueID), the top-most ones, up to four
/// folders down. Empty for anything else.
pub fn smapi_mod_dirs(unpacked: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![(unpacked.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        let entries: Vec<_> = rd.flatten().collect();
        let manifest = entries.iter().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("manifest.json") && e.path().is_file());
        if manifest.is_some_and(|m| fs::read_to_string(m.path()).map(|t| t.to_lowercase().contains("\"uniqueid\"")).unwrap_or(false)) {
            out.push(d);
            continue;
        }
        if depth < 4 {
            for e in entries.iter().filter(|e| e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.')) {
                stack.push((e.path(), depth + 1));
            }
        }
    }
    out.sort();
    out
}

/// SMAPI's installer (Nexus Mods' SMAPI download): build a folder from its `install.dat` for the
/// build installed (Windows under Proton, or Linux), to go into the game's own folder. None when
/// `unpacked` isn't SMAPI's installer.
pub fn smapi_build(unpacked: &Path, mod_dir: &Path, base_name: &str, windows: bool) -> Option<Result<crate::fomod::Installed, String>> {
    let os = if windows { "windows" } else { "linux" };
    let dat = walk(unpacked).into_iter().find(|p| p.to_string_lossy().to_lowercase().ends_with(&format!("internal/{os}/install.dat")))?;
    let build = || -> Result<crate::fomod::Installed, String> {
        let folder = library::safe_name(&format!("{base_name} (installed)"));
        let (dest, tmp) = (mod_dir.join(&folder), mod_dir.join(format!(".{folder}.building")));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        // install.dat is a zip; unpack it as one (paths that leave the folder are refused).
        let zip = tmp.join("install.zip");
        fs::copy(unpacked.join(&dat), &zip).map_err(|e| e.to_string())?;
        let res = library::unpack(&zip).and_then(|name| {
            for e in fs::read_dir(tmp.join(&name)).map_err(|e| e.to_string())?.flatten() {
                fs::rename(e.path(), tmp.join(e.file_name())).map_err(|e| e.to_string())?;
            }
            let _ = fs::remove_dir(tmp.join(&name));
            let _ = fs::remove_file(&zip);
            // Windows build: SMAPI's launcher also stands in for the game's, so Steam starts SMAPI
            // with no launch options. It's a .NET launcher that runs StardewModdingAPI.dll whatever
            // it's called; the game's own launcher is kept aside and put back with the mods.
            if windows {
                let launcher = tmp.join("StardewModdingAPI.exe");
                if launcher.is_file() {
                    fs::copy(&launcher, tmp.join("Stardew Valley.exe")).map_err(|e| e.to_string())?;
                }
            }
            fs::write(tmp.join(crate::fomod::MARK), b"SMAPI, unpacked from its installer by Zom's Modzilla.\n").map_err(|e| e.to_string())?;
            fs::write(tmp.join(GAME_ROOT_MARK), b"These files go into the game's own folder.\n").map_err(|e| e.to_string())
        });
        if let Err(e) = res {
            let _ = fs::remove_dir_all(&tmp);
            return Err(e);
        }
        let _ = fs::remove_dir_all(&dest);
        fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
        Ok(crate::fomod::Installed { folder, choices: vec![] })
    };
    Some(build())
}

/// Take out whatever mods are in the game now, putting its own files back.
pub fn clear(data: &Path, game: &str, root: &Path) -> Result<(), String> {
    clear_keeping(data, game, root, &HashMap::new()).map(|_| ())
}

/// `rel` spelled the way the game folder already has it, part by part, ignoring capitals
/// ("data/Textures/a.dds" → "Data/textures/a.dds" when those are there).
pub fn same_case(root: &Path, rel: &str) -> String {
    let mut at = root.to_path_buf();
    let mut out: Vec<String> = vec![];
    let mut exists = true;
    for part in rel.split('/').filter(|p| !p.is_empty()) {
        let mut name = part.to_string();
        if exists {
            if at.join(part).exists() {
                // Spelled the same already.
            } else if let Some(e) = fs::read_dir(&at).ok().and_then(|rd| rd.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))) {
                name = e.file_name().to_string_lossy().into_owned();
            } else {
                exists = false;
            }
        }
        at.push(&name);
        out.push(name);
    }
    out.join("/")
}

/// The game file is already the mod's file: the same file (a link), or a copy of it no older.
fn same_file(game: &Path, from: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (fs::symlink_metadata(game), fs::metadata(from)) {
        (Ok(g), Ok(f)) if g.is_file() => (g.dev() == f.dev() && g.ino() == f.ino()) || (g.len() == f.len() && g.mtime() >= f.mtime()),
        _ => false,
    }
}

/// Take the mods out, except files in `keep` (lowercase path → the file that should be there)
/// that are already right. Settings you changed are kept with the profile first.
fn clear_keeping(data: &Path, game: &str, root: &Path, keep: &HashMap<String, PathBuf>) -> Result<Deployed, String> {
    let d = deployed(data, game);
    crate::modsettings::capture(data, &d, root);
    let mut left = Deployed::default();
    for rel in &d.files {
        let Some(p) = inside(root, rel) else { continue };
        if !d.settings.contains_key(rel) {
            if let Some(from) = keep.get(&rel.to_lowercase()) {
                if same_file(&p, from) {
                    left.files.push(rel.clone());
                    continue;
                }
            }
        }
        let _ = fs::remove_file(p);
    }
    let still: HashSet<String> = left.files.iter().map(|f| f.to_lowercase()).collect();
    let backup = crate::legacy::backup_dir(root);
    for rel in &d.backups {
        if still.contains(&rel.to_lowercase()) {
            left.backups.push(rel.clone());
            continue;
        }
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
            let _ = fs::remove_dir(&p); // Only when empty.
            if p.is_dir() && !still.is_empty() {
                left.dirs.push(rel.clone());
            }
        }
    }
    prune_empty(&backup);
    if let (Some(file), false) = (&d.plugins_file, d.plugins.is_empty()) {
        let _ = set_plugins(Path::new(file), &d.plugins, &[], false);
    }
    save_manifest(data, game, &left)?;
    Ok(left)
}

/// Put plugins.txt's lines for `old` out and `new` in, keeping every other line as it was.
fn set_plugins(file: &Path, old: &[String], new: &[String], star: bool) -> Result<(), String> {
    let text = fs::read_to_string(file).unwrap_or_default();
    let ours: BTreeSet<String> = old.iter().chain(new.iter()).map(|p| p.to_lowercase()).collect();
    let mut lines: Vec<String> = text
        .lines()
        .filter(|l| {
            let name = l.trim().trim_start_matches('*').to_lowercase();
            !name.is_empty() && !ours.contains(&name)
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
    let body = if lines.is_empty() { String::new() } else { lines.join("\r\n") + "\r\n" };
    fs::write(file, body).map_err(|e| e.to_string())
}

pub struct Target<'a> {
    pub game: &'a str,
    pub root: &'a Path,
    pub mod_folder: &'a str,
    pub prefix: Option<&'a Path>,
    /// Copy every file instead of hard-linking it.
    pub copy: bool,
}

/// Apply a profile: take out what's there, then put this profile's mods in.
/// Whether two folders are on the same drive (so files can be hard-linked between them).
pub fn same_drive(a: &Path, b: &Path) -> bool {
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
        if t.copy || !same_drive(first, t.root) {
            // Only what isn't in the game already: files already right stay where they are.
            let need: u64 = ready
                .iter()
                .flat_map(|(_, u)| entries(u, t.mod_folder))
                .filter(|(rel, from)| !same_file(&t.root.join(same_case(t.root, rel)), from))
                .map(|(_, from)| fs::metadata(from).map(|m| m.len()).unwrap_or(0))
                .sum();
            library::ensure_space(t.root, need, "copying the mods into the game (the library is on another drive)")?;
        }
    }
    // What goes where: a later mod wins a file more than one has.
    let mut want: Vec<(String, PathBuf)> = vec![];
    let mut at: HashMap<String, usize> = HashMap::new();
    for (m, unpacked) in &ready {
        let unpacked = unpacked.as_path();
        if unpacked.join("fomod").is_dir() || walk(unpacked).iter().any(|p| p.components().next().map(|c| c.as_os_str().eq_ignore_ascii_case("fomod")).unwrap_or(false)) {
            out.installers.push(m.name.clone());
        }
        out.mods += 1;
        for (rel_s, from) in entries(unpacked, t.mod_folder) {
            let key = rel_s.to_lowercase();
            // Baldur's Gate 3's .pak mods don't go in the game folder (bg3.rs puts them in place).
            if t.game == crate::bg3::GAME && crate::bg3::not_for_game_folder(&key) {
                continue;
            }
            match at.get(&key) {
                Some(&j) => {
                    out.clashes += 1;
                    want[j] = (rel_s, from);
                }
                None => {
                    at.insert(key, want.len());
                    want.push((rel_s, from));
                }
            }
        }
    }
    // Steam's Play button starts the script extender, when that's on for the game.
    crate::extenders::swap_in(data, t.game, t.root, &mut want);
    // Files already right stay; only what changed comes out and goes in.
    let keep: HashMap<String, PathBuf> = want.iter().map(|(r, f)| (r.to_lowercase(), f.clone())).collect();
    let left = clear_keeping(data, t.game, t.root, &keep)?;
    let mut d = Deployed { profile_id: Some(profile.id.clone()), profile_name: Some(profile.name.clone()), files: left.files, backups: left.backups, ..Default::default() };
    let mut placed: HashSet<String> = d.files.iter().map(|f| f.to_lowercase()).collect();
    let mut made_dirs: BTreeSet<String> = left.dirs.into_iter().collect();
    let backup = crate::legacy::backup_dir(t.root);
    out.backed_up = d.backups.len();
    let put = |rel_s: &str, from: &Path, d: &mut Deployed, made_dirs: &mut BTreeSet<String>, placed: &mut HashSet<String>, out: &mut Applied| -> Result<(), String> {
        // Windows doesn't mind "Textures" and "textures"; Linux does. Use the spelling already there.
        let rel_owned = same_case(t.root, rel_s);
        let rel_s = rel_owned.as_str();
        let rel_game = PathBuf::from(rel_s);
        let Some(to) = inside(t.root, rel_s) else { return Ok(()) };
        let key = rel_s.to_lowercase();
        if placed.contains(&key) {
            let _ = fs::remove_file(&to);
        } else if to.exists() || to.is_symlink() {
            // The game's own file: keep it safe.
            let b = backup.join(&rel_game);
            fs::create_dir_all(b.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::rename(&to, &b).map_err(|e| format!("Couldn't move the game's {rel_s} aside: {e}"))?;
            d.backups.push(rel_s.to_string());
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
        // Settings are copies, so a mod saving its settings never changes the library's file.
        if t.copy || crate::modsettings::is_setting(rel_s) || fs::hard_link(from, &to).is_err() {
            fs::copy(from, &to).map_err(|e| format!("Couldn't put {rel_s} in the game folder: {e}"))?;
        }
        if placed.insert(key) {
            d.files.push(rel_s.to_string());
        }
        Ok(())
    };
    for (rel_s, from) in &want {
        if placed.contains(&rel_s.to_lowercase()) {
            continue; // Already right.
        }
        put(rel_s, from, &mut d, &mut made_dirs, &mut placed, &mut out)?;
        // Save as we go, so a failure halfway can still be undone.
        if d.files.len().is_multiple_of(500) {
            d.dirs = made_dirs.iter().cloned().collect();
            save_manifest(data, t.game, &d)?;
        }
    }
    // This profile's own settings, over the mods' defaults: where a mod put that file, or in a
    // folder made for the mods (never over a game file).
    for (rel, file) in crate::modsettings::kept(data, &profile.id) {
        let low = rel.to_lowercase();
        let ours = placed.contains(&low) || made_dirs.iter().any(|m| low.starts_with(&format!("{}/", m.to_lowercase())));
        let game_file = inside(t.root, &rel).is_some_and(|p| p.exists()) && !placed.contains(&low);
        if ours && !game_file {
            put(&rel, &file, &mut d, &mut made_dirs, &mut placed, &mut out)?;
        }
    }
    for f in d.files.iter().filter(|f| crate::modsettings::is_setting(f)) {
        if let Some(s) = inside(t.root, f).and_then(|p| crate::modsettings::stamp(&p)) {
            d.settings.insert(f.clone(), s);
        }
    }
    d.dirs = made_dirs.into_iter().collect();
    out.files = d.files.len();

    // Bethesda plugins, in the profile's order.
    if let (Some((folder, star)), Some(prefix)) = (plugins_txt(t.game), t.prefix) {
        let data_dir = t.mod_folder.to_lowercase();
        // In the profile's mod order (not the order files went in: ones already in place come
        // first there), then the kept plugin order (LOOT) on top.
        let mut plugins: Vec<String> = want
            .iter()
            .map(|(rel, _)| rel)
            .filter(|f| placed.contains(&f.to_lowercase()))
            .filter_map(|f| {
                let (dir, name) = f.rsplit_once('/')?;
                let ext = name.rsplit('.').next()?.to_lowercase();
                (dir.to_lowercase() == data_dir && matches!(ext.as_str(), "esp" | "esm" | "esl")).then(|| name.to_string())
            })
            .collect();
        crate::loot::apply_order(&mut plugins, &crate::loot::saved(data, &profile.id));
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
            adult: false,
            source: None,
            folder: folder.to_string_lossy().into_owned(),
            files: vec![StoredFile { file_id: id * 10, name: "Main".into(), file_name: "m.7z".into(), version: None, category: None, size: 1, downloaded_at: 0, archive: None, unpacked: Some("unpacked".into()), uploaded: None, verified: None, installed: None, has_installer: false }],
        }
    }

    fn profile(ids: &[u64]) -> Profile {
        Profile { id: "p1".into(), name: "Winter".into(), game: "skyrimspecialedition".into(), mods: ids.iter().map(|i| ProfileMod { mod_id: *i, file_id: i * 10, enabled: true }).collect(), collection: None, manual_requirements: false, new_downloads: false, own_saves: false, base: None, addons: vec![], rules: vec![], note: None, ts_pack: None }
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
        let t = Target { game: "skyrimspecialedition", root: &game, mod_folder: "Data", prefix: Some(&pfx), copy: false };
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
    fn steam_starts_the_script_extender_and_clear_puts_the_launcher_back() {
        let tmp = tempfile::tempdir().unwrap();
        let (data, lib, game) = (tmp.path().join("data"), tmp.path().join("lib"), tmp.path().join("game"));
        fs::create_dir_all(&game).unwrap();
        fs::write(game.join("SkyrimSELauncher.exe"), "launcher").unwrap();
        let skse = lib_mod(&lib, 1, &[("skse64_2_02/skse64_loader.exe", "loader"), ("skse64_2_02/Data/Scripts/a.pex", "s")]);
        let t = Target { game: "skyrimspecialedition", root: &game, mod_folder: "Data", prefix: None, copy: false };
        apply(&data, &t, &profile(&[1]), std::slice::from_ref(&skse)).unwrap();
        assert_eq!(fs::read_to_string(game.join("SkyrimSELauncher.exe")).unwrap(), "launcher", "off by default");
        crate::extenders::set(&data, "skyrimspecialedition", true).unwrap();
        apply(&data, &t, &profile(&[1]), std::slice::from_ref(&skse)).unwrap();
        assert_eq!(fs::read_to_string(game.join("SkyrimSELauncher.exe")).unwrap(), "loader");
        assert!(crate::extenders::info(&data, "skyrimspecialedition", &game).unwrap().swapped);
        // Switched off: the next time the profile goes in, the launcher is back.
        crate::extenders::set(&data, "skyrimspecialedition", false).unwrap();
        apply(&data, &t, &profile(&[1]), std::slice::from_ref(&skse)).unwrap();
        assert_eq!(fs::read_to_string(game.join("SkyrimSELauncher.exe")).unwrap(), "launcher");
        // A loader put in by hand works too, and Clear puts the launcher back.
        crate::extenders::set(&data, "skyrimspecialedition", true).unwrap();
        clear(&data, "skyrimspecialedition", &game).unwrap();
        fs::write(game.join("skse64_loader.exe"), "by hand").unwrap();
        apply(&data, &t, &profile(&[]), &[]).unwrap();
        assert_eq!(fs::read_to_string(game.join("SkyrimSELauncher.exe")).unwrap(), "by hand");
        clear(&data, "skyrimspecialedition", &game).unwrap();
        assert_eq!(fs::read_to_string(game.join("SkyrimSELauncher.exe")).unwrap(), "launcher");
        assert!(game.join("skse64_loader.exe").exists(), "the game's own files stay");
    }

    #[test]
    fn switching_profiles_takes_the_old_one_out() {
        let tmp = tempfile::tempdir().unwrap();
        let (data, lib, game) = (tmp.path().join("data"), tmp.path().join("lib"), tmp.path().join("game"));
        fs::create_dir_all(&game).unwrap();
        let m1 = lib_mod(&lib, 1, &[("a.txt", "1")]);
        let m2 = lib_mod(&lib, 2, &[("b.txt", "2")]);
        let t = Target { game: "skyrimspecialedition", root: &game, mod_folder: "Data", prefix: None, copy: false };
        apply(&data, &t, &profile(&[1]), &[m1.clone(), m2.clone()]).unwrap();
        assert!(game.join("Data/a.txt").exists());
        apply(&data, &t, &profile(&[2]), &[m1, m2]).unwrap();
        assert!(!game.join("Data/a.txt").exists());
        assert!(game.join("Data/b.txt").exists());
        assert_eq!(deployed(&data, "skyrimspecialedition").profile_name.as_deref(), Some("Winter"));
    }

    #[test]
    fn unchanged_files_stay_and_each_profile_keeps_its_settings() {
        use std::os::unix::fs::MetadataExt;
        let tmp = tempfile::tempdir().unwrap();
        let (data, lib, game) = (tmp.path().join("data"), tmp.path().join("lib"), tmp.path().join("game"));
        fs::create_dir_all(&game).unwrap();
        let m1 = lib_mod(&lib, 1, &[("SKSE/a.dll", "code"), ("SKSE/cool.ini", "volume=5")]);
        let m2 = lib_mod(&lib, 2, &[("b.txt", "2")]);
        let t = Target { game: "skyrimspecialedition", root: &game, mod_folder: "Data", prefix: None, copy: false };
        let p1 = profile(&[1]);
        let mut p2 = profile(&[1, 2]);
        p2.id = "p2".into();
        let all = [m1.clone(), m2.clone()];
        apply(&data, &t, &p1, &all).unwrap();
        let ino = fs::metadata(game.join("Data/SKSE/a.dll")).unwrap().ino();
        // Settings are copies: the library's file never changes.
        let ini = game.join("Data/SKSE/cool.ini");
        assert_ne!(fs::metadata(&ini).unwrap().ino(), fs::metadata(lib.join("skyrimspecialedition/1 - Mod 1/unpacked/SKSE/cool.ini")).unwrap().ino());
        // While playing: you change a setting, and the mod makes a new settings file.
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&ini, "volume=9").unwrap();
        fs::write(game.join("Data/SKSE/made.json"), "{}").unwrap();
        // Another profile: the same mod's files stay where they are; p1's settings wait.
        apply(&data, &t, &p2, &all).unwrap();
        assert_eq!(fs::metadata(game.join("Data/SKSE/a.dll")).unwrap().ino(), ino, "unchanged file left alone");
        assert_eq!(fs::read_to_string(&ini).unwrap(), "volume=5");
        assert!(!game.join("Data/SKSE/made.json").exists());
        assert_eq!(fs::read_to_string(lib.join("skyrimspecialedition/1 - Mod 1/unpacked/SKSE/cool.ini")).unwrap(), "volume=5");
        // Back to p1: its settings come back.
        apply(&data, &t, &p1, &all).unwrap();
        assert_eq!(fs::read_to_string(&ini).unwrap(), "volume=9");
        assert!(game.join("Data/SKSE/made.json").exists());
        assert!(!game.join("Data/b.txt").exists());
        // Taking the mods out keeps them with p1 and leaves nothing behind.
        clear(&data, "skyrimspecialedition", &game).unwrap();
        assert!(!game.join("Data").exists());
        assert_eq!(crate::modsettings::kept(&data, "p1").len(), 2);
    }

    #[test]
    fn mods_use_the_spelling_already_in_the_game() {
        let tmp = tempfile::tempdir().unwrap();
        let (data, lib, game) = (tmp.path().join("data"), tmp.path().join("lib"), tmp.path().join("game"));
        fs::create_dir_all(game.join("Data/textures")).unwrap();
        fs::write(game.join("Data/textures/Rock.dds"), "game").unwrap();
        let m1 = lib_mod(&lib, 1, &[("Textures/rock.dds", "mod"), ("Textures/new.dds", "n")]);
        let t = Target { game: "skyrimspecialedition", root: &game, mod_folder: "Data", prefix: None, copy: false };
        apply(&data, &t, &profile(&[1]), &[m1]).unwrap();
        assert!(!game.join("Data/Textures").exists(), "no second folder");
        assert_eq!(fs::read_to_string(game.join("Data/textures/Rock.dds")).unwrap(), "mod");
        assert!(game.join("Data/textures/new.dds").is_file());
        clear(&data, "skyrimspecialedition", &game).unwrap();
        assert_eq!(fs::read_to_string(game.join("Data/textures/Rock.dds")).unwrap(), "game", "the game's own file is back");
        assert!(!game.join("Data/textures/new.dds").exists());
    }

    #[test]
    fn plugin_order_follows_the_profile_not_the_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let (data, lib, game, pfx) = (tmp.path().join("data"), tmp.path().join("lib"), tmp.path().join("game"), tmp.path().join("pfx"));
        fs::create_dir_all(game.join("Data")).unwrap();
        let plugins = pfx.join("drive_c/users/steamuser/AppData/Local/Skyrim Special Edition/plugins.txt");
        let m1 = lib_mod(&lib, 1, &[("A.esp", "a")]);
        let m2 = lib_mod(&lib, 2, &[("B.esp", "b")]);
        let t = Target { game: "skyrimspecialedition", root: &game, mod_folder: "Data", prefix: Some(&pfx), copy: false };
        let all = [m1, m2];
        apply(&data, &t, &profile(&[2]), &all).unwrap();
        // B is in place already; A goes in now, but A comes first in the profile.
        apply(&data, &t, &profile(&[1, 2]), &all).unwrap();
        assert_eq!(fs::read_to_string(&plugins).unwrap(), "*A.esp\r\n*B.esp\r\n");
        // A kept plugin order (LOOT's) wins over the mod order.
        crate::loot::save(&data, "p1", Some(&["B.esp".to_string(), "A.esp".to_string()])).unwrap();
        apply(&data, &t, &profile(&[1, 2]), &all).unwrap();
        assert_eq!(fs::read_to_string(&plugins).unwrap(), "*B.esp\r\n*A.esp\r\n");
    }

    #[test]
    fn manifest_paths_cannot_leave_the_game_folder() {
        assert!(inside(Path::new("/g"), "../etc/passwd").is_none());
        assert!(inside(Path::new("/g"), "/etc/passwd").is_none());
        assert!(inside(Path::new("/g"), "Data/a.esp").is_some());
    }

    #[test]
    fn smapi_mods_keep_their_folders_and_smapi_goes_to_the_game_folder() {
        use std::io::Write;
        let d = tempfile::tempdir().unwrap();
        // One mod in a wrapping folder, and a pack of two.
        let one = d.path().join("one");
        fs::create_dir_all(one.join("Automate")).unwrap();
        fs::write(one.join("Automate/manifest.json"), r#"{"Name":"Automate","UniqueID":"Pathoschild.Automate"}"#).unwrap();
        fs::write(one.join("Automate/Automate.dll"), "x").unwrap();
        let e: Vec<String> = entries(&one, "Mods").into_iter().map(|(r, _)| r).collect();
        assert_eq!(e, vec!["Mods/Automate/Automate.dll", "Mods/Automate/manifest.json"]);
        let pack = d.path().join("pack/Ridgeside");
        for m in ["[CP] RSV", "[CC] RSV"] {
            fs::create_dir_all(pack.join(m)).unwrap();
            fs::write(pack.join(m).join("manifest.json"), r#"{"UniqueID":"x"}"#).unwrap();
        }
        let e: Vec<String> = entries(&d.path().join("pack"), "Mods").into_iter().map(|(r, _)| r).collect();
        assert_eq!(e, vec!["Mods/[CC] RSV/manifest.json", "Mods/[CP] RSV/manifest.json"]);

        // SMAPI's installer: install.dat (a zip) for Windows becomes a folder for the game's own folder.
        let smapi = d.path().join("SMAPI 4/SMAPI 4 installer/internal/windows");
        fs::create_dir_all(&smapi).unwrap();
        {
            let mut w = zip::ZipWriter::new(fs::File::create(smapi.join("install.dat")).unwrap());
            for (n, c) in [("StardewModdingAPI.exe", "smapi"), ("smapi-internal/a.dll", "a"), ("Mods/ConsoleCommands/manifest.json", r#"{"UniqueID":"c"}"#)] {
                w.start_file(n, zip::write::SimpleFileOptions::default()).unwrap();
                w.write_all(c.as_bytes()).unwrap();
            }
            w.finish().unwrap();
        }
        let modd = d.path().join("lib/2400 - SMAPI");
        fs::create_dir_all(&modd).unwrap();
        let built = smapi_build(&d.path().join("SMAPI 4"), &modd, "SMAPI 4", true).unwrap().unwrap();
        let folder = modd.join(&built.folder);
        assert_eq!(fs::read_to_string(folder.join("Stardew Valley.exe")).unwrap(), "smapi", "stands in for the game's launcher");
        let e: Vec<String> = entries(&folder, "Mods").into_iter().map(|(r, _)| r).collect();
        assert!(e.contains(&"StardewModdingAPI.exe".to_string()) && e.contains(&"Mods/ConsoleCommands/manifest.json".to_string()) && e.contains(&"smapi-internal/a.dll".to_string()));
        assert!(!e.iter().any(|x| x.contains(".zoms-modzilla")));
        assert!(smapi_build(&one, &modd, "x", true).is_none(), "not SMAPI");
    }
}
