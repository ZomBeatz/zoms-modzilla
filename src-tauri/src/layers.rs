//! A profile made from a collection keeps the collection apart from what you change on top.
//!
//! - The **base** is the collection's file list at one revision, saved when the profile is made
//!   (or linked later), so working out what's yours never needs Nexus Mods.
//! - **Add-ons** are named sets of changes on top: mods added, collection mods switched off,
//!   and other files picked for collection mods. Each can be switched on or off, and shared as
//!   an add-on pack (Nexus Mods numbers only, like every pack).
//! - **Your changes** are whatever else differs from the base, worked out each time.
//!
//! The profile's `mods` list stays what's put in the game, so the rest of the app needs no
//! change; these functions move it between states, keeping the order the person (or the
//! requirement sorting) gave it wherever they can.

use crate::deploy::ProfileMod;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FilePick {
    pub mod_id: u64,
    pub file_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AddedMod {
    pub mod_id: u64,
    pub file_id: u64,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// The mod it comes after in the list, so it lands in the same place again.
    #[serde(default)]
    pub after: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Changes {
    #[serde(default)]
    pub added: Vec<AddedMod>,
    /// Collection mods switched off (or taken out).
    #[serde(default)]
    pub off: Vec<u64>,
    /// Collection mods using another of their files.
    #[serde(default)]
    pub files: Vec<FilePick>,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.off.is_empty() && self.files.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Base {
    pub slug: String,
    pub revision: Option<u64>,
    pub name: String,
    /// Whether the collection's optional files were taken too.
    #[serde(default)]
    pub optional: bool,
    /// In the collection's order.
    pub files: Vec<FilePick>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Addon {
    pub id: String,
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(flatten)]
    pub changes: Changes,
}

/// How `mods` differs from the base.
pub fn diff(base: &Base, mods: &[ProfileMod]) -> Changes {
    let in_base: HashMap<u64, u64> = base.files.iter().map(|f| (f.mod_id, f.file_id)).collect();
    let mut c = Changes::default();
    let mut seen = HashSet::new();
    for (i, m) in mods.iter().enumerate() {
        seen.insert(m.mod_id);
        match in_base.get(&m.mod_id) {
            None => c.added.push(AddedMod { mod_id: m.mod_id, file_id: m.file_id, enabled: m.enabled, after: i.checked_sub(1).map(|j| mods[j].mod_id) }),
            Some(f) => {
                if *f != m.file_id {
                    c.files.push(FilePick { mod_id: m.mod_id, file_id: m.file_id });
                }
                if !m.enabled {
                    c.off.push(m.mod_id);
                }
            }
        }
    }
    c.off.extend(base.files.iter().filter(|f| !seen.contains(&f.mod_id)).map(|f| f.mod_id));
    c
}

/// Your changes: how `mods` differs from the base, less what the switched-on add-ons do.
pub fn yours(base: &Base, mods: &[ProfileMod], addons: &[Addon]) -> Changes {
    let mut c = diff(base, mods);
    for a in addons.iter().filter(|a| a.enabled) {
        c.added.retain(|m| !a.changes.added.iter().any(|x| x.mod_id == m.mod_id));
        c.off.retain(|id| !a.changes.off.contains(id));
        c.files.retain(|f| !a.changes.files.contains(f));
    }
    c
}

/// Put a set of changes onto `mods`. Added mods already there are left as they are.
pub fn apply(mods: &mut Vec<ProfileMod>, c: &Changes) {
    for f in &c.files {
        if let Some(m) = mods.iter_mut().find(|m| m.mod_id == f.mod_id) {
            m.file_id = f.file_id;
        }
    }
    for id in &c.off {
        if let Some(m) = mods.iter_mut().find(|m| m.mod_id == *id) {
            m.enabled = false;
        }
    }
    for a in &c.added {
        if mods.iter().any(|m| m.mod_id == a.mod_id) {
            continue;
        }
        let at = a.after.and_then(|after| mods.iter().position(|m| m.mod_id == after)).map(|i| i + 1).unwrap_or(mods.len());
        mods.insert(at, ProfileMod { mod_id: a.mod_id, file_id: a.file_id, enabled: a.enabled });
    }
}

/// Take an add-on's changes back off `mods`, leaving what the base or `others` (the other
/// switched-on add-ons) want.
pub fn revert(mods: &mut Vec<ProfileMod>, c: &Changes, base: &Base, others: &[&Changes]) {
    let base_file = |id: u64| base.files.iter().find(|f| f.mod_id == id).map(|f| f.file_id);
    mods.retain(|m| !c.added.iter().any(|a| a.mod_id == m.mod_id) || base_file(m.mod_id).is_some() || others.iter().any(|o| o.added.iter().any(|a| a.mod_id == m.mod_id)));
    for id in &c.off {
        if others.iter().any(|o| o.off.contains(id)) {
            continue;
        }
        match mods.iter_mut().find(|m| m.mod_id == *id) {
            Some(m) => m.enabled = true,
            None => {
                if let Some(file_id) = base_file(*id) {
                    // Taken out of the list: back where the collection has it.
                    let before = base.files.iter().take_while(|f| f.mod_id != *id).filter_map(|f| mods.iter().position(|m| m.mod_id == f.mod_id)).max();
                    mods.insert(before.map(|i| i + 1).unwrap_or(0), ProfileMod { mod_id: *id, file_id, enabled: true });
                }
            }
        }
    }
    for f in &c.files {
        if others.iter().any(|o| o.files.iter().any(|x| x.mod_id == f.mod_id)) {
            continue;
        }
        if let (Some(m), Some(file_id)) = (mods.iter_mut().find(|m| m.mod_id == f.mod_id && m.file_id == f.file_id), base_file(f.mod_id)) {
            m.file_id = file_id;
        }
    }
}

/// What moving to another revision does, in words the page shows.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rebased {
    /// Mods the new revision brings.
    pub added: Vec<u64>,
    /// Mods it no longer has (taken out of the profile).
    pub removed: Vec<u64>,
    /// Mods it moves to another file.
    pub updated: Vec<u64>,
    /// Your picks kept where the collection moved on: your file for a mod it updated.
    pub kept: Vec<u64>,
    /// Mods you'd added that are now part of the collection.
    pub joined: Vec<u64>,
}

/// Move `mods` from `old` to `new`, keeping your changes, add-ons and order. `addons` are the
/// switched-on ones (their switched-off mods stay off when a new file comes in).
pub fn rebase(mods: &mut Vec<ProfileMod>, old: &Base, new: &Base, addons: &[&Changes]) -> Rebased {
    let old_files: HashMap<u64, u64> = old.files.iter().map(|f| (f.mod_id, f.file_id)).collect();
    let new_files: HashMap<u64, u64> = new.files.iter().map(|f| (f.mod_id, f.file_id)).collect();
    let mut r = Rebased::default();

    // Gone from the collection: out of the profile.
    for f in &old.files {
        if !new_files.contains_key(&f.mod_id) {
            r.removed.push(f.mod_id);
        }
    }
    mods.retain(|m| !r.removed.contains(&m.mod_id));

    for (i, f) in new.files.iter().enumerate() {
        match old_files.get(&f.mod_id) {
            Some(old_file) if *old_file != f.file_id => {
                if let Some(m) = mods.iter_mut().find(|m| m.mod_id == f.mod_id) {
                    if m.file_id == *old_file {
                        m.file_id = f.file_id;
                        r.updated.push(f.mod_id);
                    } else if m.file_id != f.file_id {
                        r.kept.push(f.mod_id);
                    }
                }
            }
            Some(_) => {}
            None => {
                if mods.iter().any(|m| m.mod_id == f.mod_id) {
                    r.joined.push(f.mod_id);
                    continue;
                }
                let at = new.files[..i].iter().rev().find_map(|p| mods.iter().position(|m| m.mod_id == p.mod_id)).map(|i| i + 1).unwrap_or(0);
                let enabled = !addons.iter().any(|a| a.off.contains(&f.mod_id));
                mods.insert(at, ProfileMod { mod_id: f.mod_id, file_id: f.file_id, enabled });
                r.added.push(f.mod_id);
            }
        }
    }
    r
}

/// The revision in a profile's name, "… (revision 94)", for profiles made before bases were kept.
pub fn revision_in(name: &str) -> Option<u64> {
    let rest = name.rsplit_once("(revision ")?.1;
    rest.strip_suffix(')')?.trim().parse().ok()
}

/// The profile's name with its revision moved on, when it carries one.
pub fn renamed(name: &str, revision: u64) -> String {
    match name.rsplit_once("(revision ") {
        Some((head, rest)) if rest.ends_with(')') => format!("{head}(revision {revision})"),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(id: u64, file: u64, on: bool) -> ProfileMod {
        ProfileMod { mod_id: id, file_id: file, enabled: on }
    }

    fn base(files: &[(u64, u64)]) -> Base {
        Base { slug: "abc".into(), revision: Some(1), name: "C".into(), optional: false, files: files.iter().map(|(m, f)| FilePick { mod_id: *m, file_id: *f }).collect() }
    }

    #[test]
    fn finds_your_changes() {
        let b = base(&[(1, 10), (2, 20), (3, 30), (4, 40)]);
        // 2 off, 3 on another file, 4 taken out, 9 added after 1.
        let mods = vec![m(1, 10, true), m(9, 90, true), m(2, 20, false), m(3, 31, true)];
        let c = diff(&b, &mods);
        assert_eq!(c.added, vec![AddedMod { mod_id: 9, file_id: 90, enabled: true, after: Some(1) }]);
        assert_eq!(c.off, vec![2, 4]);
        assert_eq!(c.files, vec![FilePick { mod_id: 3, file_id: 31 }]);

        let addon = Addon { id: "a".into(), name: "Extras".into(), enabled: true, changes: Changes { added: c.added.clone(), off: vec![2], files: vec![] } };
        let y = yours(&b, &mods, std::slice::from_ref(&addon));
        assert!(y.added.is_empty());
        assert_eq!(y.off, vec![4]);
        let off = Addon { enabled: false, ..addon };
        assert_eq!(yours(&b, &mods, &[off]).added.len(), 1, "a switched-off add-on owns nothing");
    }

    #[test]
    fn add_ons_go_on_and_come_off() {
        let b = base(&[(1, 10), (2, 20), (3, 30)]);
        let start = vec![m(1, 10, true), m(2, 20, true), m(3, 30, true)];
        let addon = Changes { added: vec![AddedMod { mod_id: 9, file_id: 90, enabled: true, after: Some(1) }], off: vec![3], files: vec![FilePick { mod_id: 2, file_id: 21 }] };
        let mut mods = start.clone();
        apply(&mut mods, &addon);
        assert_eq!(mods, vec![m(1, 10, true), m(9, 90, true), m(2, 21, true), m(3, 30, false)]);
        revert(&mut mods, &addon, &b, &[]);
        assert_eq!(mods, start);

        // Another add-on that also switches 3 off keeps it off.
        let other = Changes { off: vec![3], ..Default::default() };
        apply(&mut mods, &addon);
        revert(&mut mods, &addon, &b, &[&other]);
        assert_eq!(mods[2], m(3, 30, false));

        // A collection mod taken out comes back in its place.
        let mut gone = vec![m(1, 10, true), m(3, 30, true)];
        revert(&mut gone, &Changes { off: vec![2], ..Default::default() }, &b, &[]);
        assert_eq!(gone, start);
    }

    #[test]
    fn moving_to_a_new_revision_keeps_your_changes() {
        let old = base(&[(1, 10), (2, 20), (3, 30), (4, 40)]);
        let new = base(&[(1, 11), (2, 21), (5, 50), (3, 30), (9, 90)]);
        // Yours: 2 on your own file, 3 off, 9 added at the end, and the list reordered (3 first).
        let mut mods = vec![m(3, 30, false), m(1, 10, true), m(2, 25, true), m(4, 40, true), m(9, 90, true)];
        let r = rebase(&mut mods, &old, &new, &[]);
        assert_eq!(r.removed, vec![4]);
        assert_eq!(r.updated, vec![1]);
        assert_eq!(r.kept, vec![2]);
        assert_eq!(r.added, vec![5]);
        assert_eq!(r.joined, vec![9]);
        assert_eq!(mods, vec![m(3, 30, false), m(1, 11, true), m(2, 25, true), m(5, 50, true), m(9, 90, true)]);
    }

    #[test]
    fn reads_revisions_in_names() {
        assert_eq!(revision_in("Fem V (revision 94)"), Some(94));
        assert_eq!(revision_in("Winter"), None);
        assert_eq!(renamed("Fem V (revision 94)", 95), "Fem V (revision 95)");
        assert_eq!(renamed("Winter", 3), "Winter");
    }
}
