//! Terraria's tModLoader: its mods come from the Steam Workshop (subscribe on Steam; only Steam can
//! download them), and this manages which are switched on and tModLoader's own mod packs, in the
//! same files tModLoader's Manage Mods menu uses (`Mods/enabled.json`, `Mods/ModPacks/<pack>/`).
//! So a pack made here shows up in tModLoader, and the other way round. Nothing of Steam's is
//! changed. The first change keeps a copy of the enabled.json there was.

use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// tModLoader on Steam.
pub const APP_ID: u32 = 1281930;

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TmlMod {
    /// Its internal name (the .tmod file's name), what enabled.json lists.
    pub name: String,
    /// "Calamity Mod" from "CalamityMod".
    pub label: String,
    pub workshop_id: Option<u64>,
    pub enabled: bool,
    pub size: u64,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TmlPack {
    pub name: String,
    pub mods: Vec<String>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TmlState {
    pub mods_dir: String,
    pub mods: Vec<TmlMod>,
    pub packs: Vec<TmlPack>,
    /// Switched on but not installed (unsubscribed, say).
    pub missing: Vec<String>,
}

/// "CalamityModMusic" → "Calamity Mod Music".
pub fn label(name: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = name.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
        if i > 0 && c.is_uppercase() && (chars[i - 1].is_lowercase() || (chars[i - 1].is_uppercase() && next_lower)) {
            out.push(' ');
        }
        out.push(if *c == '_' { ' ' } else { *c });
    }
    out
}

/// tModLoader's Mods folder: the native Linux one, or the one in its Proton prefix.
pub fn mods_dir() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let mut found = vec![home.join(".local/share/Terraria/tModLoader/Mods")];
    for lib in crate::games::steam_library_dirs() {
        found.push(lib.join(format!("steamapps/compatdata/{APP_ID}/pfx/drive_c/users/steamuser/Documents/My Games/Terraria/tModLoader/Mods")));
    }
    found.into_iter().find(|d| d.is_dir())
}

/// Workshop mods: every subscribed item's newest .tmod, by internal name.
fn workshop() -> Vec<(String, u64, PathBuf)> {
    let mut out = vec![];
    for lib in crate::games::steam_library_dirs() {
        let base = lib.join(format!("steamapps/workshop/content/{APP_ID}"));
        out.extend(workshop_in(&base));
    }
    out
}

/// Items in a workshop content folder: `<id>/<version folder>/<Name>.tmod` or `<id>/<Name>.tmod`.
pub fn workshop_in(base: &Path) -> Vec<(String, u64, PathBuf)> {
    let mut out = vec![];
    for item in fs::read_dir(base).into_iter().flatten().flatten() {
        let Some(id) = item.file_name().to_str().and_then(|s| s.parse::<u64>().ok()) else { continue };
        let mut tmods: Vec<PathBuf> = vec![];
        let mut stack = vec![(item.path(), 0)];
        while let Some((d, depth)) = stack.pop() {
            for e in fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() && depth < 2 {
                    stack.push((p, depth + 1));
                } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("tmod")) {
                    tmods.push(p);
                }
            }
        }
        // The newest version folder wins (they're named by tModLoader version: "2024.5").
        tmods.sort();
        if let Some(p) = tmods.pop() {
            let name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            out.push((name, id, p));
        }
    }
    out
}

fn read_list(p: &Path) -> Vec<String> {
    fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<Vec<String>>(&t).ok()).unwrap_or_default()
}

pub fn state_in(mods_dir: &Path, workshop: &[(String, u64, PathBuf)]) -> TmlState {
    let enabled = read_list(&mods_dir.join("enabled.json"));
    let mut all: BTreeMap<String, TmlMod> = BTreeMap::new();
    for (name, id, p) in workshop {
        all.insert(name.to_lowercase(), TmlMod { name: name.clone(), label: label(name), workshop_id: Some(*id), enabled: false, size: fs::metadata(p).map(|m| m.len()).unwrap_or(0) });
    }
    // Mods put in the Mods folder by hand (or being made).
    for e in fs::read_dir(mods_dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("tmod")) {
            let name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            all.entry(name.to_lowercase()).or_insert(TmlMod { label: label(&name), name, workshop_id: None, enabled: false, size: fs::metadata(&p).map(|m| m.len()).unwrap_or(0) });
        }
    }
    let mut missing = vec![];
    for n in &enabled {
        match all.get_mut(&n.to_lowercase()) {
            Some(m) => m.enabled = true,
            None => missing.push(n.clone()),
        }
    }
    let mut packs: Vec<TmlPack> = fs::read_dir(mods_dir.join("ModPacks"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let p = e.path();
            // tModLoader 1.4: a folder with Mods/enabled.json; older: a <name>.json list.
            let list = if p.is_dir() { read_list(&p.join("Mods/enabled.json")) } else if p.extension().is_some_and(|x| x == "json") { read_list(&p) } else { return None };
            let name = if p.is_dir() { e.file_name().to_string_lossy().into_owned() } else { p.file_stem()?.to_string_lossy().into_owned() };
            Some(TmlPack { name, mods: list })
        })
        .collect();
    packs.sort_by_key(|a| a.name.to_lowercase());
    let mut mods: Vec<TmlMod> = all.into_values().collect();
    mods.sort_by(|a, b| b.enabled.cmp(&a.enabled).then(a.label.to_lowercase().cmp(&b.label.to_lowercase())));
    TmlState { mods_dir: mods_dir.to_string_lossy().into_owned(), mods, packs, missing }
}

