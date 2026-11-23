//! Mod loaders, recognised by their files rather than by game: BepInEx, MelonLoader, UE4SS,
//! REFramework, an ASI loader, script extenders, RED4ext and friends. A game Zom's Nexus has
//! never seen still gets told "these mods need BepInEx, and it isn't there", and the loader's
//! log is read for errors that name the profile's mods.
//!
//! Only reads the game folder, never changes it.

use crate::gamelogs::Owners;
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// A loader, how to tell mods need it, and how to tell it's there.
struct Kind {
    name: &'static str,
    /// A mod file (lowercase, relative to the game folder) that only works with this loader.
    needs: fn(&str) -> bool,
    /// Any of these (lowercase) means it's there: a path from the game folder, or "*/name" for a
    /// file of that name anywhere up to four folders down.
    present: &'static [&'static str],
    /// Its log: a path from the game folder, or "*/name" as above.
    log: Option<&'static str>,
}

fn under(p: &str, dirs: &[&str]) -> bool {
    dirs.iter().any(|d| p.starts_with(d) || p.contains(&format!("/{d}")))
}

const KINDS: &[Kind] = &[
    Kind { name: "BepInEx", needs: |p| under(p, &["bepinex/plugins/", "bepinex/patchers/"]), present: &["bepinex/core"], log: Some("bepinex/logoutput.log") },
    Kind { name: "MelonLoader", needs: |p| p.starts_with("userlibs/"), present: &["melonloader"], log: Some("melonloader/latest.log") },
    Kind { name: "UE4SS", needs: |p| p.contains("/ue4ss/mods/") || (p.contains("/binaries/win64/mods/") && (p.ends_with(".lua") || p.ends_with("enabled.txt"))), present: &["*/ue4ss.dll"], log: Some("*/ue4ss.log") },
    Kind { name: "REFramework", needs: |p| p.starts_with("reframework/"), present: &["reframework/", "dinput8.dll"], log: Some("re2_framework_log.txt") },
    Kind { name: "an ASI loader", needs: |p| p.ends_with(".asi") && !p.contains("cyber_engine_tweaks"), present: &["*/dinput8.dll", "*/winmm.dll", "*/version.dll", "*/dsound.dll", "*/xinput1_3.dll", "*/vorbisfile.dll", "*/d3d9.dll", "*/d3d11.dll", "*/dxgi.dll", "*/scripthookv.dll"], log: None },
    Kind { name: "SKSE", needs: |p| p.starts_with("data/skse/plugins/") && p.ends_with(".dll"), present: &["skse64_loader.exe", "sksevr_loader.exe", "skse_loader.exe"], log: None },
    Kind { name: "F4SE", needs: |p| p.starts_with("data/f4se/plugins/") && p.ends_with(".dll"), present: &["f4se_loader.exe", "f4sevr_loader.exe"], log: None },
    Kind { name: "SFSE", needs: |p| p.starts_with("data/sfse/plugins/") && p.ends_with(".dll"), present: &["sfse_loader.exe"], log: None },
    Kind { name: "OBSE", needs: |p| (p.starts_with("data/obse/plugins/") || p.contains("/obse/plugins/")) && p.ends_with(".dll"), present: &["obse_loader.exe", "*/obse64_loader.exe"], log: None },
    Kind { name: "xNVSE", needs: |p| p.starts_with("data/nvse/plugins/") && p.ends_with(".dll"), present: &["nvse_loader.exe", "nvse_1_4.dll"], log: None },
    Kind { name: "RED4ext", needs: |p| p.starts_with("red4ext/plugins/"), present: &["red4ext/red4ext.dll"], log: None },
    Kind { name: "redscript", needs: |p| p.starts_with("r6/scripts/") && p.ends_with(".reds"), present: &["engine/tools/scc.exe"], log: None },
    Kind { name: "Cyber Engine Tweaks", needs: |p| p.starts_with("bin/x64/plugins/cyber_engine_tweaks/mods/"), present: &["bin/x64/plugins/cyber_engine_tweaks.asi"], log: Some("bin/x64/plugins/cyber_engine_tweaks/cyber_engine_tweaks.log") },
    Kind { name: "SMAPI", needs: |p| p.starts_with("mods/") && p.ends_with("/manifest.json"), present: &["stardewmoddingapi.exe", "stardewmoddingapi", "stardewmoddingapi.dll"], log: None },
];

/// A loader some of the profile's mods need that's neither in the game nor in the profile.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Missing {
    pub loader: String,
    /// The mods that need it (names), most files first.
    pub mods: Vec<String>,
}

