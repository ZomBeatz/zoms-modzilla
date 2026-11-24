//! Save snapshots: a copy of a game's saves (the ones in its save folder now) packed into a zip
//! in `<data>/save-snapshots/<game>/`, before trying risky mods or by themselves before each
//! Play. Restoring one first snapshots the saves there now, then moves them aside beside the
//! other saves (`.zoms-modzilla-saves/replaced <time>`), never deleting them, and unpacks the
//! snapshot in their place.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub id: String,
    pub at: u64,
    pub label: String,
    /// Whose saves they were ("shared", or a profile id).
    pub owner: String,
    pub files: usize,
    /// Size of the zip.
    pub size: u64,
    /// Taken by itself (before Play, before a restore): the oldest of these go first.
    #[serde(default)]
    pub auto: bool,
}

/// Snapshots taken by themselves are kept this many per game.
pub const KEEP_AUTO: usize = 10;

fn dir(data: &Path, game: &str) -> PathBuf {
    data.join("save-snapshots").join(crate::library::safe_name(game))
}

fn walk(base: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        for e in fs::read_dir(base.join(&rel)).into_iter().flatten().flatten() {
            let Ok(t) = e.file_type() else { continue };
            let r = rel.join(e.file_name());
            if t.is_dir() {
                stack.push(r);
            } else if t.is_file() {
                out.push(r);
            }
        }
    }
    out.sort();
    out
}

pub fn list(data: &Path, game: &str) -> Vec<Snapshot> {
    let mut out: Vec<Snapshot> = fs::read_dir(dir(data, game))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| serde_json::from_slice(&fs::read(e.path()).ok()?).ok())
        .collect();
    // Newest first; ones taken in the same second by their number ("<time>-2" came after "<time>").
    let order = |s: &Snapshot| (s.at, s.id.rsplit_once('-').and_then(|(_, n)| n.parse::<u32>().ok()).unwrap_or(1));
    out.sort_by_key(|s| std::cmp::Reverse(order(s)));
    out
}

/// Pack the saves in `live` into a new snapshot.
pub fn take(data: &Path, game: &str, live: &Path, owner: &str, label: &str, auto: bool) -> Result<Snapshot, String> {
    let files = walk(live);
    if files.is_empty() {
        return Err("There are no saves in the save folder to snapshot.".into());
    }
    let d = dir(data, game);
    fs::create_dir_all(&d).map_err(|e| e.to_string())?;
    let size: u64 = files.iter().map(|f| fs::metadata(live.join(f)).map(|m| m.len()).unwrap_or(0)).sum();
    crate::library::ensure_space(&d, size, "keeping a snapshot of the saves")?;
    let at = crate::queue::now();
    let mut id = format!("{at}");
    let mut n = 1;
    while d.join(format!("{id}.zip")).exists() {
        n += 1;
        id = format!("{at}-{n}");
    }
    let zip_path = d.join(format!("{id}.zip"));
    let tmp = d.join(format!(".{id}.zip.part"));
    {
        let f = fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut z = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).large_file(true);
        for rel in &files {
            let name = rel.to_string_lossy().replace('\\', "/");
            z.start_file(name, opts).map_err(|e| e.to_string())?;
            let mut src = fs::File::open(live.join(rel)).map_err(|e| format!("Couldn't read {}: {e}", rel.display()))?;
            std::io::copy(&mut src, &mut z).map_err(|e| e.to_string())?;
        }
        z.finish().map_err(|e| e.to_string())?;
    }
    fs::rename(&tmp, &zip_path).map_err(|e| e.to_string())?;
    let s = Snapshot { id: id.clone(), at, label: label.chars().take(120).collect(), owner: owner.to_string(), files: files.len(), size: fs::metadata(&zip_path).map(|m| m.len()).unwrap_or(0), auto };
    fs::write(d.join(format!("{id}.json")), serde_json::to_vec_pretty(&s).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    if auto {
        prune(data, game);
    }
    Ok(s)
}

/// Keep the newest `KEEP_AUTO` snapshots taken by themselves; ones you took stay.
fn prune(data: &Path, game: &str) {
    let d = dir(data, game);
    for old in list(data, game).into_iter().filter(|s| s.auto).skip(KEEP_AUTO) {
        let _ = fs::remove_file(d.join(format!("{}.zip", old.id)));
        let _ = fs::remove_file(d.join(format!("{}.json", old.id)));
    }
}

