//! Script extenders started from Steam. Steam's Play button (and Game Mode on a Steam Deck)
//! starts a Bethesda game's launcher, so SKSE, F4SE and the rest never run unless you leave
//! Game Mode to start their loader. With this on for a game, putting a profile in also puts the
//! loader where the launcher was: the launcher is kept aside like any other game file and comes
//! back with Clear, or when this is switched off and the profile goes in again.

use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

/// (game, extender, its loader, the launcher Steam starts).
pub const KNOWN: &[(&str, &str, &str, &str)] = &[
    ("skyrimspecialedition", "SKSE", "skse64_loader.exe", "SkyrimSELauncher.exe"),
    ("skyrim", "SKSE", "skse_loader.exe", "SkyrimLauncher.exe"),
    ("fallout4", "F4SE", "f4se_loader.exe", "Fallout4Launcher.exe"),
    ("newvegas", "NVSE", "nvse_loader.exe", "FalloutNVLauncher.exe"),
    ("fallout3", "FOSE", "fose_loader.exe", "Fallout3Launcher.exe"),
    ("oblivion", "OBSE", "obse_loader.exe", "OblivionLauncher.exe"),
];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Extender {
    pub name: String,
    pub loader: String,
    pub launcher: String,
    /// Switched on for this game.
    pub on: bool,
    /// The loader is in the game folder now (or in a mod of the profile that's in).
    pub installed: bool,
    /// The launcher is the loader right now.
    pub swapped: bool,
}

pub fn known(game: &str) -> Option<(&'static str, &'static str, &'static str)> {
    KNOWN.iter().find(|(g, ..)| *g == game).map(|(_, n, l, s)| (*n, *l, *s))
}

fn file(data: &Path) -> PathBuf {
    data.join("start-extender.json")
}

fn games_on(data: &Path) -> BTreeSet<String> {
    fs::read(file(data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn is_on(data: &Path, game: &str) -> bool {
    games_on(data).contains(game)
}

pub fn set(data: &Path, game: &str, on: bool) -> Result<(), String> {
    known(game).ok_or("This game has no script extender to start from Steam.")?;
    let mut all = games_on(data);
    if on {
        all.insert(game.to_string());
    } else {
        all.remove(game);
    }
    fs::create_dir_all(data).map_err(|e| e.to_string())?;
    fs::write(file(data), serde_json::to_vec_pretty(&all).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn find_in(root: &Path, name: &str) -> Option<PathBuf> {
    fs::read_dir(root).ok()?.flatten().map(|e| e.path()).find(|p| p.is_file() && p.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(name)))
}

pub fn info(data: &Path, game: &str, root: &Path) -> Option<Extender> {
    let (name, loader, launcher) = known(game)?;
    let d = crate::deploy::deployed(data, game);
    let swapped = d.files.iter().any(|f| f.eq_ignore_ascii_case(launcher));
    Some(Extender { name: name.into(), loader: loader.into(), launcher: launcher.into(), on: is_on(data, game), installed: find_in(root, loader).is_some(), swapped })
}

/// Called while a profile goes in: when it's on and the loader is there (from a mod in `want`,
/// or put in the game folder by hand), the launcher's place gets the loader.
pub fn swap_in(data: &Path, game: &str, root: &Path, want: &mut Vec<(String, PathBuf)>) {
    let Some((_, loader, launcher)) = known(game) else { return };
    if !is_on(data, game) {
        return;
    }
    let from_mod = want.iter().find(|(r, _)| r.eq_ignore_ascii_case(loader)).map(|(_, f)| f.clone());
    // A loader put in by hand (not one an earlier profile put in: that one is about to go).
    let by_hand = || {
        let d = crate::deploy::deployed(data, game);
        find_in(root, loader).filter(|_| !d.files.iter().any(|f| f.eq_ignore_ascii_case(loader)))
    };
    let Some(src) = from_mod.or_else(by_hand) else { return };
    want.retain(|(r, _)| !r.eq_ignore_ascii_case(launcher));
    want.push((launcher.to_string(), src));
}
