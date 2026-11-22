//! What each mod needs (other mods on Nexus Mods, downloads from elsewhere, DLC), when it was
//! last updated, and whether it's still published, from the v2 GraphQL API.
//!
//! Asked for many mods at once (up to `BATCH` a request) and kept in
//! `<data>/requirements.json` for a day, so the conflict check, which runs often, only asks
//! about mods it hasn't seen lately.

use crate::nexus::{ApiError, Nexus};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, fs, path::Path};

/// Mods per request; Nexus Mods caps how complex one query may be.
const BATCH: usize = 40;
/// How long an answer is kept, in seconds.
const KEEP: u64 = 24 * 3600;

const QUERY: &str = "query ZomsNexusRequirements($ids: [CompositeDomainWithIdInput!]!, $count: Int) {
  legacyModsByDomain(ids: $ids, count: $count) {
    nodes {
      modId name status updatedAt
      game { id domainName }
      modRequirements {
        nexusRequirements { nodes { modId modName url externalRequirement gameId notes } }
        dlcRequirements { gameExpansion { name } notes }
      }
    }
  }
}";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    /// A mod on Nexus Mods (0 when it's a download from somewhere else).
    pub mod_id: u64,
    pub name: String,
    /// The address, for downloads from outside Nexus Mods.
    pub url: Option<String>,
    /// Hosted for another game on Nexus Mods (a general tool, say).
    pub other_game: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Facts {
    pub mod_id: u64,
    pub game: String,
    pub name: String,
    /// "published", or another word when it's hidden or removed.
    pub status: String,
    /// When the mod (any of its files) was last updated, as a Unix time.
    pub updated_at: Option<u64>,
    pub requires: Vec<Requirement>,
    /// Expansions it needs ("Phantom Liberty").
    pub dlc: Vec<String>,
    pub fetched: u64,
}

fn key(game: &str, mod_id: u64) -> String {
    format!("{game}:{mod_id}")
}