pub fn state() -> Option<TmlState> {
    Some(state_in(&mods_dir()?, &workshop()))
}

fn safe_names(names: &[String]) -> Result<Vec<String>, String> {
    names.iter().map(|n| if n.is_empty() || n.contains(['/', '\\', '"']) || n.len() > 200 { Err(format!("“{n}” isn't a mod name.")) } else { Ok(n.clone()) }).collect()
}

/// Switch on exactly these mods (tModLoader reads it when it starts).
pub fn set_enabled(mods_dir: &Path, names: &[String]) -> Result<(), String> {
    let names = safe_names(names)?;
    let file = mods_dir.join("enabled.json");
    let keep = mods_dir.join("enabled.json.zoms-nexus-backup");
    if file.is_file() && !keep.exists() {
        fs::copy(&file, &keep).map_err(|e| e.to_string())?;
    }
    crate::settings::write_atomic(&file, serde_json::to_string_pretty(&names).map_err(|e| e.to_string())?.as_bytes())
}

/// Save a mod pack the way tModLoader does, so its menu lists it too.
pub fn save_pack(mods_dir: &Path, name: &str, names: &[String]) -> Result<(), String> {
    let names = safe_names(names)?;
    let n = crate::library::safe_name(name.trim());
    if n.is_empty() {
        return Err("Give the pack a name.".into());
    }
    let dir = mods_dir.join("ModPacks").join(&n).join("Mods");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&dir.join("enabled.json"), serde_json::to_string_pretty(&names).map_err(|e| e.to_string())?.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_switches_tmodloader_mods() {
        let t = tempfile::tempdir().unwrap();
        let ws = t.path().join("workshop");
        for (id, ver, name) in [(111, "2023.8", "CalamityMod"), (111, "2024.5", "CalamityMod"), (222, "2024.5", "MagicStorage")] {
            fs::create_dir_all(ws.join(format!("{id}/{ver}"))).unwrap();
            fs::write(ws.join(format!("{id}/{ver}/{name}.tmod")), ver).unwrap();
        }
        let mods = t.path().join("Mods");
        fs::create_dir_all(mods.join("ModPacks/Boss rush/Mods")).unwrap();
        fs::write(mods.join("enabled.json"), r#"["CalamityMod", "GoneMod"]"#).unwrap();
        fs::write(mods.join("ModPacks/Boss rush/Mods/enabled.json"), r#"["CalamityMod","MagicStorage"]"#).unwrap();
        fs::write(mods.join("MyOwn.tmod"), "x").unwrap();
        let w = workshop_in(&ws);
        assert!(w.iter().any(|(n, id, p)| n == "CalamityMod" && *id == 111 && p.to_string_lossy().contains("2024.5")));
        let s = state_in(&mods, &w);
        assert_eq!(s.mods.iter().map(|m| (m.name.as_str(), m.enabled)).collect::<Vec<_>>(), vec![("CalamityMod", true), ("MagicStorage", false), ("MyOwn", false)]);
        assert_eq!(s.missing, vec!["GoneMod"]);
        assert_eq!(s.packs, vec![TmlPack { name: "Boss rush".into(), mods: vec!["CalamityMod".into(), "MagicStorage".into()] }]);
        set_enabled(&mods, &["MagicStorage".into()]).unwrap();
        assert_eq!(read_list(&mods.join("enabled.json")), vec!["MagicStorage"]);
        assert_eq!(read_list(&mods.join("enabled.json.zoms-nexus-backup")), vec!["CalamityMod", "GoneMod"], "the first one is kept");
        save_pack(&mods, "Chill", &["MyOwn".into()]).unwrap();
        assert!(state_in(&mods, &w).packs.iter().any(|p| p.name == "Chill"));
        assert!(set_enabled(&mods, &["../x".into()]).is_err());
        assert_eq!(label("CalamityModMusic"), "Calamity Mod Music");
        assert_eq!(label("UIMod"), "UI Mod");
    }
}
