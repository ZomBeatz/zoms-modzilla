//! What fills the drives Zom's Nexus uses, for the bar at the bottom left: unpacked mods, mod
//! archives (zips), Steam games, and everything else.
//!
//! Mods are measured on disk (walking the library, so the answer is kept for two minutes);
//! Steam games come from the size Steam notes in each appmanifest, which is free to read.

use serde::Serialize;
use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DriveUse {
    /// What's on it, for the label: "mods", "mods and Cyberpunk 2077", "Cyberpunk 2077".
    pub holds: Vec<String>,
    pub total: u64,
    pub free: u64,
    /// Unpacked mods in the library.
    pub mods: u64,
    /// Mod archives (zip, 7z, rar) kept in the library.
    pub archives: u64,
    /// Games Steam installed on this drive.
    pub steam: u64,
}

fn dev(p: &Path) -> Option<u64> {
    let mut p = p.to_path_buf();
    while !p.exists() {
        p = p.parent()?.to_path_buf();
    }
    fs::metadata(&p).ok().map(|m| m.dev())
}

/// Bytes of Steam games in one Steam library folder, from its appmanifests.
pub fn steam_size(lib: &Path) -> u64 {
    let Ok(rd) = fs::read_dir(lib.join("steamapps")) else { return 0 };
    rd.flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("appmanifest_"))
        .filter_map(|e| fs::read_to_string(e.path()).ok())
        .filter_map(|acf| crate::games::acf_value(&acf, "SizeOnDisk")?.parse::<u64>().ok())
        .sum()
}

static MEASURED: Mutex<Option<(Instant, PathBuf, u64, u64)>> = Mutex::new(None);

/// Unpacked and archive bytes in the library, measured at most every two minutes.
fn library_bytes(lib: &Path) -> (u64, u64) {
    if let Some((at, p, m, a)) = MEASURED.lock().unwrap().clone() {
        if p == lib && at.elapsed() < Duration::from_secs(120) {
            return (m, a);
        }
    }
    let (m, a) = crate::library::usage(lib).iter().fold((0, 0), |(m, a), u| (m + u.unpacked, a + u.archive));
    *MEASURED.lock().unwrap() = Some((Instant::now(), lib.to_path_buf(), m, a));
    (m, a)
}

/// The library's drive, and the game's when it's another one.
pub fn breakdown(lib: &Path, game: Option<(&str, &Path)>, steam_libs: &[PathBuf]) -> Vec<DriveUse> {
    let mut out: Vec<(Option<u64>, DriveUse)> = vec![];
    let mut add = |path: &Path, what: &str| {
        let d = dev(path);
        if let Some((_, u)) = out.iter_mut().find(|(x, _)| x.is_some() && *x == d) {
            u.holds.push(what.to_string());
            return;
        }
        let (free, total) = crate::settings::disk(path).unwrap_or((0, 0));
        let steam = steam_libs.iter().filter(|s| d.is_some() && dev(s) == d).map(|s| steam_size(s)).sum();
        out.push((d, DriveUse { holds: vec![what.to_string()], total, free, mods: 0, archives: 0, steam }));
    };
    add(lib, "mods");
    if let Some((name, path)) = game {
        add(path, name);
    }
    let (m, a) = library_bytes(lib);
    if let Some((_, u)) = out.first_mut() {
        u.mods = m;
        u.archives = a;
    }
    out.into_iter().map(|(_, u)| u).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_steam_sizes_and_groups_one_drive() {
        let dir = tempfile::tempdir().unwrap();
        let steam = dir.path().join("SteamLibrary");
        fs::create_dir_all(steam.join("steamapps")).unwrap();
        fs::write(steam.join("steamapps/appmanifest_1.acf"), "\"AppState\"\n{\n\t\"appid\"\t\t\"1\"\n\t\"SizeOnDisk\"\t\t\"1000\"\n}\n").unwrap();
        fs::write(steam.join("steamapps/appmanifest_2.acf"), "\t\"SizeOnDisk\"\t\t\"234\"\n").unwrap();
        assert_eq!(steam_size(&steam), 1234);

        let lib = dir.path().join("lib");
        fs::create_dir_all(&lib).unwrap();
        let d = breakdown(&lib, Some(("A game", &steam)), std::slice::from_ref(&steam));
        assert_eq!(d.len(), 1, "same drive: one bar");
        assert_eq!(d[0].holds, vec!["mods", "A game"]);
        assert_eq!(d[0].steam, 1234);
    }
}
