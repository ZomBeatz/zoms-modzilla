//! Plugin load order for Bethesda games from LOOT's community masterlists (github.com/loot):
//! which plugins load after which, their groups, and LOOT's notes (warnings, incompatibilities,
//! plugins that need cleaning). The sort keeps every hard rule (masters first, each plugin after
//! its own masters and after what the masterlist says), puts groups in their order, and keeps
//! your order for everything else. The order is kept per profile, apart from the mod order
//! (which decides who wins file clashes).

use serde::Serialize;
use serde_yaml::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    time::{Duration, SystemTime},
};

/// LOOT's masterlist repository for a game.
pub fn repo(game: &str) -> Option<&'static str> {
    Some(match game {
        "skyrimspecialedition" => "skyrimse",
        "skyrim" => "skyrim",
        "fallout4" => "fallout4",
        "newvegas" => "falloutnv",
        "fallout3" => "fallout3",
        "oblivion" => "oblivion",
        "starfield" => "starfield",
        "morrowind" => "morrowind",
        _ => return None,
    })
}

/// The masterlist, fetched at most once a day (an older copy is used when GitHub can't be reached).
pub async fn masterlist(http: &reqwest::Client, cache: &Path, game: &str) -> Result<Value, String> {
    let repo = repo(game).ok_or("LOOT doesn't have a masterlist for this game.")?;
    let file = cache.join("loot").join(format!("{repo}.yaml"));
    let fresh = fs::metadata(&file).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok()).is_some_and(|a| a < Duration::from_secs(86_400));
    if !fresh {
        let got = async {
            let r = http.get(format!("https://raw.githubusercontent.com/loot/{repo}/HEAD/masterlist.yaml")).send().await.map_err(|e| format!("Couldn't reach LOOT's masterlist: {e}"))?;
            if !r.status().is_success() {
                return Err(format!("LOOT's masterlist answered {}.", r.status()));
            }
            let b = r.bytes().await.map_err(|e| e.to_string())?;
            if b.len() > 32 * 1024 * 1024 {
                return Err("LOOT's masterlist is far bigger than expected.".into());
            }
            Ok(b)
        }
        .await;
        match got {
            Ok(b) => {
                let _ = fs::create_dir_all(file.parent().unwrap());
                let _ = fs::write(&file, &b);
            }
            Err(e) if !file.is_file() => return Err(e),
            Err(_) => {}
        }
    }
    let text = fs::read_to_string(&file).map_err(|e| e.to_string())?;
    parse(&text)
}

pub fn parse(text: &str) -> Result<Value, String> {
    let mut v: Value = serde_yaml::from_str(text).map_err(|e| format!("LOOT's masterlist didn't read: {e}"))?;
    v.apply_merge().map_err(|e| e.to_string())?;
    Ok(v)
}

/// A LOOT name: a plain file name, or (with `:\*?|`) a regular expression. The usual ones are
/// turned into a simple pattern; ones that aren't simple never match.
fn matches(name: &str, plugin: &str) -> bool {
    if !name.contains([':', '\\', '*', '?', '|']) {
        return name.eq_ignore_ascii_case(plugin);
    }
    if name.contains(['(', '[', '|', '{', '+']) {
        return false;
    }
    // '\.' a dot, '.*' anything, '.' one character.
    let mut pat: Vec<Option<char>> = vec![]; // None = anything
    let mut star: Vec<bool> = vec![];
    let cs: Vec<char> = name.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        match (cs[i], cs.get(i + 1)) {
            ('\\', Some(&c)) => {
                pat.push(Some(c));
                star.push(false);
                i += 2;
            }
            ('.', Some('*')) => {
                pat.push(None);
                star.push(true);
                i += 2;
            }
            ('.', _) => {
                pat.push(None);
                star.push(false);
                i += 1;
            }
            (c, _) => {
                pat.push(Some(c));
                star.push(false);
                i += 1;
            }
        }
    }
    fn go(p: &[Option<char>], s: &[bool], t: &[char]) -> bool {
        match p.first() {
            None => t.is_empty(),
            Some(_) if s[0] => (0..=t.len()).any(|k| go(&p[1..], &s[1..], &t[k..])),
            Some(None) => !t.is_empty() && go(&p[1..], &s[1..], &t[1..]),
            Some(Some(c)) => t.first().is_some_and(|x| x.eq_ignore_ascii_case(c)) && go(&p[1..], &s[1..], &t[1..]),
        }
    }
    let t: Vec<char> = plugin.chars().collect();
    go(&pat, &star, &t)
}

/// The masterlist's entries that apply to a plugin.
fn entries_for<'a>(ml: &'a Value, plugin: &str) -> Vec<&'a Value> {
    ml["plugins"].as_sequence().into_iter().flatten().filter(|e| e["name"].as_str().is_some_and(|n| matches(n, plugin))).collect()
}

