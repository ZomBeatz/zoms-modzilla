//! Saves per profile: a profile set to keep its own saves gets a save folder of its own; every
//! other profile shares the usual one.
//!
//! The game's save folder (the "live" one) always holds the saves of whoever is in the game now.
//! The others wait beside it in `<save folder's parent>/.zoms-nexus-saves/<owner>/`, on the same
//! drive, so switching is a rename: instant, and nothing is ever copied or deleted. Something
//! unexpected already at a destination is renamed aside (`… (kept 1700000000)`), never replaced.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// The owner of the usual saves: every profile that doesn't keep its own.
pub const SHARED: &str = "shared";
const STASH: &str = ".zoms-nexus-saves";
const OWNER_FILE: &str = "owner";

/// Where a game keeps its saves, relative to its Proton prefix (`pfx:`) or the home folder
/// (`~/`). None when it isn't known; the person can set it on the game's card.
pub fn known(game: &str) -> Option<&'static str> {
    Some(match game {
        "cyberpunk2077" => "pfx:drive_c/users/steamuser/Saved Games/CD Projekt Red/Cyberpunk 2077",
        "skyrimspecialedition" => "pfx:drive_c/users/steamuser/Documents/My Games/Skyrim Special Edition/Saves",
        "skyrim" => "pfx:drive_c/users/steamuser/Documents/My Games/Skyrim/Saves",
        "fallout4" => "pfx:drive_c/users/steamuser/Documents/My Games/Fallout4/Saves",
        "newvegas" => "pfx:drive_c/users/steamuser/Documents/My Games/FalloutNV/Saves",
        "oblivion" => "pfx:drive_c/users/steamuser/Documents/My Games/Oblivion/Saves",
        "starfield" => "pfx:drive_c/users/steamuser/Documents/My Games/Starfield/Saves",
        "witcher3" => "pfx:drive_c/users/steamuser/Documents/The Witcher 3/gamesaves",
        "baldursgate3" => "pfx:drive_c/users/steamuser/AppData/Local/Larian Studios/Baldur's Gate 3/PlayerProfiles/Public/Savegames",
        "eldenring" => "pfx:drive_c/users/steamuser/AppData/Roaming/EldenRing",
        "stardewvalley" => "~/.config/StardewValley/Saves",
        "clairobscurexpedition33" => "pfx:drive_c/users/steamuser/AppData/Local/Sandfall/Saved/SaveGames",
        "finalfantasy7rebirth" => "pfx:drive_c/users/steamuser/Documents/My Games/FINAL FANTASY VII REBIRTH/Steam",
        "icarus" => "pfx:drive_c/users/steamuser/AppData/Local/Icarus/Saved/PlayerData",
        _ => return None,
    })
}

/// Games with a Linux build and a Windows one, which keep saves in different places: the
/// Windows build's program (in the install folder) and where it keeps them in the Proton prefix.
fn windows_build(game: &str) -> Option<(&'static str, &'static str)> {
    Some(match game {
        "stardewvalley" => ("Stardew Valley.exe", "pfx:drive_c/users/steamuser/AppData/Roaming/StardewValley/Saves"),
        _ => return None,
    })
}

/// The save folder as a full path: one set by the person, or the known one for the game (the
/// Windows build's, in its Proton prefix, when that's the build installed).
pub fn folder(game: &str, custom: Option<&str>, prefix: Option<&Path>, install: Option<&Path>) -> Option<PathBuf> {
    if let Some(c) = custom.filter(|c| !c.trim().is_empty()) {
        return Some(PathBuf::from(c));
    }
    let windows = windows_build(game).filter(|(exe, _)| install.is_some_and(|d| d.join(exe).is_file()) && prefix.is_some());
    let k = match windows {
        Some((_, pfx)) => pfx,
        None => known(game)?,
    };
    if let Some(rest) = k.strip_prefix("pfx:") {
        return prefix.map(|p| p.join(rest));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    k.strip_prefix("~/").map(|rest| home.join(rest))
}

fn stash_dir(live: &Path) -> Option<PathBuf> {
    Some(live.parent()?.join(STASH))
}

/// Whose saves are in the game's save folder now.
pub fn owner(live: &Path) -> String {
    stash_dir(live).and_then(|d| fs::read_to_string(d.join(OWNER_FILE)).ok()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| SHARED.to_string())
}

/// Only plain names become folder names (profile ids are "p" and digits).
fn safe_owner(o: &str) -> Result<&str, String> {
    if !o.is_empty() && o.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        Ok(o)
    } else {
        Err("That isn't a profile Zom's Nexus knows.".into())
    }
}

