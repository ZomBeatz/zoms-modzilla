//! Checking a profile before the game starts, without touching the game folder:
//!
//! - **File conflicts:** files more than one mod has. The mod later in the profile wins.
//! - **Plugins (Bethesda games):** a plugin's header names the plugins it needs (its masters).
//!   One that isn't in the game or the profile crashes the game at start; one that loads after
//!   the plugin needing it is the wrong order.
//! - **Requirements** (`needs`): mods a mod lists on Nexus Mods as needed, that the profile
//!   doesn't have switched on. Filled in by `requirements` after the check.
//! - **Compatibility** (`compat`): mods that run code in the game (script extenders, plugins,
//!   scripts) whose file is older than the game's last update, and mods taken off Nexus Mods.

use crate::{
    deploy::{self, Profile},
    library::StoredMod,
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::Read,
    path::Path,
};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    /// The mod whose copies are used.
    pub winner: String,
    /// The mods whose copies of the same files are hidden.
    pub losers: Vec<String>,
    pub files: usize,
    /// A few of the files, to show.
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PluginIssue {
    pub plugin: String,
    pub mod_name: String,
    pub master: String,
    /// "missing": not in the game or the profile. "order": in the profile, but after this one.
    pub kind: String,
}

/// Something a mod needs that the profile doesn't have switched on.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Need {
    /// What's needed.
    pub name: String,
    /// Its number on Nexus Mods, for this game (0 for downloads from elsewhere and DLC).
    pub mod_id: u64,
    /// "notDownloaded", "notInProfile", "switchedOff", "external" (from another site),
    /// "otherGame" (a tool on Nexus Mods filed under another game) or "dlc".
    pub kind: String,
    pub url: Option<String>,
    pub notes: Option<String>,
    /// The mods that need it.
    pub needed_by: Vec<String>,
}

/// A mod that may not work with the game as it is now.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Compat {
    pub mod_id: u64,
    pub mod_name: String,
    /// "newerFile": runs code, its file is from before the game's last update, and the mod has
    /// been updated since. "notUpdated": runs code and hasn't been updated since the game was.
    /// "removed": no longer published on Nexus Mods.
    pub kind: String,
    pub file_date: Option<u64>,
    pub mod_updated: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Programs {
    pub mod_name: String,
    /// A few of them, to show.
    pub files: Vec<String>,
    pub count: usize,
}

/// A mod whose files sit inside an extra folder, so they'd land where the game won't look.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Misplaced {
    pub mod_id: u64,
    pub mod_name: String,
    /// The extra folder ("Cool Mod v2").
    pub extra: String,
    /// Where its files land now, and where the game's own folders are ("Data/Cool Mod v2/textures" vs "Data/textures").
    pub lands: String,
    pub should: String,
}

/// Files that come with a mod only to be read by people.
pub fn is_doc(name: &str) -> bool {
    let ext = name.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "txt" | "md" | "pdf" | "jpg" | "jpeg" | "png" | "gif" | "webp" | "url" | "html" | "htm" | "rtf" | "docx")
}

/// `rels` (where a mod's files land, relative to the game) all sit in one folder the game doesn't
/// have, right inside `target`, and that folder holds a folder the game does have there.
pub fn misplaced(rels: &[String], target: &str, game_root: &Path) -> Option<(String, String, String)> {
    let t = target.trim_matches('/').to_lowercase();
    let pre = if t.is_empty() { String::new() } else { format!("{t}/") };
    let mut tops: HashSet<String> = HashSet::new();
    let mut inner: HashSet<String> = HashSet::new();
    for r in rels {
        let low = r.to_lowercase();
        let rest = low.strip_prefix(&pre)?;
        match rest.split_once('/') {
            Some((top, more)) => {
                tops.insert(top.to_string());
                if let Some((sub, _)) = more.split_once('/') {
                    inner.insert(sub.to_string());
                }
            }
            None if is_doc(rest) => {}
            None => return None,
        }
    }
    if tops.len() != 1 {
        return None;
    }
    let top = tops.into_iter().next()?;
    let dir = game_root.join(target);
    let names: HashSet<(String, bool)> = fs::read_dir(&dir).ok()?.flatten().map(|e| (e.file_name().to_string_lossy().to_lowercase(), e.path().is_dir())).collect();
    if names.iter().any(|(n, _)| *n == top) {
        return None;
    }
    let hit = inner.iter().find(|i| names.contains(&((*i).clone(), true)))?;
    // As written, not lowercased: the first file under the extra folder's matching one.
    let depth = t.split('/').filter(|x| !x.is_empty()).count();
    let want = format!("{pre}{top}/{hit}/");
    let real = rels.iter().find(|r| r.to_lowercase().starts_with(&want)).map(|r| r.split('/').take(depth + 2).collect::<Vec<_>>())?;
    let extra = real[depth].to_string();
    let lands = real.join("/");
    let should = real[..depth].iter().chain(std::iter::once(&real[depth + 1])).copied().collect::<Vec<_>>().join("/");
    Some((extra, lands, should))
}

