//! Each profile keeps its own mod settings. Settings files (.cfg, .ini, .json …) go into the game
//! as copies, never links, so a mod writing to one never changes the library's copy. When the
//! mods come out again, the settings you changed, and the ones mods made on their first run
//! inside folders Zom's Nexus made, move to `<data>/profile-settings/<profile>/` and go back in
//! the next time that profile does. Nothing outside the folders Zom's Nexus made, or the files it
//! placed, is ever touched.

use crate::deploy::Deployed;
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    time::UNIX_EPOCH,
};

/// Bigger than this isn't a settings file (a cache, say).
const MAX: u64 = 2 * 1024 * 1024;

/// A file a mod keeps its settings in.
pub fn is_setting(rel: &str) -> bool {
    let low = rel.to_lowercase();
    let name = low.rsplit('/').next().unwrap_or(&low);
    if matches!(name, "manifest.json" | "content.json" | "i18n.json" | "info.json" | "modinfo.json" | "package.json") || low.contains("/i18n/") {
        return false;
    }
    let ext = name.rsplit('.').next().unwrap_or("");
    name.contains('.') && matches!(ext, "cfg" | "ini" | "json" | "toml" | "yaml" | "yml" | "conf")
}

/// Where a profile's settings wait.
pub fn store(data: &Path, profile: &str) -> PathBuf {
    data.join("profile-settings").join(crate::library::safe_name(profile))
}

fn safe(root: &Path, rel: &str) -> Option<PathBuf> {
    let p = Path::new(rel);
    (!rel.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)))).then(|| root.join(p))
}

/// (size, modified) of a file, to tell later whether it changed.
pub fn stamp(p: &Path) -> Option<(u64, u64)> {
    let m = fs::metadata(p).ok()?;
    Some((m.len(), m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos() as u64 / 1_000_000))
}

/// Before the mods come out: keep the profile's changed settings, and take settings files mods
/// made inside folders Zom's Nexus made. Returns how many were kept.
pub fn capture(data: &Path, d: &Deployed, root: &Path) -> usize {
    let Some(profile) = d.profile_id.as_deref() else { return 0 };
    let to = store(data, profile);
    let mut n = 0;
    let keep = |rel: &str, from: &Path, take: bool| -> bool {
        let Some(dest) = safe(&to, rel) else { return false };
        if fs::metadata(from).map(|m| !m.is_file() || m.len() > MAX).unwrap_or(true) {
            return false;
        }
        let _ = fs::create_dir_all(dest.parent().unwrap());
        if fs::copy(from, &dest).is_err() {
            return false;
        }
        if take {
            let _ = fs::remove_file(from);
        }
        true
    };
    // Settings it placed and something changed since.
    for (rel, was) in &d.settings {
        let Some(p) = safe(root, rel) else { continue };
        if stamp(&p).is_some_and(|now| now != *was) && keep(rel, &p, false) {
            n += 1;
        }
    }
    // Settings mods made, in folders that weren't there before the mods went in.
    let placed: std::collections::HashSet<String> = d.files.iter().map(|f| f.to_lowercase()).collect();
    for dir in top_dirs(&d.dirs) {
        let Some(base) = safe(root, &dir) else { continue };
        for f in walk(&base) {
            let rel = format!("{dir}/{}", f.to_string_lossy().replace('\\', "/"));
            if !placed.contains(&rel.to_lowercase()) && is_setting(&rel) && keep(&rel, &base.join(&f), true) {
                n += 1;
            }
        }
    }
    n
}

/// The folders in `dirs` that aren't inside another one of them.
fn top_dirs(dirs: &[String]) -> Vec<String> {
    let mut all: Vec<&String> = dirs.iter().collect();
    all.sort_by_key(|d| d.len());
    let mut out: Vec<String> = vec![];
    for d in all {
        if !out.iter().any(|o| d.starts_with(&format!("{o}/"))) {
            out.push(d.clone());
        }
    }
    out
}

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
    out
}

/// The profile's kept settings: relative path → file.
pub fn kept(data: &Path, profile: &str) -> BTreeMap<String, PathBuf> {
    let base = store(data, profile);
    walk(&base).into_iter().map(|r| (r.to_string_lossy().replace('\\', "/"), base.join(r))).collect()
}

/// Forget one kept setting (back to the mod's own).
pub fn forget(data: &Path, profile: &str, rel: &str) -> Result<(), String> {
    let p = safe(&store(data, profile), rel).ok_or("That isn't one of this profile's settings.")?;
    fs::remove_file(&p).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_settings_from_mod_files() {
        assert!(is_setting("BepInEx/config/com.cool.mod.cfg"));
        assert!(is_setting("Mods/Cool Mod/config.json"));
        assert!(is_setting("Data/SKSE/Plugins/thing.ini"));
        assert!(!is_setting("Mods/Cool Mod/manifest.json"));
        assert!(!is_setting("Mods/[CP] Cool/content.json"));
        assert!(!is_setting("Data/textures/a.dds"));
        assert_eq!(top_dirs(&["a".into(), "a/b".into(), "c/d".into(), "c/d/e".into()]), vec!["a", "c/d"]);
    }
}