/// Every file name up to four folders down: lowercase relative path, for "*/name" checks.
fn files(root: &Path) -> Vec<String> {
    let mut out = vec![];
    let mut stack = vec![(root.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.starts_with('.') {
                continue;
            }
            let p = e.path();
            let rel = p.strip_prefix(root).map(|r| r.to_string_lossy().to_lowercase()).unwrap_or_default();
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                out.push(format!("{rel}/"));
                if depth < 4 && out.len() < 20_000 {
                    stack.push((p, depth + 1));
                }
            } else {
                out.push(rel);
            }
        }
    }
    out
}

fn matches(have: &[String], want: &str) -> bool {
    match want.strip_prefix("*/") {
        Some(name) => have.iter().any(|h| h == name || h.ends_with(&format!("/{name}"))),
        None => have.iter().any(|h| h == want || h.trim_end_matches('/') == want.trim_end_matches('/') || h.starts_with(&format!("{}/", want.trim_end_matches('/')))),
    }
}

/// The loaders the profile's mods need and that aren't there. `owners` are the files the profile
/// puts in (lowercase, relative), so a loader that comes as one of its mods counts.
pub fn missing(game_root: &Path, owners: &Owners) -> Vec<Missing> {
    let mut game = files(game_root);
    // Files the game had before the profile went in count, and so do the profile's own.
    game.extend(owners.keys().cloned());
    game.extend(owners.keys().filter_map(|k| k.rsplit_once('/').map(|(d, _)| format!("{d}/"))));
    missing_in(&game, owners)
}

fn missing_in(have: &[String], owners: &Owners) -> Vec<Missing> {
    let mut out = vec![];
    for k in KINDS {
        let mut users: BTreeMap<String, usize> = BTreeMap::new();
        for (file, (_, name)) in owners {
            if (k.needs)(file) {
                *users.entry(name.clone()).or_default() += 1;
            }
        }
        if users.is_empty() || k.present.iter().any(|p| matches(have, p)) {
            continue;
        }
        // The loader itself can be one of the mods that "needs" it (its own plugins folder):
        // it's there if any of its marks is among the profile's files, checked above.
        let mut mods: Vec<(String, usize)> = users.into_iter().collect();
        mods.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        out.push(Missing { loader: k.name.to_string(), mods: mods.into_iter().map(|(n, _)| n).collect() });
    }
    out
}

/// The loaders in the game (or the profile), by name.
pub fn present(game_root: &Path, owners: &Owners) -> Vec<String> {
    let mut have = files(game_root);
    have.extend(owners.keys().cloned());
    KINDS.iter().filter(|k| k.present.iter().any(|p| matches(&have, p))).map(|k| k.name.to_string()).collect()
}

/// Newest log of each loader that's there: (loader, path).
pub fn logs(game_root: &Path) -> Vec<(&'static str, PathBuf)> {
    let have = files(game_root);
    let mut out = vec![];
    for k in KINDS {
        let Some(log) = k.log else { continue };
        if !k.present.iter().any(|p| matches(&have, p)) {
            continue;
        }
        let found = match log.strip_prefix("*/") {
            Some(name) => have.iter().find(|h| *h == name || h.ends_with(&format!("/{name}"))).cloned(),
            None => have.iter().find(|h| *h == log).cloned(),
        };
        if let Some(rel) = found {
            // Back to the real case on disk: walk the parts.
            if let Some(p) = real_path(game_root, &rel) {
                out.push((k.name, p));
            }
        }
    }
    out
}

fn real_path(root: &Path, rel_lower: &str) -> Option<PathBuf> {
    let mut p = root.to_path_buf();
    for part in rel_lower.split('/').filter(|s| !s.is_empty()) {
        let e = fs::read_dir(&p).ok()?.flatten().find(|e| e.file_name().to_string_lossy().to_lowercase() == part)?;
        p = e.path();
    }
    Some(p)
}

/// Words in an error line that point at a mod's file or name: "Foo.dll", "[Better Foo]",
/// "mods/BetterFoo/init.lua".
fn clues(line: &str) -> Vec<String> {
    let mut out = vec![];
    for raw in line.split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '(' | ')' | ',' | ';' | '`')) {
        let t = raw.trim_matches(|c: char| matches!(c, '[' | ']' | ':' | '.' | '<' | '>'));
        let low = t.to_lowercase();
        if [".dll", ".asi", ".lua", ".pak", ".reds", ".esp", ".esm", ".esl", ".js", ".cs"].iter().any(|e| low.ends_with(e)) {
            out.push(low.replace('\\', "/"));
        }
    }
    // Bracketed names: "[Error  :Better Foo] …", "[12:00:00.000] [Better Foo] …".
    let mut rest = line;
    while let Some(i) = rest.find('[') {
        let Some(j) = rest[i..].find(']') else { break };
        let inner = rest[i + 1..i + j].rsplit(':').next().unwrap_or("").trim();
        if inner.len() >= 3 && !inner.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ' ') {
            out.push(inner.to_lowercase());
        }
        rest = &rest[i + j + 1..];
    }
    out
}