/// Move `from` to `to`, first renaming anything already at `to` aside (never replacing it).
fn move_aside(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        let kept = to.with_file_name(format!("{} (kept {})", to.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), crate::queue::now()));
        fs::rename(to, &kept).map_err(|e| format!("Couldn't keep {} aside: {e}", to.display()))?;
    }
    fs::rename(from, to).map_err(|e| format!("Couldn't move saves from {} to {}: {e}", from.display(), to.display()))
}

/// Put `want`'s saves in the game's save folder, keeping whoever's are there now beside it.
/// Gives true when it switched.
pub fn switch(live: &Path, want: &str) -> Result<bool, String> {
    let want = safe_owner(want)?.to_string();
    let now = owner(live);
    if now == want {
        return Ok(false);
    }
    let stash = stash_dir(live).ok_or("The save folder has no parent folder.")?;
    fs::create_dir_all(&stash).map_err(|e| format!("Couldn't make {}: {e}", stash.display()))?;
    // 1. The current saves go beside it, under their owner.
    if live.exists() {
        move_aside(live, &stash.join(safe_owner(&now)?))?;
    }
    // 2. The wanted saves come in (or an empty folder, for a profile's first time).
    let theirs = stash.join(&want);
    if theirs.exists() {
        fs::rename(&theirs, live).map_err(|e| format!("Couldn't move saves into {}: {e}", live.display()))?;
    } else {
        fs::create_dir_all(live).map_err(|e| format!("Couldn't make {}: {e}", live.display()))?;
    }
    fs::write(stash.join(OWNER_FILE), &want).map_err(|e| e.to_string())?;
    crate::log::info(&format!("Saves in {}: {now} → {want}", live.display()));
    crate::activity::add("saves", "Saves switched", Some(format!("{} now holds the {} saves; the {} ones wait beside it, untouched.", live.display(), if want == SHARED { "shared" } else { "profile's own" }, if now == SHARED { "shared" } else { "other profile's" })), None, Some("profiles"), None);
    Ok(true)
}

/// Whether two files or folders hold the same things (names, sizes and bytes).
fn same(a: &Path, b: &Path) -> bool {
    match (fs::metadata(a), fs::metadata(b)) {
        (Ok(x), Ok(y)) if x.is_file() && y.is_file() => x.len() == y.len() && fs::read(a).ok() == fs::read(b).ok(),
        (Ok(x), Ok(y)) if x.is_dir() && y.is_dir() => {
            let names = |d: &Path| -> Vec<std::ffi::OsString> {
                let mut v: Vec<_> = fs::read_dir(d).map(|r| r.flatten().map(|e| e.file_name()).collect()).unwrap_or_default();
                v.sort();
                v
            };
            let (na, nb) = (names(a), names(b));
            na == nb && na.iter().all(|n| same(&a.join(n), &b.join(n)))
        }
        _ => false,
    }
}