fn load(data: &Path) -> HashMap<String, Facts> {
    fs::read_to_string(data.join("requirements.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save(data: &Path, all: &HashMap<String, Facts>) {
    if let Ok(b) = serde_json::to_vec(all) {
        let _ = crate::settings::write_atomic(&data.join("requirements.json"), &b);
    }
}

/// "2026-11-13T09:01:37Z" as a Unix time (UTC; fractions and offsets other than Z are ignored).
pub fn unix_time(s: &str) -> Option<u64> {
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.trim_end_matches('Z').split(':').map(|p| p.split('.').next().and_then(|x| x.parse::<i64>().ok()));
    let (hh, mm, ss) = (t.next()??, t.next().flatten().unwrap_or(0), t.next().flatten().unwrap_or(0));
    // Days from the civil date (Howard Hinnant's algorithm).
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    u64::try_from(days * 86400 + hh * 3600 + mm * 60 + ss).ok()
}

fn num(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(str::trim).filter(|s| !s.is_empty()).map(String::from)
}

/// Read one answer. Tolerant: anything missing is left out, never fatal.
pub fn read(data: &Value, now: u64) -> Vec<Facts> {
    let mut out = vec![];
    for n in data["legacyModsByDomain"]["nodes"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let (Some(mod_id), Some(game)) = (num(&n["modId"]), text(&n["game"]["domainName"])) else { continue };
        let game_id = num(&n["game"]["id"]);
        let requires = n["modRequirements"]["nexusRequirements"]["nodes"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .filter_map(|r| {
                let external = r["externalRequirement"].as_bool().unwrap_or(false);
                let name = text(&r["modName"])?;
                Some(Requirement {
                    mod_id: if external { 0 } else { num(&r["modId"]).unwrap_or(0) },
                    url: text(&r["url"]),
                    other_game: !external && game_id.is_some() && num(&r["gameId"]) != game_id,
                    notes: text(&r["notes"]),
                    name,
                })
            })
            .collect();
        let dlc = n["modRequirements"]["dlcRequirements"].as_array().map(Vec::as_slice).unwrap_or(&[]).iter().filter_map(|d| text(&d["gameExpansion"]["name"])).collect();
        out.push(Facts {
            mod_id,
            game: game.to_ascii_lowercase(),
            name: text(&n["name"]).unwrap_or_else(|| format!("Mod {mod_id}")),
            status: text(&n["status"]).unwrap_or_else(|| "published".into()),
            updated_at: n["updatedAt"].as_str().and_then(unix_time),
            requires,
            dlc,
            fetched: now,
        });
    }
    out
}

/// Facts for each of `wanted` (game, mod number): from the day's cache, the rest asked for in
/// as few requests as possible. A mod Nexus Mods doesn't answer for is simply left out.
pub async fn facts(nexus: &Nexus, data: &Path, wanted: &[(String, u64)]) -> Result<HashMap<String, Facts>, ApiError> {
    let now = crate::queue::now();
    let mut all = load(data);
    let mut ask: Vec<&(String, u64)> = wanted.iter().filter(|(g, m)| all.get(&key(g, *m)).map(|f| now.saturating_sub(f.fetched) > KEEP).unwrap_or(true)).collect();
    ask.sort();
    ask.dedup();
    let mut error = None;
    for chunk in ask.chunks(BATCH) {
        let ids: Vec<Value> = chunk.iter().map(|(g, m)| json!({ "gameDomain": g, "modId": m })).collect();
        match nexus.graphql(QUERY, json!({ "ids": ids, "count": chunk.len() })).await {
            Ok(d) => {
                for f in read(&d, now) {
                    all.insert(key(&f.game, f.mod_id), f);
                }
            }
            Err(e) => {
                error = Some(e);
                break;
            }
        }
    }
    if !ask.is_empty() {
        save(data, &all);
    }
    let found: HashMap<String, Facts> = wanted.iter().filter_map(|(g, m)| all.get(&key(g, *m)).map(|f| (key(g, *m), f.clone()))).collect();
    // With nothing to show at all, say why; otherwise use what there is.
    match error {
        Some(e) if found.is_empty() => Err(e),
        _ => Ok(found),
    }
}

/// What `resolve` changed in a profile, and what it couldn't.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Resolved {
    /// Requirements put in from the library.
    pub added: Vec<String>,
    /// Requirements that were in the profile but switched off.
    pub switched_on: Vec<String>,
    /// How many mods moved up to load before the mods needing them.
    pub moved: usize,
    /// Requirements on Nexus Mods (for this game) that aren't downloaded: (mod number, name).
    pub missing: Vec<(u64, String)>,
}

/// Make a profile hold what its mods need: put in requirements that are in the library (their
/// newest file), switch on ones that are off, and move each requirement above the first mod
/// needing it. Nothing else changes order. Runs on what `facts` knows; the caller asks Nexus
/// Mods about mods it adds and runs it again, so requirements of requirements come in too.
pub fn resolve(p: &mut crate::deploy::Profile, library: &[crate::library::StoredMod], facts: &HashMap<String, Facts>) -> Resolved {
    use crate::deploy::ProfileMod;
    let mut r = Resolved::default();
    let name_of = |id: u64, fallback: &str| library.iter().find(|m| m.game == p.game && m.mod_id == id).map(|m| m.name.clone()).unwrap_or_else(|| fallback.to_string());
    // 1. Put in and switch on what's needed.
    let mut i = 0;
    while i < p.mods.len() {
        let (needer, on) = (p.mods[i].mod_id, p.mods[i].enabled);
        i += 1;
        if !on {
            continue;
        }
        let Some(f) = lookup(facts, &p.game, needer) else { continue };
        for q in f.requires.iter().filter(|q| q.mod_id != 0 && !q.other_game) {
            if let Some(m) = p.mods.iter_mut().find(|m| m.mod_id == q.mod_id) {
                if !m.enabled {
                    m.enabled = true;
                    r.switched_on.push(name_of(q.mod_id, &q.name));
                }
                continue;
            }
            match library.iter().find(|m| m.game == p.game && m.mod_id == q.mod_id) {
                Some(m) => {
                    let newest = m.files.iter().max_by_key(|f| f.downloaded_at).map(|f| f.file_id);
                    if let Some(file_id) = newest {
                        p.mods.push(ProfileMod { mod_id: q.mod_id, file_id, enabled: true });
                        r.added.push(m.name.clone());
                    }
                }
                None => {
                    if !r.missing.iter().any(|(id, _)| *id == q.mod_id) {
                        r.missing.push((q.mod_id, q.name.clone()));
                    }
                }
            }
        }
    }
    // 2. Requirements before the mods needing them, keeping the order otherwise.
    let before: Vec<u64> = p.mods.iter().map(|m| m.mod_id).collect();
    let mut placed: Vec<ProfileMod> = Vec::with_capacity(p.mods.len());
    let mut visiting = std::collections::HashSet::new();
    fn place(id: u64, all: &[ProfileMod], game: &str, facts: &HashMap<String, Facts>, placed: &mut Vec<ProfileMod>, visiting: &mut std::collections::HashSet<u64>) {
        if placed.iter().any(|m| m.mod_id == id) || !visiting.insert(id) {
            return; // Already placed, or a loop (two mods needing each other): leave it be.
        }
        if let Some(f) = lookup(facts, game, id) {
            for q in &f.requires {
                if q.mod_id != 0 && all.iter().any(|m| m.mod_id == q.mod_id) {
                    place(q.mod_id, all, game, facts, placed, visiting);
                }
            }
        }
        if let Some(m) = all.iter().find(|m| m.mod_id == id) {
            if !placed.iter().any(|x| x.mod_id == id) {
                placed.push(m.clone());
            }
        }
    }
    let all = p.mods.clone();
    for m in &all {
        place(m.mod_id, &all, &p.game, facts, &mut placed, &mut visiting);
    }
    // Two entries for one mod (different files) are kept, in their place.
    for m in &all {
        if !placed.iter().any(|x| x.mod_id == m.mod_id && x.file_id == m.file_id) {
            let at = placed.iter().rposition(|x| x.mod_id == m.mod_id).map(|i| i + 1).unwrap_or(placed.len());
            placed.insert(at, m.clone());
        }
    }
    // Mods that moved up (not every position that shifted because of them).
    r.moved = placed.iter().enumerate().filter(|(i, m)| before.iter().position(|b| *b == m.mod_id).map(|o| o > *i).unwrap_or(false)).count();
    p.mods = placed;
    r
}

pub fn lookup<'a>(all: &'a HashMap<String, Facts>, game: &str, mod_id: u64) -> Option<&'a Facts> {
    all.get(&key(game, mod_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact(id: u64, requires: &[u64]) -> Facts {
        Facts {
            mod_id: id,
            game: "g".into(),
            name: format!("M{id}"),
            status: "published".into(),
            updated_at: None,
            requires: requires.iter().map(|r| Requirement { mod_id: *r, name: format!("M{r}"), url: None, other_game: false, notes: None }).collect(),
            dlc: vec![],
            fetched: 0,
        }
    }

    #[test]
    fn puts_in_and_sorts_requirements() {
        use crate::deploy::{Profile, ProfileMod};
        use crate::library::{StoredFile, StoredMod};
        let lib = |id: u64| StoredMod { game: "g".into(), mod_id: id, name: format!("M{id}"), author: None, version: None, summary: None, picture_url: None, local: false, folder: String::new(), files: vec![StoredFile { file_id: id * 10, name: "f".into(), file_name: "f.zip".into(), version: None, category: None, size: 1, downloaded_at: 1, archive: None, unpacked: None, uploaded: None, verified: None }] };
        // 5 (a texture pack) first, then 1, which needs 2 (in the library) and 9 (not downloaded);
        // 2 needs 3, which is in the profile but switched off and below everything.
        let library = vec![lib(1), lib(2), lib(3), lib(5)];
        let mut facts = HashMap::new();
        for f in [fact(1, &[2, 9]), fact(2, &[3]), fact(3, &[]), fact(5, &[])] {
            facts.insert(key("g", f.mod_id), f);
        }
        let on = |id: u64, e: bool| ProfileMod { mod_id: id, file_id: id * 10, enabled: e };
        let mut p = Profile { id: "p".into(), name: "P".into(), game: "g".into(), mods: vec![on(5, true), on(1, true), on(3, false)], collection: None, manual_requirements: false, new_downloads: false };
        let r = resolve(&mut p, &library, &facts);
        assert_eq!(r.added, vec!["M2".to_string()]);
        assert_eq!(r.switched_on, vec!["M3".to_string()]);
        assert_eq!(r.missing, vec![(9, "M9".to_string())]);
        assert_eq!(p.mods.iter().map(|m| m.mod_id).collect::<Vec<_>>(), vec![5, 3, 2, 1], "requirements just above what needs them; 5 stays first");
        assert!(p.mods.iter().all(|m| m.enabled));
        // Running it again changes nothing.
        let again = resolve(&mut p, &library, &facts);
        assert_eq!((again.added.len(), again.switched_on.len(), again.moved), (0, 0, 0));
    }

    #[test]
    fn reads_dates() {
        assert_eq!(unix_time("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_time("2026-11-13T09:01:37Z"), Some(1_794_560_497));
        assert_eq!(unix_time("2024-02-29T12:00:00.123Z"), Some(1_709_208_000));
        assert_eq!(unix_time("nonsense"), None);
    }

    #[test]
    fn reads_requirements_tolerantly() {
        let d = json!({ "legacyModsByDomain": { "nodes": [
            { "modId": 4198, "name": "ArchiveXL", "status": "published", "updatedAt": "2026-11-13T09:01:37Z",
              "game": { "id": "3333", "domainName": "cyberpunk2077" },
              "modRequirements": {
                "nexusRequirements": { "nodes": [
                    { "modId": "2380", "modName": "RED4ext", "url": "", "externalRequirement": false, "gameId": "3333", "notes": "" },
                    { "modId": "0", "modName": "Some tool", "url": "https://example.com/tool", "externalRequirement": true, "gameId": "3333", "notes": "Get 2.0" },
                    { "modId": "77", "modName": "General helper", "url": "", "externalRequirement": false, "gameId": "2295", "notes": null }
                ] },
                "dlcRequirements": [ { "gameExpansion": { "name": "Phantom Liberty" }, "notes": null } ]
              } },
            { "name": "No number, skipped" },
            { "modId": 7358, "game": { "domainName": "cyberpunk2077" } }
        ] } });
        let f = read(&d, 5);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].updated_at, Some(1_794_560_497));
        assert_eq!(f[0].requires[0], Requirement { mod_id: 2380, name: "RED4ext".into(), url: None, other_game: false, notes: None });
        assert_eq!(f[0].requires[1].mod_id, 0);
        assert_eq!(f[0].requires[1].url.as_deref(), Some("https://example.com/tool"));
        assert!(f[0].requires[2].other_game);
        assert_eq!(f[0].dlc, vec!["Phantom Liberty".to_string()]);
        assert_eq!(f[1].status, "published");
        assert!(f[1].requires.is_empty());
    }
}
