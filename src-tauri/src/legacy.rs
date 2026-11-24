//! Zom's Modzilla was called Zom's Nexus before its first release. A computer that ran it under
//! the old name keeps everything: the folders move to the new ones on the first start, and the
//! marks and backups it left under the old names are picked up where they are.

use std::{
    fs,
    path::{Path, PathBuf},
};

pub const OLD_ID: &str = "com.zombeatz.zoms-nexus";
pub const OLD_BACKUP: &str = ".zoms-nexus-backup";
pub const OLD_INSTALLED: &str = ".zoms-nexus-installed";
pub const OLD_GAME_ROOT: &str = ".zoms-nexus-game-folder";
pub const OLD_SAVES: &str = ".zoms-nexus-saves";
pub const OLD_PACK: &str = "zoms-nexus-pack";
pub const OLD_SHARE: &str = "ZN1-";
pub const OLD_SECRET: [&str; 4] = ["application", "zoms-nexus", "kind", "api-key"];
/// The desktop files the old name made (nxm links, start with the computer, the dev build).
pub const OLD_NXM_DESKTOP: &str = "com.zombeatz.zoms-nexus-nxm.desktop";
pub const OLD_DESKTOP: &str = "zoms-nexus.desktop";
/// What marked the old name's own dev desktop file.
pub const OLD_DEV_MARK: &str = "X-Zoms-Nexus-Dev=true";

/// `new` takes `old`'s place when only the old one is there. True when it moved.
pub fn adopt(old: &Path, new: &Path) -> bool {
    if new.exists() || !old.exists() {
        return false;
    }
    if let Some(p) = new.parent() {
        let _ = fs::create_dir_all(p);
    }
    fs::rename(old, new).is_ok()
}

/// The old name's folder beside a new one (`.../com.zombeatz.zoms-nexus` next to
/// `.../com.zombeatz.zoms-modzilla`).
pub fn old_beside(new: &Path) -> Option<PathBuf> {
    let name = new.file_name()?.to_string_lossy().into_owned();
    (name != OLD_ID && name.starts_with("com.zombeatz.")).then(|| new.with_file_name(OLD_ID))
}

/// The config, data and cache folders, moved from the old name on the first start. True when the
/// data folder moved (so the rest of the old name's things are tidied up once).
pub fn move_folders(config: &Path, data: &Path, cache: &Path) -> bool {
    for d in [config, cache] {
        if let Some(old) = old_beside(d) {
            adopt(&old, d);
        }
    }
    let moved = old_beside(data).is_some_and(|old| adopt(&old, data));
    if moved {
        let _ = fs::rename(data.join("zoms-nexus.log"), data.join("zoms-modzilla.old-name.log"));
    }
    moved
}

/// Where the desktop keeps each kind of folder (what Tauri's app_*_dir use), for this app's id.
fn xdg(var: &str, fallback: &str) -> Option<PathBuf> {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(fallback)))
}

/// The first thing at start, before any window (which makes the data folder): the old name's
/// folders move over. A mark in the data folder tells the start-up to tidy the rest.
pub fn move_early(id: &str) {
    let (Some(c), Some(d), Some(k)) = (xdg("XDG_CONFIG_HOME", ".config"), xdg("XDG_DATA_HOME", ".local/share"), xdg("XDG_CACHE_HOME", ".cache")) else { return };
    let data = d.join(id);
    if move_folders(&c.join(id), &data, &k.join(id)) {
        let _ = fs::write(data.join(RENAMED), "");
    }
}

pub const RENAMED: &str = ".renamed-from-zoms-nexus";

/// A game's backup folder: the old name's is renamed when that's the one there.
pub fn backup_dir(root: &Path) -> PathBuf {
    let new = root.join(crate::deploy::BACKUP);
    adopt(&root.join(OLD_BACKUP), &new);
    new
}

/// A save folder's stash of other profiles' saves, likewise.
pub fn saves_stash(parent: &Path) -> PathBuf {
    let new = parent.join(crate::saves::STASH);
    adopt(&parent.join(OLD_SAVES), &new);
    new
}

/// Built folders (installer choices, loaders for the game folder) in the library carry marks
/// under the old name: they get the new ones.
pub fn rename_marks(mod_dirs: &[PathBuf]) -> usize {
    let mut n = 0;
    for m in mod_dirs {
        for e in fs::read_dir(m).into_iter().flatten().flatten() {
            let d = e.path();
            if !d.is_dir() {
                continue;
            }
            for (old, new) in [(OLD_INSTALLED, crate::fomod::MARK), (OLD_GAME_ROOT, crate::deploy::GAME_ROOT_MARK)] {
                if adopt(&d.join(old), &d.join(new)) {
                    n += 1;
                }
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_the_old_names_things() {
        let t = tempfile::tempdir().unwrap();
        let (cfg, data, cache) = (t.path().join("config/com.zombeatz.zoms-modzilla"), t.path().join("share/com.zombeatz.zoms-modzilla"), t.path().join("cache/com.zombeatz.zoms-modzilla"));
        fs::create_dir_all(t.path().join("config").join(OLD_ID)).unwrap();
        fs::write(t.path().join("config").join(OLD_ID).join("profiles.json"), "[]").unwrap();
        fs::create_dir_all(t.path().join("share").join(OLD_ID)).unwrap();
        fs::write(t.path().join("share").join(OLD_ID).join("zoms-nexus.log"), "x").unwrap();
        assert!(move_folders(&cfg, &data, &cache));
        assert!(cfg.join("profiles.json").exists() && data.join("zoms-modzilla.old-name.log").exists());
        assert!(!move_folders(&cfg, &data, &cache), "only once");
        // A game folder with the old backups.
        let game = t.path().join("game");
        fs::create_dir_all(game.join(OLD_BACKUP).join("Data")).unwrap();
        assert!(backup_dir(&game).join("Data").is_dir());
        // A built folder in the library.
        let m = t.path().join("lib/g/1 - Mod");
        fs::create_dir_all(m.join("built")).unwrap();
        fs::write(m.join("built").join(OLD_INSTALLED), "").unwrap();
        fs::write(m.join("built").join(OLD_GAME_ROOT), "").unwrap();
        assert_eq!(rename_marks(&[m.clone()]), 2);
        assert!(m.join("built").join(crate::fomod::MARK).is_file());
    }
}
