//! Restore points: each time a profile is put into a game, the list of mods that went in is
//! noted (when it changed since the last time), so "go back to how it was on Tuesday" is one
//! click. Kept per game in `history/<game>.json` in the data folder, the newest 30.

use crate::deploy::{Profile, ProfileMod};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

const KEEP: usize = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    /// When the mods went in (Unix time); also the point's name.
    pub at: u64,
    pub profile_id: String,
    pub profile_name: String,
    pub mods: Vec<ProfileMod>,
    #[serde(default)]
    pub rules: Vec<crate::order::Rule>,
    /// Why it changed, in your words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn file(data: &Path, game: &str) -> std::path::PathBuf {
    data.join("history").join(format!("{}.json", crate::library::safe_name(game)))
}

/// Newest first.
pub fn list(data: &Path, game: &str) -> Vec<Point> {
    fs::read_to_string(file(data, game)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// Note what just went in, unless it's the same as the newest point for this profile.
pub fn note(data: &Path, p: &Profile, at: u64) -> Result<bool, String> {
    let mut all = list(data, &p.game);
    if all.iter().find(|x| x.profile_id == p.id).is_some_and(|x| x.mods == p.mods && x.rules == p.rules) {
        return Ok(false);
    }
    all.insert(0, Point { at, profile_id: p.id.clone(), profile_name: p.name.clone(), mods: p.mods.clone(), rules: p.rules.clone(), note: None });
    all.truncate(KEEP);
    let f = file(data, &p.game);
    fs::create_dir_all(f.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(&f, serde_json::to_vec_pretty(&all).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Add or change the note on a restore point.
pub fn set_note(data: &Path, game: &str, at: u64, note: &str) -> Result<(), String> {
    let mut all = list(data, game);
    let p = all.iter_mut().find(|x| x.at == at).ok_or("That restore point isn't there any more.")?;
    let n = note.trim().chars().take(1000).collect::<String>();
    p.note = (!n.is_empty()).then_some(n);
    fs::write(file(data, game), serde_json::to_vec_pretty(&all).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_only_changes_and_keeps_the_newest() {
        let d = tempfile::tempdir().unwrap();
        let mut p = Profile { id: "p1".into(), name: "A".into(), game: "g".into(), mods: vec![ProfileMod { mod_id: 1, file_id: 10, enabled: true }], collection: None, manual_requirements: false, new_downloads: false, own_saves: false, base: None, addons: vec![], rules: vec![], note: None, ts_pack: None };
        assert!(note(d.path(), &p, 1).unwrap());
        assert!(!note(d.path(), &p, 2).unwrap(), "same mods: nothing new");
        p.mods[0].enabled = false;
        assert!(note(d.path(), &p, 3).unwrap());
        assert_eq!(list(d.path(), "g").iter().map(|x| x.at).collect::<Vec<_>>(), vec![3, 1]);
        for i in 0..40 {
            p.mods[0].file_id = 100 + i;
            note(d.path(), &p, 10 + i).unwrap();
        }
        assert_eq!(list(d.path(), "g").len(), KEEP);
    }
}