/// Programs and scripts a person (or the game) could run: worth knowing about in a mod.
pub fn is_program(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(ext.as_str(), "exe" | "msi" | "bat" | "cmd" | "com" | "scr" | "ps1" | "vbs" | "sh" | "jar" | "appimage")
}

#[derive(Debug, Clone, Default)]
pub struct CodeMod {
    pub mod_id: u64,
    pub name: String,
    pub uploaded: Option<u64>,
}

/// Files that run inside the game: script extender and game plugins, and scripts.
pub fn runs_code(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    let ext = p.rsplit('.').next().unwrap_or("");
    matches!(ext, "dll" | "asi" | "reds" | "lua" | "pex") || p.contains("red4ext/") || p.contains("/skse/") || p.contains("/f4se/") || p.contains("/sfse/") || p.starts_with("r6/scripts/")
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub conflicts: Vec<Conflict>,
    /// Mods needed and not switched on in the profile.
    pub needs: Vec<Need>,
    /// Mods that may not work with this version of the game.
    pub compat: Vec<Compat>,
    /// Whether Nexus Mods was asked about requirements (not without a key or a connection).
    pub requirements_checked: bool,
    /// When Steam last updated the game, when known.
    pub game_updated: Option<u64>,
    /// Mods with programs in them (.exe, scripts): rarely needed, and the riskiest thing to find.
    pub programs: Vec<Programs>,
    /// Mods that run code in the game, for the compatibility check.
    #[serde(skip)]
    pub code: Vec<CodeMod>,
    pub plugins: Vec<PluginIssue>,
    /// Plugins in load order (the profile's, after the game's own).
    pub load_order: Vec<String>,
    pub missing: Vec<String>,
    pub installers: Vec<String>,
    pub files: usize,
    pub at: u64,
    /// Mod loader DLLs the game's Steam launch options don't let run under Proton.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch_fix: Option<crate::launchopts::LaunchFix>,
    /// Mod loaders some mods need that aren't in the game or the profile.
    #[serde(default)]
    pub loaders_missing: Vec<crate::loaders::Missing>,
    /// Mod loaders in the game or the profile.
    #[serde(default)]
    pub loaders: Vec<String>,
    /// Mods whose files would land somewhere the game won't look (packed a folder too deep).
    #[serde(default)]
    pub misplaced: Vec<Misplaced>,
    /// What the game's own logs said the last time it started with this profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub game_log: Option<crate::gamelogs::GameLog>,
    /// This profile is in the game, but its files changed since (Play or Put mods in game
    /// brings the game up to date).
    pub out_of_date: bool,
    /// The file each game file should be (the winning mod's), for telling whether the game is
    /// up to date: lowercase relative path → (relative path as written, source file).
    #[serde(skip)]
    pub sources: HashMap<String, (String, std::path::PathBuf)>,
    /// Who puts each file in the game (for tracing the game's logs back to mods).
    #[serde(skip)]
    pub owners: crate::gamelogs::Owners,
}

