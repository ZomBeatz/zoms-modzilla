//! Backing up what Zom's Nexus knows (not the mods themselves): profiles with their rules,
//! add-ons and notes, your notes and tags on mods, restore points, time played, settings, and
//! the curators' installer choices. One JSON file, to keep before reinstalling or to take to
//! another computer. Never the API key, the download list, or what's put in games right now
//! (that belongs to this computer's game folders).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

/// Files kept, by where they live: "config/…" or "data/…". Only these are ever written back.
const CONFIG: &[&str] = &["settings.json", "profiles.json", "notes.json", "mod-tags.json"];
const DATA: &[&str] = &["playtime.json", "ignored.json", "collection-choices.json"];
const MAX: u64 = 50 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
pub struct Backup {
    pub app: String,
    pub made: u64,
    /// "config/profiles.json" → its contents.
    pub files: BTreeMap<String, Value>,
}

fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(p).ok()?).ok()
}

pub fn make(config: &Path, data: &Path, now: u64) -> Backup {
    let mut files = BTreeMap::new();
    for f in CONFIG {
        if let Some(v) = read_json(&config.join(f)) {
            files.insert(format!("config/{f}"), v);
        }
    }
    for f in DATA {
        if let Some(v) = read_json(&data.join(f)) {
            files.insert(format!("data/{f}"), v);
        }
    }
    for e in fs::read_dir(data.join("history")).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.ends_with(".json") {
            if let Some(v) = read_json(&e.path()) {
                files.insert(format!("data/history/{name}"), v);
            }
        }
    }
    Backup { app: format!("Zom's Nexus {}", crate::nexus::APP_VERSION), made: now, files }
}

/// Whether a name in a backup is one Zom's Nexus writes back (and so where).
fn place(name: &str, config: &Path, data: &Path) -> Option<std::path::PathBuf> {
    if let Some(f) = name.strip_prefix("config/") {
        return CONFIG.contains(&f).then(|| config.join(f));
    }
    if let Some(f) = name.strip_prefix("data/history/") {
        let ok = f.ends_with(".json") && f.len() < 80 && f[..f.len() - 5].chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        return ok.then(|| data.join("history").join(f));
    }
    let f = name.strip_prefix("data/")?;
    DATA.contains(&f).then(|| data.join(f))
}

pub fn read(path: &Path) -> Result<Backup, String> {
    if fs::metadata(path).map(|m| m.len()).unwrap_or(0) > MAX {
        return Err("That file is too big to be a Zom's Nexus backup.".into());
    }
    let b: Backup = serde_json::from_str(&fs::read_to_string(path).map_err(|e| format!("Couldn't read that file: {e}"))?).map_err(|_| "That isn't a Zom's Nexus backup.".to_string())?;
    if !b.app.starts_with("Zom's Nexus") {
        return Err("That isn't a Zom's Nexus backup.".into());
    }
    Ok(b)
}

/// Write a backup's files back. Settings keep this computer's library folder, per-game folders,
/// key storage and autostart (those belong to the computer, not to you). Gives what was put back.
pub fn restore(b: &Backup, config: &Path, data: &Path, keep: &crate::settings::Settings) -> Result<Vec<String>, String> {
    let mut done = vec![];
    for (name, v) in &b.files {
        let Some(to) = place(name, config, data) else { continue };
        let v = if name == "config/settings.json" {
            let mut s: crate::settings::Settings = serde_json::from_value(v.clone()).map_err(|e| format!("The backup's settings couldn't be read: {e}"))?;
            s.library_dir = keep.library_dir.clone();
            s.game_libraries = keep.game_libraries.clone();
            // Where the key is and the autostart entry belong to this computer too.
            s.key_in_keyring = keep.key_in_keyring;
            s.start_with_computer = keep.start_with_computer;
            serde_json::to_value(s.clamp()).map_err(|e| e.to_string())?
        } else {
            v.clone()
        };
        if let Some(p) = to.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        crate::settings::write_atomic(&to, &serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?)?;
        done.push(name.clone());
    }
    Ok(done)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backs_up_and_restores_only_known_files() {
        let d = tempfile::tempdir().unwrap();
        let (c, dd) = (d.path().join("c"), d.path().join("d"));
        fs::create_dir_all(c.join("x")).unwrap();
        fs::create_dir_all(dd.join("history")).unwrap();
        fs::write(c.join("profiles.json"), "[]").unwrap();
        fs::write(c.join("api-key"), "secret").unwrap();
        fs::write(dd.join("history/skyrim.json"), "[]").unwrap();
        fs::write(dd.join("queue.json"), "[]").unwrap();
        let b = make(&c, &dd, 1);
        assert_eq!(b.files.keys().cloned().collect::<Vec<_>>(), vec!["config/profiles.json", "data/history/skyrim.json"]);
        assert!(!serde_json::to_string(&b).unwrap().contains("secret"));

        let mut evil = make(&c, &dd, 2);
        evil.files.insert("config/../../escape.json".into(), Value::Null);
        evil.files.insert("data/history/../../x.json".into(), Value::Null);
        evil.files.insert("config/api-key".into(), Value::Null);
        let done = restore(&evil, &c, &dd, &crate::settings::Settings::default()).unwrap();
        assert_eq!(done, vec!["config/profiles.json", "data/history/skyrim.json"]);
        assert!(!d.path().join("escape.json").exists());
    }
}