const BAD: &[&str] = &["error", "exception", "failed", "fail to", "couldn't", "could not", "cannot load", "unable to"];
/// Lines that say "error" but aren't trouble.
const FINE: &[&str] = &["0 error", "no error", "errors: 0", "error handler", "error reporting"];

/// The mod a clue points at: a file it put in, a folder of its, or its name.
fn owner_of<'a>(owners: &'a Owners, clue: &str) -> Option<&'a (u64, String)> {
    let squash = |s: &str| s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
    if clue.contains('.') {
        let file = clue.rsplit('/').next().unwrap_or(clue);
        if let Some(v) = owners.iter().find(|(k, _)| k.ends_with(clue) || k.rsplit('/').next() == Some(file)).map(|(_, v)| v) {
            return Some(v);
        }
    }
    let c = squash(clue);
    if c.len() < 4 {
        return None;
    }
    owners.values().find(|(_, n)| squash(n) == c).or_else(|| owners.iter().find(|(k, _)| k.split('/').any(|part| squash(part) == c)).map(|(_, v)| v))
}

/// Errors in a loader's log, counted by the mod they name: (mod number, name, count, sample).
pub fn errors_in(text: &str, owners: &Owners) -> Vec<(u64, String, usize, String)> {
    let mut by: BTreeMap<u64, (String, usize, String)> = BTreeMap::new();
    for line in text.lines() {
        let low = line.to_lowercase();
        if !BAD.iter().any(|b| low.contains(b)) || FINE.iter().any(|f| low.contains(f)) {
            continue;
        }
        if let Some((id, name)) = clues(line).iter().find_map(|c| owner_of(owners, c)) {
            let e = by.entry(*id).or_insert_with(|| (name.clone(), 0, short(line)));
            e.1 += 1;
        }
    }
    let mut out: Vec<_> = by.into_iter().map(|(id, (n, c, s))| (id, n, c, s)).collect();
    out.sort_by(|a, b| b.2.cmp(&a.2));
    out
}

fn short(line: &str) -> String {
    let t = line.trim();
    // Drop a leading "[time] [level]" part.
    let t = t.trim_start_matches(|c: char| c == '[' || c.is_ascii_digit() || matches!(c, ':' | '.' | ' ' | '-' | ']'));
    let mut s: String = t.chars().take(160).collect();
    if t.chars().count() > 160 {
        s.push('…');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn owners(list: &[(&str, u64, &str)]) -> Owners {
        list.iter().map(|(f, id, n)| (f.to_string(), (*id, n.to_string()))).collect::<HashMap<_, _>>()
    }

    #[test]
    fn tells_which_loader_is_missing() {
        let o = owners(&[("bepinex/plugins/coolmod/coolmod.dll", 1, "Cool Mod"), ("data/skse/plugins/thing.dll", 2, "Thing"), ("textures/a.dds", 3, "Pretty")]);
        let m = missing_in(&["game.exe".into()], &o);
        assert_eq!(m.iter().map(|x| x.loader.as_str()).collect::<Vec<_>>(), vec!["BepInEx", "SKSE"]);
        assert_eq!(m[0].mods, vec!["Cool Mod"]);
        // There already (in the game, or as one of the profile's mods): nothing to say.
        let m = missing_in(&["bepinex/".into(), "bepinex/core/".into(), "bepinex/core/bepinex.dll".into(), "skse64_loader.exe".into()], &o);
        assert!(m.is_empty(), "{m:?}");
        let o = owners(&[("pak/binaries/win64/ue4ss/mods/x/scripts/main.lua", 4, "X"), ("pak/binaries/win64/ue4ss.dll", 5, "UE4SS")]);
        let mut have: Vec<String> = vec![];
        have.extend(o.keys().cloned());
        assert!(missing_in(&have, &o).is_empty());
    }

    #[test]
    fn finds_errors_naming_mods() {
        let o = owners(&[("bepinex/plugins/coolmod/coolmod.dll", 1, "Cool Mod"), ("ue4ss/mods/betterhud/scripts/main.lua", 2, "Better HUD")]);
        let log = "[Info   :   BepInEx] Loading [Cool Mod 1.0]\n[Error  :Cool Mod] NullReferenceException: Object reference not set\n[Error  :Cool Mod] Failed again\n[2024] [Lua] [BetterHUD] error: attempt to index nil in mods/BetterHUD/Scripts/main.lua:12\nNo errors in Other\n[Error] Something unrelated failed";
        let e = errors_in(log, &o);
        assert_eq!(e.iter().map(|x| (x.0, x.2)).collect::<Vec<_>>(), vec![(1, 2), (2, 1)]);
    }
}
