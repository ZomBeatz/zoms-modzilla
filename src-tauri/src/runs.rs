//! Each time a profile is played: which mod files were in, and how long the game ran. A game
//! that closes within a minute of starting almost always means a mod stopped it, so the last such
//! run is compared with the last one that ran fine, which shows what changed in between. Works
//! for any game, with no logs needed. Kept in `<data>/runs/<profile>.json`, the last 20.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

const KEEP: usize = 20;
/// Closed this soon after starting (and not by Stop): something stopped it.
pub const QUICK: u64 = 60;
/// Ran this long: the mods were fine.
pub const GOOD: u64 = 5 * 60;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub start: u64,
    pub secs: u64,
    /// Stopped from Zom's Modzilla: says nothing about the mods.
    #[serde(default)]
    pub stopped: bool,
    /// (mod number, file number) of each switched-on mod.
    pub mods: Vec<(u64, u64)>,
}

impl Run {
    pub fn quick(&self) -> bool {
        !self.stopped && self.secs < QUICK
    }
    pub fn good(&self) -> bool {
        self.secs >= GOOD
    }
}

/// A mod that changed between the run that went fine and the one that didn't.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Changed {
    pub mod_id: u64,
    pub name: String,
}

/// The last run closed quickly: when, and what changed since the last run that went fine.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Trouble {
    pub at: u64,
    pub secs: u64,
    /// How many runs in a row closed quickly.
    pub times: usize,
    /// The last run that went fine, when there was one.
    pub good_at: Option<u64>,
    pub good_secs: Option<u64>,
    pub added: Vec<Changed>,
    pub removed: Vec<Changed>,
    pub updated: Vec<Changed>,
    /// The profile's mods changed since the quick exit: maybe fixed already.
    pub changed_since: bool,
}

static PENDING: Mutex<Vec<(String, Vec<(u64, u64)>)>> = Mutex::new(Vec::new());

fn path(data: &Path, profile: &str) -> PathBuf {
    data.join("runs").join(format!("{}.json", crate::library::safe_name(profile)))
}

pub fn list(data: &Path, profile: &str) -> Vec<Run> {
    fs::read(path(data, profile)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

/// The mods going into the game for `profile`, noted at Play for when the game closes.
pub fn begin(profile: &str, mods: Vec<(u64, u64)>) {
    let mut p = PENDING.lock().unwrap();
    p.retain(|(id, _)| id != profile);
    p.push((profile.to_string(), mods));
}

/// The game closed: note the run, if Play noted its mods.
pub fn finish(data: &Path, profile: &str, start: u64, secs: u64, stopped: bool) -> Option<Run> {
    let mods = {
        let mut p = PENDING.lock().unwrap();
        let i = p.iter().position(|(id, _)| id == profile)?;
        p.remove(i).1
    };
    let run = Run { start, secs, stopped, mods };
    let mut all = list(data, profile);
    all.push(run.clone());
    let skip = all.len().saturating_sub(KEEP);
    all.drain(..skip);
    let file = path(data, profile);
    let _ = fs::create_dir_all(file.parent().unwrap());
    let _ = fs::write(&file, serde_json::to_vec_pretty(&all).unwrap_or_default());
    Some(run)
}

/// What to say about the last run, when it closed quickly. `now` is the profile's switched-on
/// mods; `names` names mods by number.
pub fn trouble(runs: &[Run], now: &[(u64, u64)], names: &HashMap<u64, String>) -> Option<Trouble> {
    let played: Vec<&Run> = runs.iter().filter(|r| !r.stopped).collect();
    let last = *played.last()?;
    if !last.quick() {
        return None;
    }
    let times = played.iter().rev().take_while(|r| r.quick()).count();
    let good = played.iter().rev().find(|r| r.good());
    let name = |id: u64| Changed { mod_id: id, name: names.get(&id).cloned().unwrap_or_else(|| format!("Mod {id}")) };
    let (mut added, mut removed, mut updated) = (vec![], vec![], vec![]);
    if let Some(g) = good {
        let before: HashMap<u64, u64> = g.mods.iter().copied().collect();
        let after: HashMap<u64, u64> = last.mods.iter().copied().collect();
        for (id, file) in &last.mods {
            match before.get(id) {
                None => added.push(name(*id)),
                Some(f) if f != file => updated.push(name(*id)),
                _ => {}
            }
        }
        removed = g.mods.iter().filter(|(id, _)| !after.contains_key(id)).map(|(id, _)| name(*id)).collect();
    }
    let mut a: Vec<(u64, u64)> = last.mods.clone();
    let mut b: Vec<(u64, u64)> = now.to_vec();
    a.sort();
    b.sort();
    Some(Trouble { at: last.start, secs: last.secs, times, good_at: good.map(|g| g.start), good_secs: good.map(|g| g.secs), added, removed, updated, changed_since: a != b })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quick_exit_is_compared_with_the_last_good_run() {
        let dir = tempfile::tempdir().unwrap();
        begin("p", vec![(1, 10), (2, 20)]);
        finish(dir.path(), "p", 100, 3600, false);
        begin("p", vec![(1, 10), (2, 21), (3, 30)]);
        finish(dir.path(), "p", 9000, 30, true); // Stopped: doesn't count.
        begin("p", vec![(1, 10), (2, 21), (3, 30)]);
        finish(dir.path(), "p", 9100, 12, false);
        assert!(finish(dir.path(), "p", 9200, 5, false).is_none(), "nothing noted at Play");
        let runs = list(dir.path(), "p");
        assert_eq!(runs.len(), 3);
        let names = HashMap::from([(3, "New Thing".to_string())]);
        let t = trouble(&runs, &[(1, 10), (2, 21), (3, 30)], &names).unwrap();
        assert_eq!((t.at, t.secs, t.times, t.good_at), (9100, 12, 1, Some(100)));
        assert_eq!(t.added, vec![Changed { mod_id: 3, name: "New Thing".into() }]);
        assert_eq!(t.updated.iter().map(|c| c.mod_id).collect::<Vec<_>>(), vec![2]);
        assert!(t.removed.is_empty() && !t.changed_since);
        assert!(trouble(&runs, &[(1, 10)], &names).unwrap().changed_since);
        // Once it runs fine, nothing to say.
        begin("p", vec![(1, 10)]);
        finish(dir.path(), "p", 9300, 900, false);
        assert!(trouble(&list(dir.path(), "p"), &[(1, 10)], &names).is_none());
    }
}