/// A name in a list: a plain string, or a map with `name`.
fn names(list: &Value) -> Vec<(String, Option<String>, Option<String>)> {
    list.as_sequence()
        .into_iter()
        .flatten()
        .filter_map(|x| match x {
            Value::String(s) => Some((s.clone(), None, None)),
            Value::Mapping(_) => Some((x["name"].as_str()?.to_string(), x["display"].as_str().map(String::from), x["condition"].as_str().map(String::from))),
            _ => None,
        })
        .collect()
}

/// Only simple conditions are checked: `file("x")`, `active("x")`, `not …`, joined by `and`.
/// Anything else (checksums, versions, `or`) isn't, and the note is left out rather than guessed.
fn condition(c: &str, have: &dyn Fn(&str) -> bool, active: &dyn Fn(&str) -> bool) -> Option<bool> {
    let mut all = true;
    for part in c.split(" and ") {
        let mut p = part.trim();
        let mut neg = false;
        if let Some(rest) = p.strip_prefix("not ") {
            neg = true;
            p = rest.trim();
        }
        let arg = |f: &str| p.strip_prefix(f).and_then(|r| r.strip_suffix(")")).map(|a| a.trim_matches('"').to_string());
        let v = if let Some(a) = arg("file(") {
            if a.contains(['*', '\\', '|']) {
                return None;
            }
            have(&a)
        } else {
            let a = arg("active(")?;
            if a.contains(['*', '\\', '|']) {
                return None;
            }
            active(&a)
        };
        all &= v != neg;
    }
    Some(all)
}