/// The masters a plugin names in its header (TES4 record, MAST sub-records).
pub fn masters(path: &Path) -> Vec<String> {
    let mut head = vec![0u8; 64 * 1024];
    let n = fs::File::open(path).and_then(|mut f| f.read(&mut head)).unwrap_or(0);
    head.truncate(n);
    if head.len() < 24 || &head[0..4] != b"TES4" {
        return vec![];
    }
    let size = u32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
    // Oblivion's record header is 20 bytes; every later game's is 24.
    for header in [24usize, 20] {
        let end = (header + size).min(head.len());
        let mut at = header;
        let mut out = vec![];
        let mut sane = true;
        while at + 6 <= end {
            let kind = &head[at..at + 4];
            let len = u16::from_le_bytes([head[at + 4], head[at + 5]]) as usize;
            if !kind.iter().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == b'_') {
                sane = false;
                break;
            }
            let data = &head[(at + 6).min(end)..(at + 6 + len).min(end)];
            if kind == b"MAST" {
                let name = data.split(|b| *b == 0).next().unwrap_or(&[]);
                out.push(String::from_utf8_lossy(name).into_owned());
            }
            at += 6 + len;
        }
        if sane {
            return out;
        }
    }
    vec![]
}

fn is_plugin(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".esp") || l.ends_with(".esm") || l.ends_with(".esl")
}

pub fn check(profile: &Profile, mods: &[StoredMod], mod_folder: &str, game_root: Option<&Path>, ours_now: &HashSet<String>) -> Report {
    let mut r = Report { at: crate::queue::now(), ..Default::default() };
    // Who provides each file, in profile order.
    let mut by_file: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut plugins: Vec<(String, String, std::path::PathBuf)> = vec![]; // name, mod, source
    for pm in profile.mods.iter().filter(|m| m.enabled) {
        let Some(m) = mods.iter().find(|m| m.game == profile.game && m.mod_id == pm.mod_id) else {
            r.missing.push(format!("Mod {}", pm.mod_id));
            continue;
        };
        let Some(f) = m.files.iter().find(|f| f.file_id == pm.file_id) else {
            r.missing.push(m.name.clone());
            continue;
        };
        let Ok(dir) = deploy::unpacked_dir(m, f) else {
            r.missing.push(m.name.clone());
            continue;
        };
        if dir.join("fomod").is_dir() {
            r.installers.push(m.name.clone());
        }
        let mut code = false;
        let mut programs: Vec<String> = vec![];
        let entries = deploy::entries(&dir, mod_folder);
        if let Some(root) = game_root.filter(|_| !dir.join(crate::fomod::MARK).is_file()) {
            let rels: Vec<String> = entries.iter().map(|(r, _)| r.clone()).collect();
            if let Some((extra, lands, should)) = misplaced(&rels, mod_folder, root) {
                r.misplaced.push(Misplaced { mod_id: m.mod_id, mod_name: m.name.clone(), extra, lands, should });
            }
        }
        for (rel, from) in entries {
            code |= runs_code(&rel);
            if is_program(&rel) {
                programs.push(rel.clone());
            }
            let (folder, name) = rel.rsplit_once('/').unwrap_or(("", rel.as_str()));
            if is_plugin(name) && folder.eq_ignore_ascii_case(mod_folder) {
                plugins.retain(|(n, _, _)| !n.eq_ignore_ascii_case(name));
                plugins.push((name.to_string(), m.name.clone(), from.clone()));
            }
            r.owners.insert(rel.to_lowercase(), (m.mod_id, m.name.clone()));
            r.sources.insert(rel.to_lowercase(), (rel.clone(), from.clone()));
            let providers = by_file.entry(rel.to_lowercase()).or_default();
            providers.retain(|p| p != &m.name);
            providers.push(m.name.clone());
        }
        if code && !m.local {
            r.code.push(CodeMod { mod_id: m.mod_id, name: m.name.clone(), uploaded: f.uploaded });
        }
        if !programs.is_empty() {
            r.programs.push(Programs { mod_name: m.name.clone(), count: programs.len(), files: programs.into_iter().take(3).collect() });
        }
    }
    r.files = by_file.len();

    // Group conflicts by who wins over whom.
    let mut groups: HashMap<(String, Vec<String>), Vec<String>> = HashMap::new();
    for (file, providers) in &by_file {
        if providers.len() > 1 {
            let winner = providers.last().unwrap().clone();
            let losers = providers[..providers.len() - 1].to_vec();
            groups.entry((winner, losers)).or_default().push(file.clone());
        }
    }
    r.conflicts = groups
        .into_iter()
        .map(|((winner, losers), files)| Conflict { winner, losers, files: files.len(), examples: files.into_iter().take(5).collect() })
        .collect();
    r.conflicts.sort_by(|a, b| b.files.cmp(&a.files).then(a.winner.cmp(&b.winner)));

    // Masters: the game's own plugins (not ones Zom's Nexus put there) count as present.
    let in_game = |name: &str| {
        game_root
            .map(|root| {
                let rel = format!("{}/{}", mod_folder, name).trim_start_matches('/').to_string();
                !ours_now.contains(&rel.to_lowercase()) && fs::read_dir(root.join(mod_folder)).map(|rd| rd.flatten().any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))).unwrap_or(false)
            })
            .unwrap_or(false)
    };
    let order: Vec<String> = plugins.iter().map(|(n, _, _)| n.to_lowercase()).collect();
    for (i, (name, mod_name, src)) in plugins.iter().enumerate() {
        for master in masters(src) {
            let ml = master.to_lowercase();
            let kind = match order.iter().position(|p| *p == ml) {
                Some(j) if j < i => continue,
                Some(_) => "order",
                None if in_game(&master) => continue,
                None => "missing",
            };
            r.plugins.push(PluginIssue { plugin: name.clone(), mod_name: mod_name.clone(), master, kind: kind.into() });
        }
    }
    r.load_order = plugins.into_iter().map(|(n, _, _)| n).collect();
    r
}

