//! Saved profiles (`profiles.json` in the config folder) and mod packs.
//!
//! A mod pack is a profile written to a small file you can share: which mods and files, in
//! which order, by their Nexus Mods numbers. It never holds the mods themselves (their authors
//! share them on Nexus Mods, not us). Importing one makes the profile and queues its downloads.

use crate::deploy::{Profile, ProfileMod};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub const PACK_FORMAT: &str = "zoms-modzilla-pack";

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
    /// For an add-on pack: the mods it adds. Otherwise the whole profile.
    pub mods: Vec<PackMod>,
    /// "addon" for an add-on pack (version 2); a whole profile otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The collection an add-on pack goes on top of.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collection: Option<PackCollection>,
    /// Collection mods an add-on pack switches off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub off: Vec<PackMod>,
    /// Collection mods an add-on pack puts on another of their files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<PackMod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackCollection {
    pub slug: String,
    #[serde(default)]
    pub revision: Option<u64>,
    #[serde(default)]
    pub name: String,
}

impl Pack {
    pub fn is_addon(&self) -> bool {
        self.kind.as_deref() == Some("addon")
    }
}

/// An add-on as a pack. `names` gives a mod's and a file's names for people reading the file.
pub fn addon_pack(game: &str, base: &crate::layers::Base, a: &crate::layers::Addon, names: impl Fn(u64, u64) -> (String, String)) -> Pack {
    let one = |mod_id: u64, file_id: u64, enabled: bool| {
        let (name, file_name) = names(mod_id, file_id);
        PackMod { mod_id, file_id, name, file_name, enabled }
    };
    let c = &a.changes;
    Pack {
        format: PACK_FORMAT.into(),
        version: 2,
        name: a.name.clone(),
        game: game.to_string(),
        description: None,
        mods: c.added.iter().filter(|m| !crate::library::is_local(m.mod_id)).map(|m| one(m.mod_id, m.file_id, m.enabled)).collect(),
        kind: Some("addon".into()),
        collection: Some(PackCollection { slug: base.slug.clone(), revision: base.revision, name: base.name.clone() }),
        off: c.off.iter().map(|id| one(*id, 0, false)).collect(),
        files: c.files.iter().map(|f| one(f.mod_id, f.file_id, true)).collect(),
    }
}

/// An add-on pack's changes. Mods are placed after the one before them in the pack.
pub fn addon_from_pack(p: &Pack, id: String) -> crate::layers::Addon {
    use crate::layers::{AddedMod, Addon, Changes, FilePick};
    Addon {
        id,
        name: p.name.chars().take(80).collect(),
        enabled: true,
        changes: Changes {
            added: p.mods.iter().enumerate().map(|(i, m)| AddedMod { mod_id: m.mod_id, file_id: m.file_id, enabled: m.enabled, after: i.checked_sub(1).map(|j| p.mods[j].mod_id) }).collect(),
            off: p.off.iter().map(|m| m.mod_id).collect(),
            files: p.files.iter().map(|m| FilePick { mod_id: m.mod_id, file_id: m.file_id }).collect(),
        },
    }
}

pub fn to_pack(p: &Profile, names: impl Fn(u64, u64) -> (String, String)) -> Pack {
    Pack {
        format: PACK_FORMAT.into(),
        version: 1,
        name: p.name.clone(),
        game: p.game.clone(),
        description: None,
        kind: None,
        collection: None,
        off: vec![],
        files: vec![],
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
    let p: Pack = serde_json::from_str(text).map_err(|_| "That file isn't a Zom's Modzilla mod pack.".to_string())?;
    if p.format != PACK_FORMAT && p.format != crate::legacy::OLD_PACK {
        return Err("That file isn't a Zom's Modzilla mod pack.".into());
    }
    if p.version > 2 {
        return Err("That mod pack was made by a newer Zom's Modzilla. Update the app to open it.".into());
    }
    if p.game.is_empty() || !p.game.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("The mod pack doesn't name a game Nexus Mods uses.".into());
    }
    if p.is_addon() && !p.collection.as_ref().is_some_and(|c| !c.slug.is_empty() && c.slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')) {
        return Err("That add-on pack doesn't say which collection it goes on.".into());
    }
    if p.mods.len() + p.off.len() + p.files.len() > 5000 {
        return Err("That mod pack lists more mods than Zom's Modzilla takes at once (5,000).".into());
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
            own_saves: false,
            base: None,
            addons: vec![],
            rules: vec![],
            note: None,
        ts_pack: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_go_out_and_come_back() {
        let p = Profile { id: "a".into(), name: "Winter".into(), game: "skyrim".into(), mods: vec![ProfileMod { mod_id: 7, file_id: 70, enabled: false }], collection: None, manual_requirements: false, new_downloads: false, own_saves: false, base: None, addons: vec![], rules: vec![], note: None, ts_pack: None };
        let pack = to_pack(&p, |m, f| (format!("Mod {m}"), format!("f{f}.7z")));
        let text = serde_json::to_string(&pack).unwrap();
        let back = from_pack(&read_pack(&text).unwrap(), "b".into());
        assert_eq!(back.mods, p.mods);
        assert_eq!(back.name, "Winter");
        assert!(read_pack("{}").is_err());
        assert!(read_pack(&text.replace("\"skyrim\"", "\"../x\"")).is_err());
        assert!(read_pack(&text.replace("\"version\":1", "\"version\":3")).is_err());
    }

    #[test]
    fn add_on_packs_go_out_and_come_back() {
        use crate::layers::*;
        let base = Base { slug: "abc123".into(), revision: Some(94), name: "Fem V".into(), optional: false, files: vec![] };
        let a = Addon { id: "a".into(), name: "Extras".into(), enabled: true, changes: Changes { added: vec![AddedMod { mod_id: 9, file_id: 90, enabled: true, after: Some(1) }, AddedMod { mod_id: 8, file_id: 80, enabled: false, after: Some(9) }], off: vec![3], files: vec![FilePick { mod_id: 2, file_id: 21 }] } };
        let pack = addon_pack("cyberpunk2077", &base, &a, |m, _| (format!("Mod {m}"), String::new()));
        let back = read_pack(&serde_json::to_string(&pack).unwrap()).unwrap();
        assert!(back.is_addon());
        assert_eq!(back.collection.as_ref().unwrap().revision, Some(94));
        let b = addon_from_pack(&back, "b".into());
        assert_eq!(b.changes.off, vec![3]);
        assert_eq!(b.changes.files, a.changes.files);
        assert_eq!(b.changes.added[1].after, Some(9));
        assert!(!b.changes.added[1].enabled);
        let mut bad = pack.clone();
        bad.collection = None;
        assert!(read_pack(&serde_json::to_string(&bad).unwrap()).is_err());
    }
}