fn safe_id(id: &str) -> Result<&str, String> {
    (!id.is_empty() && id.chars().all(|c| c.is_ascii_digit() || c == '-')).then_some(id).ok_or_else(|| "That isn't a snapshot.".to_string())
}

pub fn delete(data: &Path, game: &str, id: &str) -> Result<(), String> {
    let id = safe_id(id)?;
    let d = dir(data, game);
    fs::remove_file(d.join(format!("{id}.zip"))).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(d.join(format!("{id}.json")));
    Ok(())
}

/// Put a snapshot's saves back: the saves there now are snapshotted and moved aside first.
pub fn restore(data: &Path, game: &str, live: &Path, owner: &str, id: &str) -> Result<Snapshot, String> {
    let id = safe_id(id)?;
    let zip_path = dir(data, game).join(format!("{id}.zip"));
    let snap = list(data, game).into_iter().find(|s| s.id == id).ok_or("That snapshot is gone.")?;
    let mut z = zip::ZipArchive::new(fs::File::open(&zip_path).map_err(|e| e.to_string())?).map_err(|e| format!("The snapshot didn't read: {e}"))?;
    // What's there now: a snapshot of it, then aside (never deleted).
    if !walk(live).is_empty() {
        take(data, game, live, owner, &format!("Before restoring “{}”", snap.label), true)?;
        let parent = live.parent().ok_or("The save folder has no parent folder.")?;
        let aside = parent.join(".zoms-modzilla-saves").join(format!("replaced {}", crate::queue::now()));
        fs::create_dir_all(aside.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::rename(live, &aside).map_err(|e| format!("Couldn't move the saves there now aside: {e}"))?;
    }
    fs::create_dir_all(live).map_err(|e| e.to_string())?;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = f.enclosed_name() else { continue };
        if f.is_dir() {
            continue;
        }
        let to = live.join(&rel);
        if let Some(p) = to.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&to).map_err(|e| e.to_string())?;
        let most = f.size();
        let mut buf = vec![0u8; 1 << 16];
        let mut done = 0u64;
        loop {
            let n = f.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            done += n as u64;
            if done > most {
                return Err("The snapshot is damaged (a file is bigger than it says).".into());
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
    }
    Ok(snap)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn takes_restores_and_never_deletes() {
        let t = tempfile::tempdir().unwrap();
        let data = t.path().join("data");
        let live = t.path().join("game/Saves");
        fs::create_dir_all(live.join("slot1")).unwrap();
        fs::write(live.join("slot1/save.sav"), "before mods").unwrap();
        let s = take(&data, "g", &live, "shared", "Before trying Big Overhaul", false).unwrap();
        assert_eq!((s.files, s.auto), (1, false));
        // Play happens; the save changes.
        fs::write(live.join("slot1/save.sav"), "broken by mods").unwrap();
        fs::write(live.join("new.sav"), "x").unwrap();
        restore(&data, "g", &live, "shared", &s.id).unwrap();
        assert_eq!(fs::read_to_string(live.join("slot1/save.sav")).unwrap(), "before mods");
        assert!(!live.join("new.sav").exists());
        // What was there is kept aside, and snapshotted.
        let aside: Vec<_> = fs::read_dir(t.path().join("game/.zoms-modzilla-saves")).unwrap().flatten().collect();
        assert_eq!(aside.len(), 1);
        assert_eq!(fs::read_to_string(aside[0].path().join("slot1/save.sav")).unwrap(), "broken by mods");
        let all = list(&data, "g");
        assert_eq!(all.len(), 2);
        assert!(all[0].auto && all[0].label.starts_with("Before restoring"));
        assert!(restore(&data, "g", &live, "shared", "../x").is_err());
        // Ones taken by themselves are kept to KEEP_AUTO.
        for _ in 0..KEEP_AUTO + 3 {
            take(&data, "g", &live, "shared", "auto", true).unwrap();
        }
        assert_eq!(list(&data, "g").iter().filter(|s| s.auto).count(), KEEP_AUTO);
        assert!(list(&data, "g").iter().any(|x| x.id == s.id), "yours stay");
        assert!(take(&data, "g", &t.path().join("empty"), "shared", "x", false).is_err());
    }
}