/// Fill in `needs` and `compat` from what Nexus Mods says about the profile's mods.
/// The author's note makes it a suggestion, not a must: "optional", "not required",
/// "Recommended but not required" (but not "Required to claim … Recommended to use …").
pub fn soft(notes: Option<&str>) -> bool {
    let n = notes.unwrap_or("").to_lowercase();
    // "MANDATORY" on its own is a must; "Optional - MANDATORY for LEO addon" isn't.
    if n.contains("mandatory") && !n.contains("optional") {
        return false;
    }
    let n = n.replace("not required", "optional").replace("not needed", "optional").replace("isn't required", "optional").replace("is not required", "optional");
    n.contains("optional")
        || n.contains("addon")
        || n.contains("add-on")
        || n.contains("for devs")
        || n.contains("for developers")
        || (n.contains("recommend") && !n.contains("requir") && !n.contains("need"))
}

pub fn add_requirements(r: &mut Report, profile: &Profile, library: &[StoredMod], facts: &HashMap<String, crate::requirements::Facts>, game_updated: Option<u64>) {
    use crate::requirements::lookup;
    r.requirements_checked = true;
    r.game_updated = game_updated;
    let on: HashSet<u64> = profile.mods.iter().filter(|m| m.enabled).map(|m| m.mod_id).collect();
    let off: HashSet<u64> = profile.mods.iter().filter(|m| !m.enabled).map(|m| m.mod_id).collect();
    let have: HashSet<u64> = library.iter().filter(|m| m.game == profile.game).map(|m| m.mod_id).collect();
    let name_of = |id: u64| library.iter().find(|m| m.game == profile.game && m.mod_id == id).map(|m| m.name.clone());
    let mut needs: Vec<Need> = vec![];
    let mut add = |n: Need| match needs.iter_mut().find(|x| x.kind == n.kind && x.mod_id == n.mod_id && x.name == n.name) {
        Some(x) => {
            for b in n.needed_by {
                if !x.needed_by.contains(&b) {
                    x.needed_by.push(b);
                }
            }
        }
        None => needs.push(n),
    };
    for pm in profile.mods.iter().filter(|m| m.enabled) {
        let Some(f) = lookup(facts, &profile.game, pm.mod_id) else { continue };
        let owner = name_of(pm.mod_id).unwrap_or_else(|| f.name.clone());
        for q in &f.requires {
            let kind = if q.mod_id == 0 {
                "external"
            } else if q.other_game {
                "otherGame"
            } else if on.contains(&q.mod_id) {
                continue;
            } else if off.contains(&q.mod_id) {
                "switchedOff"
            } else if have.contains(&q.mod_id) {
                "notInProfile"
            } else if lookup(facts, &profile.game, q.mod_id).map(|x| x.status != "published") == Some(true) {
                // Hidden or removed on Nexus Mods: it can't be downloaded any more.
                "unavailable"
            } else {
                "notDownloaded"
            };
            let kind = if matches!(kind, "notDownloaded" | "notInProfile" | "switchedOff") && soft(q.notes.as_deref()) { "optional" } else { kind };
            add(Need { name: name_of(q.mod_id).filter(|_| q.mod_id != 0).unwrap_or_else(|| q.name.clone()), mod_id: q.mod_id, kind: kind.into(), url: q.url.clone(), notes: q.notes.clone(), needed_by: vec![owner.clone()] });
        }
        for d in &f.dlc {
            add(Need { name: d.clone(), mod_id: 0, kind: "dlc".into(), url: None, notes: None, needed_by: vec![owner.clone()] });
        }
        if f.status != "published" {
            r.compat.push(Compat { mod_id: pm.mod_id, mod_name: owner.clone(), kind: "removed".into(), file_date: None, mod_updated: f.updated_at });
        }
    }
    // Problems first: mods missing from the profile, then things to check by hand.
    let rank = |k: &str| match k {
        "notDownloaded" => 0,
        "notInProfile" => 1,
        "switchedOff" => 2,
        "dlc" => 3,
        "unavailable" => 4,
        "external" => 5,
        "optional" => 7,
        _ => 6,
    };
    needs.sort_by_key(|n| rank(&n.kind));
    r.needs = needs;

    if let Some(g) = game_updated {
        for c in &r.code {
            let updated = lookup(facts, &profile.game, c.mod_id).and_then(|f| f.updated_at);
            let kind = match (c.uploaded, updated) {
                (Some(file), Some(m)) if file < g && m > g => "newerFile",
                (Some(file), _) if file < g => "notUpdated",
                (None, Some(m)) if m < g => "notUpdated",
                _ => continue,
            };
            r.compat.push(Compat { mod_id: c.mod_id, mod_name: c.name.clone(), kind: kind.into(), file_date: c.uploaded, mod_updated: updated });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deploy::ProfileMod;
    use crate::library::StoredFile;

    /// A tiny Skyrim SE style plugin naming `masters`.
    fn plugin(masters: &[&str]) -> Vec<u8> {
        let mut sub = vec![];
        sub.extend(b"HEDR");
        sub.extend(12u16.to_le_bytes());
        sub.extend([0u8; 12]);
        for m in masters {
            let mut d = m.as_bytes().to_vec();
            d.push(0);
            sub.extend(b"MAST");
            sub.extend((d.len() as u16).to_le_bytes());
            sub.extend(&d);
            sub.extend(b"DATA");
            sub.extend(8u16.to_le_bytes());
            sub.extend([0u8; 8]);
        }
        let mut rec = b"TES4".to_vec();
        rec.extend((sub.len() as u32).to_le_bytes());
        rec.extend([0u8; 16]);
        rec.extend(sub);
        rec
    }

    fn lib_mod(root: &Path, id: u64, files: &[(&str, Vec<u8>)]) -> StoredMod {
        let folder = root.join(format!("lib/{id}"));
        for (rel, body) in files {
            let p = folder.join("u").join(rel);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, body).unwrap();
        }
        StoredMod {
            game: "skyrimspecialedition".into(),
            mod_id: id,
            name: format!("Mod {id}"),
            author: None,
            version: None,
            summary: None,
            picture_url: None,
            local: false,
            adult: false,
            source: None,
            folder: folder.to_string_lossy().into_owned(),
            files: vec![StoredFile { file_id: id, name: "Main".into(), file_name: "m.7z".into(), version: None, category: None, size: 1, downloaded_at: 0, archive: None, unpacked: Some("u".into()), uploaded: None, verified: None, installed: None, has_installer: false }],
        }
    }

    #[test]
    fn judges_requirements_and_game_updates() {
        use crate::requirements::{Facts, Requirement};
        let dir = tempfile::tempdir().unwrap();
        let mut code = lib_mod(dir.path(), 1, &[("SKSE/Plugins/helper.dll", b"x".to_vec())]);
        code.files[0].uploaded = Some(100);
        let mods = vec![code, lib_mod(dir.path(), 2, &[("textures/a.dds", b"1".to_vec())]), lib_mod(dir.path(), 3, &[("textures/b.dds", b"1".to_vec())])];
        let p = Profile {
            id: "p".into(),
            name: "P".into(),
            game: "skyrimspecialedition".into(),
            mods: vec![ProfileMod { mod_id: 1, file_id: 1, enabled: true }, ProfileMod { mod_id: 2, file_id: 2, enabled: true }],
            collection: None,
            manual_requirements: false,
            new_downloads: false,
            own_saves: false,
            base: None,
            addons: vec![],
            rules: vec![],
            note: None,
            ts_pack: None,
        };
        let mut r = check(&p, &mods, "Data", None, &HashSet::new());
        assert_eq!(r.code.len(), 1, "only the mod with a plugin runs code");
        let req = |id: u64, name: &str| Requirement { mod_id: id, name: name.into(), url: None, other_game: false, notes: None };
        let fact = |id: u64, requires: Vec<Requirement>, updated: u64| Facts { mod_id: id, game: "skyrimspecialedition".into(), name: format!("Mod {id}"), status: "published".into(), updated_at: Some(updated), requires, dlc: vec![], adult: false, category: None, fetched: 0, v: 0 };
        let mut facts = HashMap::new();
        facts.insert("skyrimspecialedition:1".to_string(), fact(1, vec![req(3, "Mod 3"), req(4, "Address Library"), Requirement { url: Some("https://example.com".into()), ..req(0, "A tool") }], 300));
        facts.insert("skyrimspecialedition:2".to_string(), fact(2, vec![req(1, "Mod 1"), req(4, "Address Library")], 50));
        add_requirements(&mut r, &p, &mods, &facts, Some(200));
        let kinds: Vec<(&str, &str)> = r.needs.iter().map(|n| (n.name.as_str(), n.kind.as_str())).collect();
        assert_eq!(kinds, vec![("Address Library", "notDownloaded"), ("Mod 3", "notInProfile"), ("A tool", "external")]);
        assert_eq!(r.needs[0].needed_by, vec!["Mod 1".to_string(), "Mod 2".to_string()], "one line for a mod two others need");
        // Mod 1 runs code, its file is from before the game update, and the mod has a newer one.
        assert_eq!(r.compat.len(), 1);
        assert_eq!((r.compat[0].mod_id, r.compat[0].kind.as_str()), (1, "newerFile"));

        // A requirement taken down on Nexus Mods can't be got; one the author calls optional isn't a must.
        let mut gone = fact(5, vec![], 1);
        gone.status = "hidden".into();
        facts.insert("skyrimspecialedition:5".to_string(), gone);
        facts.insert("skyrimspecialedition:2".to_string(), fact(2, vec![req(5, "Hidden Outfit"), Requirement { notes: Some("Recommended but not required.".into()), ..req(6, "Nice Extra") }], 50));
        let mut r2 = check(&p, &mods, "Data", None, &HashSet::new());
        add_requirements(&mut r2, &p, &mods, &facts, None);
        let kinds: Vec<(&str, &str)> = r2.needs.iter().filter(|n| n.mod_id >= 5).map(|n| (n.name.as_str(), n.kind.as_str())).collect();
        assert_eq!(kinds, vec![("Hidden Outfit", "unavailable"), ("Nice Extra", "optional")]);
        assert!(soft(Some("optional")) && soft(Some("Recommended")) && !soft(Some("Required to claim crowd vehicles. Recommended to use the CET version.")) && !soft(Some("needs it")) && !soft(None));
        assert!(soft(Some("ADDON: modded vehicle list")) && soft(Some("Optional - MANDATORY for LEO addon")) && soft(Some("FOR DEVS: (to view logs)")));
        assert!(!soft(Some("MANDATORY")) && !soft(Some("Required for v3.xx")));
        assert!(runs_code("r6/scripts/thing.reds") && runs_code("bin/x64/plugins/x.asi") && !runs_code("archive/pc/mod/tex.archive"));
        assert!(is_program("tools/Setup.EXE") && is_program("install.sh") && !is_program("Data/a.esp"));
    }

    #[test]
    fn reads_masters() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.esp");
        fs::write(&p, plugin(&["Skyrim.esm", "Lanterns.esm"])).unwrap();
        assert_eq!(masters(&p), vec!["Skyrim.esm", "Lanterns.esm"]);
        fs::write(&p, b"not a plugin at all, just text").unwrap();
        assert!(masters(&p).is_empty());
    }

    #[test]
    fn finds_conflicts_and_plugin_problems() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("game");
        fs::create_dir_all(game.join("Data")).unwrap();
        fs::write(game.join("Data/Skyrim.esm"), plugin(&[])).unwrap();
        let mods = vec![
            lib_mod(dir.path(), 1, &[("textures/sky.dds", b"1".to_vec()), ("textures/sun.dds", b"1".to_vec()), ("Patch.esp", plugin(&["Skyrim.esm", "Base.esm"]))]),
            lib_mod(dir.path(), 2, &[("textures/sky.dds", b"2".to_vec()), ("textures/sun.dds", b"2".to_vec()), ("Base.esm", plugin(&["Skyrim.esm"]))]),
            lib_mod(dir.path(), 3, &[("Other.esp", plugin(&["Missing.esm"]))]),
        ];
        let profile = Profile { id: "p".into(), name: "P".into(), game: "skyrimspecialedition".into(), mods: [1, 2, 3, 4].iter().map(|i| ProfileMod { mod_id: *i, file_id: *i, enabled: true }).collect(), collection: None, manual_requirements: false, new_downloads: false, own_saves: false, base: None, addons: vec![], rules: vec![], note: None, ts_pack: None };
        let r = check(&profile, &mods, "Data", Some(&game), &HashSet::new());
        assert_eq!(r.conflicts, vec![Conflict { winner: "Mod 2".into(), losers: vec!["Mod 1".into()], files: 2, examples: vec!["data/textures/sky.dds".into(), "data/textures/sun.dds".into()] }]);
        assert_eq!(r.load_order, vec!["Patch.esp", "Base.esm", "Other.esp"]);
        assert_eq!(r.plugins.len(), 2);
        assert!(r.plugins.contains(&PluginIssue { plugin: "Patch.esp".into(), mod_name: "Mod 1".into(), master: "Base.esm".into(), kind: "order".into() }));
        assert!(r.plugins.contains(&PluginIssue { plugin: "Other.esp".into(), mod_name: "Mod 3".into(), master: "Missing.esm".into(), kind: "missing".into() }));
        assert_eq!(r.missing, vec!["Mod 4"]);
        assert_eq!(r.plugins.len() + r.missing.len(), 3);
    }
}

#[cfg(test)]
mod placed_tests {
    use super::*;

    #[test]
    fn notices_a_mod_packed_a_folder_too_deep() {
        let game = tempfile::tempdir().unwrap();
        fs::create_dir_all(game.path().join("Data/textures")).unwrap();
        let r = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let m = misplaced(&r(&["Data/Cool Mod v2/textures/a.dds", "Data/readme.txt"]), "Data", game.path()).unwrap();
        assert_eq!(m, ("Cool Mod v2".into(), "Data/Cool Mod v2/textures".into(), "Data/textures".into()));
        assert!(misplaced(&r(&["Data/textures/a.dds"]), "Data", game.path()).is_none(), "right where it belongs");
        assert!(misplaced(&r(&["Data/NewThing/b.cfg"]), "Data", game.path()).is_none(), "a new folder of its own");
    }
}
