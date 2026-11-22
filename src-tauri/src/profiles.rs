//! Saved profiles (`profiles.json` in the config folder) and mod packs.
//!
//! A mod pack is a profile written to a small file you can share: which mods and files, in
//! which order, by their Nexus Mods numbers. It never holds the mods themselves (their authors
//! share them on Nexus Mods, not us). Importing one makes the profile and queues its downloads.

use crate::deploy::{Profile, ProfileMod};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub const PACK_FORMAT: &str = "zoms-nexus-pack";

pub fn load(dir: &Path) -> Vec<Profile> {
    fs::read_to_string(dir.join("profiles.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(dir: &Path, all: &[Profile]) -> Result<(), String> {
    crate::settings::write_atomic(&dir.join("profiles.json"), &serde_json::to_vec_pretty(all).map_err(|e| e.to_string())?)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackMod {
    pub mod_id: u64,
    pub file_id: u64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub file_name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pack {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub game: String,
    #[serde(default)]
    pub description: Option<String>,
    pub mods: Vec<PackMod>,
}

pub fn to_pack(p: &Profile, names: impl Fn(u64, u64) -> (String, String)) -> Pack {
    Pack {
        format: PACK_FORMAT.into(),
        version: 1,
        name: p.name.clone(),
        game: p.game.clone(),
        description: None,
        mods: p
            .mods
            .iter()
            .map(|m| {
                let (name, file_name) = names(m.mod_id, m.file_id);
                PackMod { mod_id: m.mod_id, file_id: m.file_id, name, file_name, enabled: m.enabled }
            })
            .collect(),
    }
}

pub fn read_pack(text: &str) -> Result<Pack, String> {
    let p: Pack = serde_json::from_str(text).map_err(|_| "That file isn't a Zom's Nexus mod pack.".to_string())?;
    if p.format != PACK_FORMAT {
        return Err("That file isn't a Zom's Nexus mod pack.".into());
    }
    if p.version > 1 {
        return Err("That mod pack was made by a newer Zom's Nexus. Update the app to open it.".into());
    }
    if p.game.is_empty() || !p.game.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("The mod pack doesn't name a game Nexus Mods uses.".into());
    }
    if p.mods.len() > 5000 {
        return Err("That mod pack lists more mods than Zom's Nexus takes at once (5,000).".into());
    }
    Ok(p)
}

pub fn from_pack(p: &Pack, id: String) -> Profile {
    Profile {
        id,
        name: p.name.chars().take(80).collect(),
        game: p.game.clone(),
        mods: p.mods.iter().map(|m| ProfileMod { mod_id: m.mod_id, file_id: m.file_id, enabled: m.enabled }).collect(),
        collection: None,
        manual_requirements: false,
            new_downloads: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_go_out_and_come_back() {
        let p = Profile { id: "a".into(), name: "Winter".into(), game: "skyrim".into(), mods: vec![ProfileMod { mod_id: 7, file_id: 70, enabled: false }], collection: None, manual_requirements: false, new_downloads: false };
        let pack = to_pack(&p, |m, f| (format!("Mod {m}"), format!("f{f}.7z")));
        let text = serde_json::to_string(&pack).unwrap();
        let back = from_pack(&read_pack(&text).unwrap(), "b".into());
        assert_eq!(back.mods, p.mods);
        assert_eq!(back.name, "Winter");
        assert!(read_pack("{}").is_err());
        assert!(read_pack(&text.replace("\"skyrim\"", "\"../x\"")).is_err());
        assert!(read_pack(&text.replace("\"version\":1", "\"version\":2")).is_err());
    }
}
