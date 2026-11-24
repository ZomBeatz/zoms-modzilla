//! Games that were uninstalled: Zom's Modzilla remembers where each game it has seen lives, and when
//! a game's folder is gone while the folder that held it is still there (so not an unplugged
//! drive), it offers to delete that game's mods from the library. Kept in `known-games.json` in
//! the data folder.

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Known {
    pub name: String,
    pub path: String,
    /// The person chose to keep its mods; not asked again until it's installed again.
    #[serde(default)]
    pub kept: bool,
}

pub type Seen = BTreeMap<String, Known>;

fn file(data: &Path) -> std::path::PathBuf {
    data.join("known-games.json")
}

pub fn load(data: &Path) -> Seen {
    fs::read_to_string(file(data)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(data: &Path, s: &Seen) -> Result<(), String> {
    fs::write(file(data), serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Whether a game that lived at `path` was uninstalled: its folder is gone but the one that held
/// it (Steam's `common` folder, say) is still there.
pub fn uninstalled(path: &Path) -> bool {
    !path.exists() && path.parent().is_some_and(|p| p.is_dir())
}

/// Note the games installed now (name, folder). Gives whether anything changed. A game seen
/// installed again forgets "keep its mods".
pub fn note(seen: &mut Seen, installed: &[(String, String, String)]) -> bool {
    let mut changed = false;
    for (domain, name, path) in installed {
        let k = Known { name: name.clone(), path: path.clone(), kept: false };
        if seen.get(domain) != Some(&k) {
            seen.insert(domain.clone(), k);
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unplugged_drives_dont_count() {
        let d = tempfile::tempdir().unwrap();
        let common = d.path().join("steamapps/common");
        fs::create_dir_all(common.join("Game A")).unwrap();
        assert!(!uninstalled(&common.join("Game A")), "still there");
        fs::remove_dir(common.join("Game A")).unwrap();
        assert!(uninstalled(&common.join("Game A")), "folder gone, library still there");
        assert!(!uninstalled(&d.path().join("unplugged/steamapps/common/Game B")), "the whole drive is missing");

        let mut s = Seen::new();
        assert!(note(&mut s, &[("a".into(), "A".into(), "/x".into())]));
        assert!(!note(&mut s, &[("a".into(), "A".into(), "/x".into())]));
        s.get_mut("a").unwrap().kept = true;
        assert!(note(&mut s, &[("a".into(), "A".into(), "/x".into())]), "seen again: asked again next time");
    }
}
