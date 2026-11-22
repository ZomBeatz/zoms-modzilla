//! Newer versions of the files in the library.
//!
//! When an author uploads a new version on Nexus Mods, the mod's file list says which old file
//! it replaces. Following that chain from a file you have leads to its newest version. One API
//! call per mod; a few mods are checked at the same time.

use crate::{
    library::StoredMod,
    nexus::{FileInfo, Nexus},
};
use futures_util::{stream, StreamExt};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub game: String,
    pub mod_id: u64,
    pub mod_name: String,
    /// The file you have.
    pub file_id: u64,
    /// Its newest version, when there is one.
    pub newer: Option<FileInfo>,
    /// Nexus Mods marks the file you have as an old version (or took it down), with no direct
    /// replacement named.
    pub outdated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub updates: Vec<Update>,
    pub checked: usize,
    /// Mods that couldn't be checked, with why.
    pub problems: Vec<(String, String)>,
    pub at: u64,
}

/// Follow "old file → new file" links from `from` to the end. Where one file was replaced by
/// several, the newest (highest number) wins.
pub fn newest(from: u64, links: &[(u64, u64)]) -> u64 {
    let mut cur = from;
    for _ in 0..100 {
        match links.iter().filter(|(o, _)| *o == cur).map(|(_, n)| *n).max() {
            Some(n) if n != cur => cur = n,
            _ => break,
        }
    }
    cur
}

pub fn compare(m: &StoredMod, files: &[FileInfo], links: &[(u64, u64)]) -> Vec<Update> {
    let have: Vec<u64> = m.files.iter().map(|f| f.file_id).collect();
    let mut out = vec![];
    for f in &m.files {
        let latest = newest(f.file_id, links);
        let newer = (latest != f.file_id && !have.contains(&latest)).then(|| files.iter().find(|x| x.file_id == latest).cloned()).flatten();
        let listed = files.iter().find(|x| x.file_id == f.file_id);
        let outdated = newer.is_none() && latest == f.file_id && listed.map(|x| matches!(x.category.as_deref(), Some("OLD_VERSION") | Some("ARCHIVED"))).unwrap_or(true);
        if newer.is_some() || outdated {
            out.push(Update { game: m.game.clone(), mod_id: m.mod_id, mod_name: m.name.clone(), file_id: f.file_id, newer, outdated });
        }
    }
    out
}

pub async fn check(nexus: &Nexus, mods: Vec<StoredMod>) -> Report {
    let checked = mods.len();
    let results: Vec<_> = stream::iter(mods)
        .map(|m| async move {
            let r = nexus.files_and_updates(&m.game, m.mod_id).await;
            (m, r)
        })
        .buffer_unordered(4)
        .collect()
        .await;
    let mut updates = vec![];
    let mut problems = vec![];
    for (m, r) in results {
        match r {
            Ok((files, links)) => updates.extend(compare(&m, &files, &links)),
            Err(e) => problems.push((m.name.clone(), e.message)),
        }
    }
    updates.sort_by(|a, b| a.mod_name.to_lowercase().cmp(&b.mod_name.to_lowercase()));
    Report { updates, checked, problems, at: crate::queue::now() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::StoredFile;

    fn file(id: u64, cat: &str) -> FileInfo {
        FileInfo { file_id: id, name: format!("File {id}"), file_name: format!("f{id}.7z"), version: None, category: Some(cat.into()), size: 1, uploaded: None, is_primary: false, description: None }
    }

    fn stored(ids: &[u64]) -> StoredMod {
        StoredMod {
            game: "g".into(),
            mod_id: 1,
            name: "Mod".into(),
            author: None,
            version: None,
            summary: None,
            picture_url: None,
            local: false,
            folder: String::new(),
            files: ids
                .iter()
                .map(|i| StoredFile { file_id: *i, name: String::new(), file_name: String::new(), version: None, category: None, size: 1, downloaded_at: 0, archive: None, unpacked: None, uploaded: None, verified: None })
                .collect(),
        }
    }

    #[test]
    fn follows_the_chain_to_the_newest() {
        assert_eq!(newest(1, &[(1, 2), (2, 5), (7, 8)]), 5);
        assert_eq!(newest(9, &[(1, 2)]), 9);
        assert_eq!(newest(1, &[(1, 3), (1, 4)]), 4);
        assert_eq!(newest(1, &[(1, 2), (2, 1)]), 1, "a loop ends");
    }

    #[test]
    fn reports_newer_and_old_files() {
        let files = vec![file(1, "OLD_VERSION"), file(2, "MAIN"), file(3, "OPTIONAL"), file(4, "OLD_VERSION")];
        let links = [(1, 2)];
        let u = compare(&stored(&[1, 3, 4]), &files, &links);
        assert_eq!(u.len(), 2);
        assert_eq!(u[0].newer.as_ref().unwrap().file_id, 2);
        assert!(u[1].outdated && u[1].file_id == 4);
        // Already have the newer one: nothing to say about the old one's update.
        assert!(compare(&stored(&[1, 2]), &files, &links).iter().all(|x| x.file_id != 1 || x.newer.is_none()));
    }
}