/// Save folders in a profile's own saves that are exact copies of the shared saves (Steam
/// Cloud puts them back into an emptied save folder). Only folders count: settings files the
/// game writes next to them stay.
pub fn copies(live: &Path) -> Vec<String> {
    if owner(live) == SHARED {
        return vec![];
    }
    let Some(shared) = stash_dir(live).map(|d| d.join(SHARED)) else { return vec![] };
    let mut out: Vec<String> = fs::read_dir(live)
        .map(|r| {
            r.flatten()
                .filter(|e| e.path().is_dir())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                // Exact copies, or empty folders named like a shared save (Steam Cloud makes the
                // folders first, and sometimes leaves them empty).
                .filter(|n| same(&live.join(n), &shared.join(n)) || (shared.join(n).is_dir() && fs::read_dir(live.join(n)).map(|mut d| d.next().is_none()).unwrap_or(false)))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

/// Move those copies out of the profile's saves, into `.zoms-nexus-saves/copies <time>/`
/// (nothing is deleted). Gives how many moved.
pub fn move_copies(live: &Path) -> Result<usize, String> {
    let list = copies(live);
    if list.is_empty() {
        return Ok(0);
    }
    let stash = stash_dir(live).ok_or("The save folder has no parent folder.")?;
    let dest = stash.join(format!("copies {}", crate::queue::now()));
    fs::create_dir_all(&dest).map_err(|e| format!("Couldn't make {}: {e}", dest.display()))?;
    for n in &list {
        fs::rename(live.join(n), dest.join(n)).map_err(|e| format!("Couldn't move {n}: {e}"))?;
    }
    crate::log::info(&format!("Moved {} copied saves out of {} into {}", list.len(), live.display(), dest.display()));
    Ok(list.len())
}

#[cfg(test)]
mod build_tests {
    use super::*;

    #[test]
    fn follows_the_installed_build() {
        let d = tempfile::tempdir().unwrap();
        let (install, pfx) = (d.path().join("Stardew Valley"), d.path().join("pfx"));
        fs::create_dir_all(&install).unwrap();
        let native = folder("stardewvalley", None, Some(&pfx), Some(&install)).unwrap();
        assert!(native.ends_with(".config/StardewValley/Saves"));
        fs::write(install.join("Stardew Valley.exe"), b"MZ").unwrap();
        let win = folder("stardewvalley", None, Some(&pfx), Some(&install)).unwrap();
        assert_eq!(win, pfx.join("drive_c/users/steamuser/AppData/Roaming/StardewValley/Saves"));
        assert_eq!(folder("stardewvalley", Some("/mine"), Some(&pfx), Some(&install)).unwrap(), PathBuf::from("/mine"), "yours wins");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_saves_without_losing_any() {
        let dir = tempfile::tempdir().unwrap();
        let live = dir.path().join("Saves");
        fs::create_dir_all(&live).unwrap();
        fs::write(live.join("shared.sav"), b"s").unwrap();

        // A profile with its own saves starts with an empty folder; the shared ones wait.
        assert!(switch(&live, "p1").unwrap());
        assert_eq!(owner(&live), "p1");
        assert!(fs::read_dir(&live).unwrap().next().is_none());
        fs::write(live.join("p1.sav"), b"1").unwrap();

        // Back to shared: the shared save is back, p1's waits.
        assert!(switch(&live, SHARED).unwrap());
        assert_eq!(fs::read(live.join("shared.sav")).unwrap(), b"s");
        assert!(!live.join("p1.sav").exists());

        // And p1 again: its save is back.
        switch(&live, "p1").unwrap();
        assert_eq!(fs::read(live.join("p1.sav")).unwrap(), b"1");
        assert!(!switch(&live, "p1").unwrap(), "already in place: nothing moves");

        // Something unexpected where p1's saves would go is kept, not replaced.
        switch(&live, SHARED).unwrap();
        let stray = dir.path().join(".zoms-nexus-saves/shared");
        fs::create_dir_all(&stray).unwrap();
        fs::write(stray.join("stray.sav"), b"x").unwrap();
        switch(&live, "p1").unwrap();
        let kept: Vec<_> = fs::read_dir(dir.path().join(".zoms-nexus-saves")).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert!(kept.iter().any(|n| n.starts_with("shared (kept ")), "{kept:?}");
        assert!(switch(&live, "../evil").is_err());

        // Copies of the shared saves in p1's folder (Steam Cloud) move out; p1's own stay.
        let shared = dir.path().join(".zoms-nexus-saves/shared");
        fs::create_dir_all(shared.join("AutoSave-0")).unwrap();
        fs::write(shared.join("AutoSave-0/sav.dat"), b"same").unwrap();
        fs::create_dir_all(live.join("AutoSave-0")).unwrap();
        fs::write(live.join("AutoSave-0/sav.dat"), b"same").unwrap();
        fs::create_dir_all(live.join("ManualSave-1")).unwrap();
        fs::write(live.join("ManualSave-1/sav.dat"), b"mine").unwrap();
        assert_eq!(copies(&live), vec!["AutoSave-0".to_string()]);
        assert_eq!(move_copies(&live).unwrap(), 1);
        assert!(!live.join("AutoSave-0").exists() && live.join("ManualSave-1").exists());
        assert!(copies(&live).is_empty());

        // An empty folder named like a shared save (Steam Cloud's placeholder) counts too; an
        // empty folder of its own name doesn't.
        fs::create_dir_all(shared.join("Farm_1")).unwrap();
        fs::write(shared.join("Farm_1/Farm_1"), b"farm").unwrap();
        fs::create_dir_all(live.join("Farm_1")).unwrap();
        fs::create_dir_all(live.join("NewFarm_2")).unwrap();
        assert_eq!(copies(&live), vec!["Farm_1".to_string()]);
    }
}