fn text_of(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Sequence(list) => list.iter().find(|x| x["lang"].as_str() == Some("en")).or(list.first()).and_then(|x| x["text"].as_str()).map(String::from),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub plugin: String,
    /// "say", "warn" or "error".
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Sorted {
    pub order: Vec<String>,
    pub changed: bool,
    pub notes: Vec<Note>,
    /// Plugins that LOOT knows aren't in its masterlist (unknown to it).
    pub unknown: usize,
}

/// A plugin to sort: its file name, whether it's a master, its masters, and its CRC (for
/// LOOT's dirty-plugin notes).
pub struct Plugin {
    pub name: String,
    pub master: bool,
    pub masters: Vec<String>,
    pub crc: Option<u32>,
}

/// Sort `plugins` (in the order they load now). `have` says whether a file is in the game's data.
pub fn sort(ml: &Value, plugins: &[Plugin], have: &dyn Fn(&str) -> bool) -> Sorted {
    let n = plugins.len();
    let idx: HashMap<String, usize> = plugins.iter().enumerate().map(|(i, p)| (p.name.to_lowercase(), i)).collect();
    let active = |name: &str| idx.contains_key(&name.to_lowercase());
    let mut before: Vec<HashSet<usize>> = vec![HashSet::new(); n]; // before[b] = plugins that must load before b
    let mut notes: Vec<Note> = vec![];
    let mut unknown = 0;

    // Groups: each group's place in the chain ("Main Plugins" first, "Late Loaders" last).
    let mut group_after: HashMap<String, Vec<String>> = HashMap::new();
    for g in ml["groups"].as_sequence().into_iter().flatten() {
        if let Some(name) = g["name"].as_str() {
            group_after.insert(name.to_string(), names(&g["after"]).into_iter().map(|x| x.0).collect());
        }
    }
    fn depth(g: &str, after: &HashMap<String, Vec<String>>, seen: &mut HashSet<String>) -> usize {
        if !seen.insert(g.to_string()) {
            return 0;
        }
        after.get(g).map(|a| a.iter().map(|x| 1 + depth(x, after, seen)).max().unwrap_or(0)).unwrap_or(0)
    }
    let default_group = "default";
    let mut rank: Vec<usize> = vec![0; n];

    for (i, p) in plugins.iter().enumerate() {
        // Masters load before every non-master.
        if !p.master {
            for (j, q) in plugins.iter().enumerate() {
                if q.master {
                    before[i].insert(j);
                }
            }
        }
        for m in &p.masters {
            if let Some(&j) = idx.get(&m.to_lowercase()) {
                if j != i {
                    before[i].insert(j);
                }
            }
        }
        let es = entries_for(ml, &p.name);
        if es.is_empty() {
            unknown += 1;
        }
        let mut group = default_group.to_string();
        for e in &es {
            if let Some(g) = e["group"].as_str() {
                group = g.to_string();
            }
            for (a, _, _) in names(&e["after"]) {
                for (j, q) in plugins.iter().enumerate() {
                    if j != i && matches(&a, &q.name) {
                        before[i].insert(j);
                    }
                }
            }
            for m in e["msg"].as_sequence().into_iter().flatten() {
                let ok = m["condition"].as_str().map(|c| condition(c, have, &active)).unwrap_or(Some(true));
                if ok != Some(true) {
                    continue;
                }
                let Some(mut text) = text_of(&m["content"]) else { continue };
                for (k, s) in m["subs"].as_sequence().into_iter().flatten().enumerate() {
                    if let Some(s) = s.as_str() {
                        text = text.replace(&format!("{{{k}}}"), s);
                    }
                }
                notes.push(Note { plugin: p.name.clone(), kind: m["type"].as_str().unwrap_or("say").to_string(), text });
            }
            for (other, display, cond) in names(&e["inc"]) {
                if cond.is_none() && plugins.iter().any(|q| q.name != p.name && matches(&other, &q.name)) {
                    notes.push(Note { plugin: p.name.clone(), kind: "error".into(), text: format!("It doesn't work together with {}.", display.unwrap_or(other)) });
                }
            }
            for (req, display, cond) in names(&e["req"]) {
                let is_plugin = req.to_lowercase().ends_with(".esp") || req.to_lowercase().ends_with(".esm") || req.to_lowercase().ends_with(".esl");
                if cond.is_none() && is_plugin && !req.contains(['*', '\\', '|']) && !active(&req) && !have(&req) {
                    notes.push(Note { plugin: p.name.clone(), kind: "error".into(), text: format!("It needs {}, which isn't there.", display.unwrap_or(req)) });
                }
            }
            if let Some(crc) = p.crc {
                for d in e["dirty"].as_sequence().into_iter().flatten() {
                    if d["crc"].as_u64() == Some(crc as u64) {
                        let count = |k: &str| d[k].as_u64().unwrap_or(0);
                        let (itm, udr, nav) = (count("itm"), count("udr"), count("nav"));
                        let util = d["util"].as_str().unwrap_or("xEdit");
                        let mut what = vec![];
                        if itm > 0 {
                            what.push(format!("{itm} identical-to-master records"));
                        }
                        if udr > 0 {
                            what.push(format!("{udr} deleted references"));
                        }
                        if nav > 0 {
                            what.push(format!("{nav} deleted navmeshes (can't be cleaned automatically)"));
                        }
                        notes.push(Note { plugin: p.name.clone(), kind: "warn".into(), text: format!("Needs cleaning: {}. Clean it with {util} (Quick Auto Clean).", if what.is_empty() { "it has dirty edits".into() } else { what.join(", ") }) });
                    }
                }
            }
        }
        rank[i] = depth(&group, &group_after, &mut HashSet::new());
    }

    // Kahn's sort: of the plugins whose "befores" are placed, the one with the earliest group,
    // then the earliest now, goes next. Cycles (contradicting rules) keep the current order.
    let mut placed = vec![false; n];
    let mut order = vec![];
    for _ in 0..n {
        let next = (0..n).filter(|&i| !placed[i] && before[i].iter().all(|&j| placed[j])).min_by_key(|&i| (rank[i], i)).or_else(|| (0..n).find(|&i| !placed[i]));
        let Some(i) = next else { break };
        placed[i] = true;
        order.push(plugins[i].name.clone());
    }
    let changed = order.iter().zip(plugins).any(|(a, p)| *a != p.name);
    notes.sort_by(|a, b| (b.kind == "error").cmp(&(a.kind == "error")).then((b.kind == "warn").cmp(&(a.kind == "warn"))));
    notes.dedup();
    Sorted { order, changed, notes, unknown }
}

/// The plugin order kept for a profile (empty: the mod order).
pub fn saved(data: &Path, profile: &str) -> Vec<String> {
    fs::read(data.join("plugin-order").join(format!("{}.json", crate::library::safe_name(profile)))).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save(data: &Path, profile: &str, order: Option<&[String]>) -> Result<(), String> {
    let f = data.join("plugin-order").join(format!("{}.json", crate::library::safe_name(profile)));
    match order {
        None => {
            let _ = fs::remove_file(&f);
            Ok(())
        }
        Some(o) => {
            fs::create_dir_all(f.parent().unwrap()).map_err(|e| e.to_string())?;
            crate::settings::write_atomic(&f, &serde_json::to_vec_pretty(o).map_err(|e| e.to_string())?)
        }
    }
}

/// `plugins` (in mod order) in the kept order: the ones it lists first, in its order, then any
/// newer ones in mod order.
pub fn apply_order(plugins: &mut Vec<String>, kept: &[String]) {
    if kept.is_empty() {
        return;
    }
    let pos: HashMap<String, usize> = kept.iter().enumerate().map(|(i, p)| (p.to_lowercase(), i)).collect();
    let mut known: Vec<String> = plugins.iter().filter(|p| pos.contains_key(&p.to_lowercase())).cloned().collect();
    known.sort_by_key(|p| pos[&p.to_lowercase()]);
    let new: Vec<String> = plugins.iter().filter(|p| !pos.contains_key(&p.to_lowercase())).cloned().collect();
    *plugins = known.into_iter().chain(new).collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    const ML: &str = r#"
prelude:
  common:
    - &dirtyNote
      util: 'SSEEdit'
groups:
  - name: &main Main Plugins
  - name: &early Early Loaders
    after: [ *main ]
  - name: default
    after: [ *early ]
  - name: &late Late Loaders
    after: [ default ]
plugins:
  - name: 'Unofficial Skyrim Special Edition Patch.esp'
    group: *early
    dirty:
      - <<: *dirtyNote
        crc: 0x1234
        itm: 3
  - name: 'Late.esp'
    group: *late
  - name: 'B.esp'
    after: [ 'C.esp' ]
    msg:
      - type: warn
        content: 'Use {0} instead.'
        subs: [ 'Better B' ]
      - type: say
        content: 'Only with checksums.'
        condition: 'checksum("x.esp", 1234)'
    inc: [ 'Bad.esp' ]
    req: [ 'Needed.esm' ]
  - name: 'Alternate Start.*\.esp'
    group: *late
"#;

    fn p(name: &str, master: bool, masters: &[&str]) -> Plugin {
        Plugin { name: name.into(), master, masters: masters.iter().map(|s| s.to_string()).collect(), crc: None }
    }

    #[test]
    fn sorts_like_loot_and_keeps_the_rest() {
        let ml = parse(ML).unwrap();
        let mut ussep = p("Unofficial Skyrim Special Edition Patch.esp", false, &[]);
        ussep.crc = Some(0x1234);
        let plugins = vec![p("Late.esp", false, &[]), p("B.esp", false, &[]), p("C.esp", false, &[]), ussep, p("Alternate Start - Live Another Life.esp", false, &[]), p("Patch.esp", false, &["My.esm"]), p("My.esm", true, &[]), p("Bad.esp", false, &[])];
        let s = sort(&ml, &plugins, &|_| false);
        assert_eq!(s.order, vec!["My.esm", "Unofficial Skyrim Special Edition Patch.esp", "C.esp", "B.esp", "Patch.esp", "Bad.esp", "Late.esp", "Alternate Start - Live Another Life.esp"]);
        assert!(s.changed);
        let texts: Vec<&str> = s.notes.iter().map(|n| n.text.as_str()).collect();
        assert!(texts.iter().any(|t| t.contains("Use Better B instead.")));
        assert!(texts.iter().any(|t| t.contains("doesn't work together with Bad.esp")));
        assert!(texts.iter().any(|t| t.contains("needs Needed.esm")));
        assert!(texts.iter().any(|t| t.contains("3 identical-to-master records") && t.contains("SSEEdit")));
        assert!(!texts.iter().any(|t| t.contains("checksums")), "conditions it can't check are left out");
        // Already in order: nothing changes.
        let again: Vec<Plugin> = s.order.iter().map(|n| p(n, n.ends_with(".esm"), &[])).collect();
        assert!(!sort(&ml, &again, &|_| true).changed);
    }

    #[test]
    fn simple_patterns_and_kept_orders() {
        assert!(matches("Alternate Start.*\\.esp", "alternate start - lal.esp"));
        assert!(!matches("Alternate Start.*\\.esp", "Alternate Start.esm"));
        assert!(!matches("(a|b)\\.esp", "a.esp"), "not simple: never matches");
        let mut ps = vec!["A.esp".to_string(), "B.esp".into(), "New.esp".into(), "C.esp".into()];
        apply_order(&mut ps, &["C.esp".into(), "a.esp".into(), "B.esp".into(), "Gone.esp".into()]);
        assert_eq!(ps, vec!["C.esp", "A.esp", "B.esp", "New.esp"]);
        let d = tempfile::tempdir().unwrap();
        save(d.path(), "p1", Some(&["X.esp".into()])).unwrap();
        assert_eq!(saved(d.path(), "p1"), vec!["X.esp"]);
        save(d.path(), "p1", None).unwrap();
        assert!(saved(d.path(), "p1").is_empty());
    }

    /// `LOOT_ML=path cargo test real_masterlist -- --ignored`: reads a real masterlist.
    #[test]
    #[ignore]
    fn real_masterlist() {
        let ml = parse(&fs::read_to_string(std::env::var("LOOT_ML").unwrap()).unwrap()).unwrap();
        let plugins = vec![p("SkyUI_SE.esp", false, &[]), p("Unofficial Skyrim Special Edition Patch.esp", false, &[]), p("Alternate Start - Live Another Life.esp", false, &[])];
        let s = sort(&ml, &plugins, &|_| true);
        println!("{:?}\n{} notes, {} unknown", s.order, s.notes.len(), s.unknown);
    }
}
