//! What the game said the last time it started, for the conflict check: Cyberpunk 2077's
//! RED4ext log (plugins refused for being made for another game patch) and its script compiler's
//! log (mods whose scripts didn't compile, and the mod they were looking for). Each problem is
//! traced back to the profile's mod that put the file there.
//!
//! Only read, never changed. Only logs written after the profile went into the game count, so
//! an old run never blames today's mods.

use serde::Serialize;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogIssue {
    pub mod_id: u64,
    pub mod_name: String,
    /// "oldPlugin": made for another game patch. "needsNewer": its scripts look for something a
    /// mod in the profile doesn't have (usually that mod is too old). "needsMod": they look for a
    /// mod that isn't in the profile. "scripts": its scripts didn't compile, another reason.
    /// "loaderRefused": a script extender, BepInEx or SMAPI wouldn't load it.
    pub kind: String,
    /// The other mod, for needsNewer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub other_name: Option<String>,
    /// What the log said, short.
    pub detail: String,
}

/// A mod whose files ArchiveXL or TweakXL complained about (broken or missing outfits, meshes,
/// translations, tweaks). These rarely stop the game, but right before a crash they're the
/// first suspects.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AssetIssue {
    pub mod_id: u64,
    pub mod_name: String,
    pub count: usize,
    /// One of the messages, short.
    pub sample: String,
    /// Logged during play (not while the game loaded) in the five minutes before the crash.
    pub near_crash: bool,
}

/// The game crashed during its last run.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Crash {
    pub at: u64,
    /// Minutes into the run.
    pub minutes: u64,
    /// Where it happened, when a crash reporter said ("Cyberpunk2077.exe+0x500AA5").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// The crash happened in the game's own code (no mod's program on the trace): usually
    /// something a mod loaded (an outfit, a mesh) rather than a mod's code.
    pub in_game_code: bool,
    pub out_of_memory: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameLog {
    /// When the game wrote these logs.
    pub at: u64,
    /// Written before the mods last went in: what it says may be fixed already.
    pub before_change: bool,
    #[serde(default)]
    pub assets: Vec<AssetIssue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crash: Option<Crash>,
    pub issues: Vec<LogIssue>,
    /// What SMAPI's log says, worked out (Stardew Valley).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smapi: Option<Smapi>,
}

/// A mod SMAPI wouldn't load for a reason of its own (too old, missing something), and the
/// skipped mods that needed it, directly or through each other.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Broken {
    pub name: String,
    pub version: String,
    /// The mod in the profile it came from, when known.
    pub mod_id: Option<u64>,
    /// "old" (made for an older game or SMAPI), "missing" (needs mods that aren't there), or "other".
    pub kind: String,
    pub why: String,
    /// For "missing": what it needs.
    pub needs: Vec<String>,
    pub dependents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SmapiUpdate {
    pub name: String,
    pub have: String,
    pub version: String,
    /// Its Nexus Mods number, when the update is there.
    pub nexus_id: Option<u64>,
}

/// SMAPI's last run, worked out: what broke and why, what has updates, and what kept erroring.
#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Smapi {
    pub at: u64,
    pub smapi_version: Option<String>,
    pub game_version: Option<String>,
    /// Root causes, most dependents first.
    pub broken: Vec<Broken>,
    /// How many mods were skipped in all.
    pub skipped: usize,
    pub updates: Vec<SmapiUpdate>,
    /// Who logged errors while the game ran, and how many (most first, at most 6).
    pub errors: Vec<(String, usize)>,
    /// Mods named in the details of errors (the code that failed), most often first.
    pub culprits: Vec<Culprit>,
    /// The game stopped with an error (not closed by you).
    pub crashed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Culprit {
    pub name: String,
    pub mod_id: Option<u64>,
    pub count: usize,
    /// The error, short ("ArgumentNullException: Value cannot be null").
    pub what: String,
    /// It was failing in the last seconds before the game stopped.
    pub at_the_end: bool,
}

/// Code namespaces that are the game's, SMAPI's or the frameworks', never a mod's.
const NOT_MODS: &[&str] = &["Microsoft", "System", "StardewValley", "StardewModdingAPI", "xTile", "HarmonyLib", "MonoMod", "Netcode", "MonoGame", "Galaxy", "Steamworks", "SMAPI", "Mono", "Internal", "Pathoschild.Stardew.Common"];

/// The mod in an error's details: the first frame from code that isn't the game's or SMAPI's.
fn culprit_in(frames: &[&str]) -> Option<String> {
    for f in frames {
        let Some(at) = f.trim().strip_prefix("at ") else { continue };
        let ns = at.split(['(', '<']).next().unwrap_or("");
        let first = ns.split('.').next().unwrap_or("");
        if first.is_empty() || NOT_MODS.iter().any(|n| ns.starts_with(n)) {
            continue;
        }
        // "Pathoschild.Stardew.Automate.X" → "Automate"; "DynamicReflections.Framework.X" → "DynamicReflections".
        let parts: Vec<&str> = ns.split('.').collect();
        let name = if parts.len() > 2 && (parts[1] == "Stardew" || parts[1] == "Stardew Valley") { parts[2] } else { first };
        return Some(name.to_string());
    }
    None
}

/// "DynamicReflections" and "Dynamic Reflections" are the same mod.
fn same_name(a: &str, b: &str) -> bool {
    let k = |s: &str| s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
    !k(a).is_empty() && k(a) == k(b)
}

/// Who put each file in the game: lowercase path relative to the game folder → (mod number, name).
pub type Owners = HashMap<String, (u64, String)>;

fn modified(p: &Path) -> Option<u64> {
    fs::metadata(p).ok()?.modified().ok()?.duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs())
}

/// "S:\steamapps\common\Cyberpunk 2077\r6\scripts\X\a.reds:6:1:" → "r6/scripts/x/a.reds".
fn rel_from(win: &str, game_dir_name: &str) -> Option<String> {
    let p = win.replace('\\', "/").to_lowercase();
    let key = format!("/{}/", game_dir_name.to_lowercase());
    let i = p.find(&key)?;
    let rest = &p[i + key.len()..];
    // Drop ":line:col:" after the file name.
    let end = rest.find(".reds").map(|e| e + 5).or_else(|| rest.find(".dll").map(|e| e + 4)).unwrap_or(rest.len());
    Some(rest[..end].trim_end_matches(['\'', ':']).to_string())
}

/// The mod that put anything in `folder` (lowercase, relative), if one did.
fn owner_of_folder<'a>(owners: &'a Owners, folder: &str) -> Option<&'a (u64, String)> {
    let prefix = format!("{}/", folder.trim_end_matches('/'));
    owners.iter().find(|(k, _)| k.starts_with(&prefix)).map(|(_, v)| v)
}

/// Read the last run's logs. Ones written before `since` (when the mods last went in) are still
/// read, and marked, since a problem the person hasn't changed anything about is still there.
pub fn read(game_root: &Path, owners: &Owners, sources: &HashMap<String, (String, PathBuf)>, prefix: Option<&Path>, since: u64) -> Option<GameLog> {
    let since_change = since;
    let since = 0;
    let name = game_root.file_name()?.to_string_lossy().into_owned();
    let mut issues: Vec<LogIssue> = vec![];
    let mut at = 0;
    fn push(issues: &mut Vec<LogIssue>, i: LogIssue) {
        if !issues.iter().any(|x| x.mod_id == i.mod_id && x.kind == i.kind && x.other_id == i.other_id) {
            issues.push(i);
        }
    }

    // RED4ext: plugins refused for being made for another game patch.
    let logs = game_root.join("red4ext/logs");
    let newest = fs::read_dir(&logs).into_iter().flatten().flatten().filter(|e| e.file_name().to_string_lossy().to_lowercase().starts_with("red4ext")).max_by_key(|e| modified(&e.path()).unwrap_or(0));
    if let Some(e) = newest {
        let t = modified(&e.path()).unwrap_or(0);
        if t >= since {
            at = at.max(t);
            let text = fs::read_to_string(e.path()).unwrap_or_default();
            let mut last_plugin: Option<String> = None;
            for line in text.lines() {
                if let Some(i) = line.find("Loading plugin from '") {
                    last_plugin = line[i + 21..].split('\'').next().and_then(|p| rel_from(p, &name));
                } else if line.contains("is incompatible with the current patch") {
                    if let Some((id, n)) = last_plugin.as_ref().and_then(|p| owners.get(p)) {
                        let plugin = line.split("[RED4ext] ").nth(1).and_then(|r| r.split(" (").next()).unwrap_or("its plugin");
                        push(&mut issues, LogIssue { mod_id: *id, mod_name: n.clone(), kind: "oldPlugin".into(), other_id: None, other_name: None, detail: format!("RED4ext didn't load {plugin}: it was made for another version of the game.") });
                    }
                }
            }
        }
    }

    // The script compiler: group errors by the mod whose script failed.
    let script_log = game_root.join("r6/logs/redscript_rCURRENT.log");
    if modified(&script_log).map(|t| t >= since) == Some(true) {
        at = at.max(modified(&script_log).unwrap_or(0));
        let text = fs::read_to_string(&script_log).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let mut other: BTreeMap<u64, (String, usize)> = BTreeMap::new();
        for (i, line) in lines.iter().enumerate() {
            if !line.starts_with("[ERROR") {
                continue;
            }
            let Some(at_i) = line.find("] At ") else { continue };
            let Some(rel) = rel_from(&line[at_i + 5..], &name) else { continue };
            let Some((id, n)) = owners.get(&rel) else { continue };
            // An import names the module it wanted: "import NightCityTrafficOverhaul.Tags.*".
            let wanted = lines.get(i + 1).and_then(|l| l.trim().strip_prefix("import ")).and_then(|m| m.split(['.', ' ', ';']).next()).map(str::to_string);
            match wanted {
                Some(module) => match owner_of_folder(owners, &format!("r6/scripts/{}", module.to_lowercase())) {
                    Some((oid, oname)) if oid != id => push(&mut issues, LogIssue {
                        mod_id: *id,
                        mod_name: n.clone(),
                        kind: "needsNewer".into(),
                        other_id: Some(*oid),
                        other_name: Some(oname.clone()),
                        detail: format!("Its scripts look for parts of {oname} that your version doesn't have. Usually {oname} needs updating."),
                    }),
                    Some(_) => {} // Its own native half: RED4ext's line says why.
                    None => push(&mut issues, LogIssue { mod_id: *id, mod_name: n.clone(), kind: "needsMod".into(), other_id: None, other_name: Some(module.clone()), detail: format!("Its scripts look for “{module}”, which no mod in this profile has. Check its page for what it needs.") }),
                },
                None => other.entry(*id).or_insert((n.clone(), 0)).1 += 1,
            }
        }
        for (id, (n, count)) in other {
            if !issues.iter().any(|x| x.mod_id == id) {
                push(&mut issues, LogIssue { mod_id: id, mod_name: n, kind: "scripts".into(), other_id: None, other_name: None, detail: format!("{count} script {} didn't compile. It may be made for another game version, or need a mod that's missing.", if count == 1 { "error" } else { "errors" }) });
            }
        }
    }
    // Script extenders and mod loaders for other games.
    for (i, t) in loader_issues(game_root, owners, prefix) {
        at = at.max(t);
        push(&mut issues, i);
    }
    // The game's crash record, when it's from this run (written after the scripts compiled).
    let started = modified(&script_log).unwrap_or(0);
    let crash = prefix.and_then(|p| crash_info(&p.join("drive_c/users/steamuser/AppData/Local/CD Projekt Red/Cyberpunk 2077/CrashInfo.json"), started)).map(|mut c| {
        if let Some((place, ours)) = crash_place(game_root, c.at) {
            c.place = Some(place);
            c.in_game_code = ours;
        }
        c
    });
    if let Some(c) = &crash {
        at = at.max(c.at);
    }
    let assets = asset_issues(game_root, owners, sources, crash.as_ref().map(|c| crate::log::local(c.at)));
    let smapi = smapi_log(prefix).map(|(text, t)| {
        at = at.max(t);
        let mut s = smapi_report(&text, owners);
        s.at = t;
        s
    });
    (at > 0).then_some(GameLog { at, before_change: at < since_change, issues, assets, crash, smapi })
}

/// The mod that put a file named `file` (any folder) in the game, matching without case.
fn owner_of_file<'a>(owners: &'a Owners, file: &str) -> Option<&'a (u64, String)> {
    let f = file.to_lowercase();
    owners.iter().find(|(k, _)| k.rsplit('/').next() == Some(f.as_str())).map(|(_, v)| v)
}

/// A mod whose name or folder matches `name` (for loaders that log a mod's own name).
fn owner_named<'a>(owners: &'a Owners, name: &str) -> Option<&'a (u64, String)> {
    let n = name.trim().to_lowercase();
    if n.len() < 3 {
        return None;
    }
    owners.values().find(|(_, m)| m.to_lowercase() == n).or_else(|| owners.iter().find(|(k, _)| k.split('/').any(|part| part == n)).map(|(_, v)| v))
}

/// What script extenders (SKSE, F4SE and friends), BepInEx and SMAPI say they refused to load
/// in their newest log, with when each log was written.
fn loader_issues(game_root: &Path, owners: &Owners, prefix: Option<&Path>) -> Vec<(LogIssue, u64)> {
    let mut out = vec![];
    let issue = |m: &(u64, String), kind: &str, detail: String| LogIssue { mod_id: m.0, mod_name: m.1.clone(), kind: kind.into(), other_id: None, other_name: None, detail };
    // Script extenders log to Documents/My Games/<game>/<XSE>/<xse>.log in the Proton prefix.
    if let Some(docs) = prefix.map(|p| p.join("drive_c/users/steamuser/Documents/My Games")) {
        for (folder, xse, log) in [
            ("Skyrim Special Edition", "SKSE", "skse64.log"),
            ("Skyrim Special Edition GOG", "SKSE", "skse64.log"),
            ("Skyrim VR", "SKSE", "sksevr.log"),
            ("Skyrim", "SKSE", "skse.log"),
            ("Fallout4", "F4SE", "f4se.log"),
            ("Fallout4VR", "F4SE", "f4sevr.log"),
            ("Starfield", "SFSE", "sfse.log"),
            ("Oblivion Remastered", "OBSE", "obse64.log"),
        ] {
            let path = docs.join(folder).join(xse).join(log);
            let Some(t) = modified(&path) else { continue };
            let text = fs::read_to_string(&path).unwrap_or_default();
            for line in text.lines().map(str::trim) {
                let low = line.to_lowercase();
                if !low.starts_with("plugin ") || low.contains("loaded correctly") {
                    continue;
                }
                let bad = ["couldn't load", "disabled", "incompatible", "not loaded", "failed"].iter().any(|w| low.contains(w));
                let Some(dll) = line.split_whitespace().nth(1).map(|d| d.rsplit(['\\', '/']).next().unwrap_or(d).to_string()) else { continue };
                if bad {
                    if let Some(m) = owner_of_file(owners, &dll) {
                        let why = line.rsplit(") ").next().unwrap_or("").trim();
                        out.push((issue(m, "loaderRefused", format!("{xse} didn't load {dll}{}. It's usually made for another version of the game: update the mod, or the game's {xse}.", if why.is_empty() { String::new() } else { format!(" ({why})") })), t));
                    }
                }
            }
        }
    }
    // OBSE for the original Oblivion writes its log in the game folder.
    let obse = game_root.join("obse.log");
    if let Some(t) = modified(&obse) {
        for line in fs::read_to_string(&obse).unwrap_or_default().lines().map(str::trim) {
            let low = line.to_lowercase();
            if !low.starts_with("plugin ") || low.contains("loaded correctly") || !["couldn't load", "disabled", "incompatible", "not loaded", "failed"].iter().any(|w| low.contains(w)) {
                continue;
            }
            let Some(dll) = line.split_whitespace().nth(1).map(|d| d.rsplit(['\\', '/']).next().unwrap_or(d).to_string()) else { continue };
            if let Some(m) = owner_of_file(owners, &dll) {
                out.push((issue(m, "loaderRefused", format!("OBSE didn't load {dll}. It's usually made for another version of the game or of OBSE.")), t));
            }
        }
    }
    // BepInEx (Unity games): the log in the game folder.
    let bep = game_root.join("BepInEx/LogOutput.log");
    if let Some(t) = modified(&bep) {
        for line in fs::read_to_string(&bep).unwrap_or_default().lines() {
            // "Could not load [Plugin Name 1.2.3] because it has missing dependencies: X"
            if let Some(i) = line.find("Could not load [") {
                let rest = &line[i + 16..];
                let name = rest.split(']').next().unwrap_or("").rsplit_once(' ').map(|(n, _)| n).unwrap_or("");
                if let Some(m) = owner_named(owners, name) {
                    let why = rest.split("because ").nth(1).unwrap_or("it had a problem").trim_end_matches('.');
                    out.push((issue(m, "loaderRefused", format!("BepInEx didn't load {name}: {why}.")), t));
                }
            }
        }
    }
    // Any other loader's log (MelonLoader, UE4SS, REFramework, Cyber Engine Tweaks, BepInEx at
    // play time): errors that name a mod's file or name.
    for (loader, path) in crate::loaders::logs(game_root) {
        let Some(t) = modified(&path) else { continue };
        if fs::metadata(&path).map(|m| m.len() > 20 * 1024 * 1024).unwrap_or(true) {
            continue;
        }
        let text = String::from_utf8_lossy(&fs::read(&path).unwrap_or_default()).into_owned();
        for (id, name, count, sample) in crate::loaders::errors_in(&text, owners).into_iter().take(8) {
            let errors = if count == 1 { "an error".to_string() } else { format!("{count} errors") };
            out.push((issue(&(id, name), "loaderErrors", format!("{loader}'s log has {errors} from it, like: “{sample}”")), t));
        }
    }
    // SMAPI (Stardew Valley): only the root causes (a mod skipped because another was is
    // summed up under that one).
    if let Some((text, t)) = smapi_log(prefix) {
        for b in smapi_report(&text, owners).broken {
            if let Some(m) = owner_named(owners, &b.name) {
                let others = if b.dependents.is_empty() { String::new() } else { format!(" {} other mod{} need{} it, so they were skipped too.", b.dependents.len(), if b.dependents.len() == 1 { "" } else { "s" }, if b.dependents.len() == 1 { "s" } else { "" }) };
                out.push((issue(m, "loaderRefused", format!("SMAPI skipped it: {}.{others}", b.why.trim_end_matches('.'))), t));
            }
        }
    }
    out
}

/// SMAPI's newest log (native, or in the Proton prefix) and when it was written. At most 20 MB.
fn smapi_log(prefix: Option<&Path>) -> Option<(String, u64)> {
    let paths = [
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/StardewValley/ErrorLogs/SMAPI-latest.txt")),
        prefix.map(|p| p.join("drive_c/users/steamuser/AppData/Roaming/StardewValley/ErrorLogs/SMAPI-latest.txt")),
    ];
    let (path, t) = paths.into_iter().flatten().filter_map(|p| modified(&p).map(|t| (p, t))).max_by_key(|(_, t)| *t)?;
    if fs::metadata(&path).ok()?.len() > 20 * 1024 * 1024 {
        return None;
    }
    Some((fs::read_to_string(&path).ok()?, t))
}

/// "Name 1.2.3" → ("Name", "1.2.3"); a name without a version stays whole.
fn name_version(head: &str) -> (String, String) {
    match head.rsplit_once(' ') {
        Some((n, v)) if v.chars().next().is_some_and(|c| c.is_ascii_digit()) => (n.trim().to_string(), v.to_string()),
        _ => (head.trim().to_string(), String::new()),
    }
}

/// Work out a SMAPI log.
pub fn smapi_report(text: &str, owners: &Owners) -> Smapi {
    let mut out = Smapi::default();
    // name → (version, why, the mod it waits for)
    let mut skipped: Vec<(String, String, String, Option<String>)> = vec![];
    let mut errors: BTreeMap<String, usize> = BTreeMap::new();
    let lines: Vec<&str> = text.lines().collect();
    // name → (count, what, last line number)
    let mut culprits: BTreeMap<String, (usize, String, usize)> = BTreeMap::new();
    for (i, l) in lines.iter().enumerate() {
        if !l.contains(" ERROR ") {
            continue;
        }
        let frames: Vec<&str> = lines[i + 1..].iter().take(40).take_while(|f| f.starts_with("   ") || f.trim_start().starts_with("at ")).copied().collect();
        if let Some(name) = culprit_in(&frames) {
            let what = l.split_once("]").map(|(_, b)| b).unwrap_or(l).trim();
            let what = what.rsplit(": ").take(2).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(": ");
            let e = culprits.entry(name).or_insert((0, String::new(), 0));
            e.0 += 1;
            e.1 = what.chars().take(160).collect();
            e.2 = i;
        }
        if l.contains("The game failed to launch") || l.contains("game has crashed") || l.contains("SMAPI-crash") {
            out.crashed = true;
        }
    }
    let last_line = lines.len();
    let (mut in_skipped, mut in_updates) = (false, false);
    for line in text.lines() {
        let body = if line.starts_with('[') { line.split_once(']').map(|(_, b)| b).unwrap_or(line) } else { line };
        let b = body.trim();
        // "SMAPI 4.5.2 with Stardew Valley 1.6.15 build 24356 on Microsoft Windows …"
        if out.smapi_version.is_none() {
            if let Some(rest) = b.strip_prefix("SMAPI ") {
                if let Some((sv, game)) = rest.split_once(" with Stardew Valley ") {
                    out.smapi_version = Some(sv.trim().to_string());
                    out.game_version = game.split_whitespace().next().map(String::from);
                }
            }
        }
        if b.contains("Skipped mods") {
            in_skipped = true;
            continue;
        }
        if b.starts_with("You can update") && b.ends_with("mods:") || b.starts_with("You can update") && b.ends_with("mod:") {
            in_updates = true;
            continue;
        }
        if in_skipped {
            if let Some(item) = b.strip_prefix("- ") {
                if let Some((head, why)) = item.split_once(" because ") {
                    let (name, version) = name_version(head);
                    let waits = why.split("needs the '").nth(1).and_then(|r| r.split("' mod").next()).map(String::from);
                    skipped.push((name, version, why.trim_end_matches('.').to_string(), waits));
                }
                continue;
            }
            if !(b.is_empty() || b.starts_with("---") || b.starts_with("These mods")) {
                in_skipped = false;
            }
        }
        if in_updates {
            // "Content Patcher 2.9.1: https://www.nexusmods.com/stardewvalley/mods/1915 (you have 2.3.0)"
            if let Some((head, rest)) = b.split_once(": http") {
                let (name, version) = name_version(head);
                let have = rest.split("(you have ").nth(1).map(|h| h.trim_end_matches(')').to_string()).unwrap_or_default();
                let nexus_id = rest.split("nexusmods.com/stardewvalley/mods/").nth(1).and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()).and_then(|n| n.parse().ok());
                out.updates.push(SmapiUpdate { name, have, version, nexus_id });
                continue;
            }
            in_updates = false;
        }
        // "[17:03:28 ERROR Source] …": count by source, outside the skipped list.
        if let Some(tag) = line.split_once(" ERROR ").map(|(_, r)| r).and_then(|r| r.split_once(']')).map(|(src, _)| src.trim()) {
            if !in_skipped {
                *errors.entry(tag.to_string()).or_insert(0) += 1;
            }
        }
    }
    out.skipped = skipped.len();
    // Root causes: skipped for their own reason, not waiting on another skipped mod.
    let waiting_on = |name: &str| -> Vec<String> {
        let mut found: Vec<String> = vec![];
        let mut todo = vec![name.to_string()];
        while let Some(n) = todo.pop() {
            for (m, _, _, w) in &skipped {
                if w.as_deref() == Some(n.as_str()) && !found.contains(m) && m != name {
                    found.push(m.clone());
                    todo.push(m.clone());
                }
            }
        }
        found
    };
    for (name, version, why, waits) in &skipped {
        if waits.as_ref().is_some_and(|w| skipped.iter().any(|(n, ..)| n == w)) {
            continue;
        }
        let low = why.to_lowercase();
        let (kind, needs) = if let Some(list) = why.split("aren't installed (").nth(1) {
            ("missing", list.trim_end_matches(')').split(", ").map(String::from).collect())
        } else if let Some(w) = waits {
            ("missing", vec![w.clone()])
        } else if low.contains("no longer compatible") || low.contains("newer version") || low.contains("assume broken") || low.contains("obsolete") || low.contains("older version") {
            ("old", vec![])
        } else {
            ("other", vec![])
        };
        out.broken.push(Broken { name: name.clone(), version: version.clone(), mod_id: owner_named(owners, name).map(|m| m.0), kind: kind.into(), why: why.clone(), needs, dependents: waiting_on(name) });
    }
    out.broken.sort_by(|a, b| b.dependents.len().cmp(&a.dependents.len()).then(a.name.cmp(&b.name)));
    let mut c: Vec<Culprit> = culprits
        .into_iter()
        .map(|(name, (count, what, at))| {
            // Named after the mod's code; the profile's mods are named for people.
            let shown = owners.values().find(|(_, m)| same_name(m, &name)).map(|(_, m)| m.clone()).or_else(|| skipped.iter().map(|(n, ..)| n.clone()).find(|n| same_name(n, &name))).unwrap_or(name.clone());
            let mod_id = owners.values().find(|(_, m)| same_name(m, &name)).map(|(id, _)| *id);
            Culprit { name: shown, mod_id, count, what, at_the_end: last_line.saturating_sub(at) < 200 }
        })
        .collect();
    c.sort_by(|a, b| b.count.cmp(&a.count));
    c.truncate(5);
    out.culprits = c;
    let mut e: Vec<(String, usize)> = errors.into_iter().collect();
    e.sort_by(|a, b| b.1.cmp(&a.1));
    e.truncate(6);
    out.errors = e;
    out
}

fn crash_info(path: &Path, started: u64) -> Option<Crash> {
    let t = modified(path)?;
    if t <= started {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let pm = &v["Data"]["postMortem"];
    let secs = pm["sessionLength"].as_f64().unwrap_or(0.0);
    Some(Crash { at: t, minutes: (secs / 60.0).round() as u64, place: None, in_game_code: false, out_of_memory: pm["isOom"].as_bool().unwrap_or(false) })
}

/// What a crash reporter (AppearanceRuntimePlugin's CrashProbe) said, for a crash at `at`.
fn crash_place(game_root: &Path, at: u64) -> Option<(String, bool)> {
    let dir = game_root.join("red4ext/plugins/AppearanceRuntimePlugin/Logs");
    let f = fs::read_dir(dir).ok()?.flatten().filter(|e| e.file_name().to_string_lossy().starts_with("crash-")).max_by_key(|e| modified(&e.path()).unwrap_or(0))?;
    if modified(&f.path())?.abs_diff(at) > 120 {
        return None;
    }
    let text = fs::read_to_string(f.path()).ok()?;
    let line = text.lines().find(|l| l.contains("ACCESS VIOLATION") || l.contains("EXCEPTION"))?;
    let place = line.split_whitespace().find(|w| w.contains("+0x"))?.to_string();
    // Every module on the trace is the game itself: nothing from a mod's program.
    let ours = text.lines().filter(|l| l.contains("+0x")).all(|l| l.contains("Cyberpunk2077.exe"));
    Some((place, ours))
}

/// FNV-1a 64, which archives use to name what's in them.
fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// The hashed paths a Cyberpunk .archive holds (its index; the rest isn't read).
pub fn archive_hashes(path: &Path) -> Vec<u64> {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(mut f) = fs::File::open(path) else { return vec![] };
    let mut hdr = [0u8; 20];
    if f.read_exact(&mut hdr).is_err() || &hdr[..4] != b"RDAR" {
        return vec![];
    }
    let pos = u64::from_le_bytes(hdr[8..16].try_into().unwrap());
    let size = u32::from_le_bytes(hdr[16..20].try_into().unwrap()) as usize;
    if size > 64 << 20 || f.seek(SeekFrom::Start(pos)).is_err() {
        return vec![];
    }
    let mut idx = vec![0u8; size];
    if f.read_exact(&mut idx).is_err() || idx.len() < 28 {
        return vec![];
    }
    let n = u32::from_le_bytes(idx[16..20].try_into().unwrap()) as usize;
    (0..n).filter_map(|i| idx.get(28 + i * 56..36 + i * 56)).map(|b| u64::from_le_bytes(b.try_into().unwrap())).collect()
}

/// "[2026-11-22 09:07:39.653]" → that local time, as a number to compare.
fn line_time(line: &str) -> Option<u64> {
    let t = line.strip_prefix('[')?.get(..19)?;
    crate::requirements::unix_time(&format!("{}T{}Z", &t[..10], &t[11..]))
}

fn quoted(line: &str) -> Vec<&str> {
    line.split('"').skip(1).step_by(2).filter(|p| p.contains('\\') || p.contains('/') || p.contains('.')).collect()
}

/// ArchiveXL's and TweakXL's complaints, traced to mods: a path is found in a mod's archive
/// (it has the broken thing) or in a mod's .xl/.yaml/.tweak file (it points at the missing
/// thing); a tweak record is found in a mod's tweak file.
fn asset_issues(game_root: &Path, owners: &Owners, sources: &HashMap<String, (String, PathBuf)>, crash_local: Option<u64>) -> Vec<AssetIssue> {
    let newest = |dir: &Path, start: &str| -> Option<PathBuf> {
        fs::read_dir(dir).ok()?.flatten().filter(|e| e.file_name().to_string_lossy().starts_with(start) && e.file_name().to_string_lossy().ends_with(".log")).max_by_key(|e| modified(&e.path()).unwrap_or(0)).map(|e| e.path())
    };
    let axl = newest(&game_root.join("red4ext/plugins/ArchiveXL"), "ArchiveXL-");
    let txl = newest(&game_root.join("red4ext/plugins/TweakXL"), "TweakXL-");
    if axl.is_none() && txl.is_none() {
        return vec![];
    }
    // What each mod's archives hold, and its text configs.
    let mut by_hash: HashMap<u64, u64> = HashMap::new();
    let mut texts: Vec<(u64, String)> = vec![];
    for (rel, (_, from)) in sources {
        let Some(&(id, _)) = owners.get(rel) else { continue };
        if rel.ends_with(".archive") {
            for h in archive_hashes(from) {
                by_hash.insert(h, id);
            }
        } else if [".xl", ".yaml", ".yml", ".tweak"].iter().any(|e| rel.ends_with(e)) {
            if let Ok(t) = fs::read_to_string(from) {
                texts.push((id, t.to_lowercase().replace("\\\\", "\\")));
            }
        }
    }
    let name_of: HashMap<u64, String> = owners.values().map(|(i, n)| (*i, n.clone())).collect();
    let mut found: BTreeMap<u64, AssetIssue> = BTreeMap::new();
    let mut note = |id: u64, msg: &str, t: Option<u64>| {
        let near = matches!((crash_local, t), (Some(c), Some(t)) if t <= c + 5 && c - t.min(c) <= 300);
        let e = found.entry(id).or_insert_with(|| AssetIssue { mod_id: id, mod_name: name_of.get(&id).cloned().unwrap_or_default(), count: 0, sample: msg.chars().take(200).collect(), near_crash: false });
        e.count += 1;
        if near && !e.near_crash {
            e.near_crash = true;
            e.sample = msg.chars().take(200).collect();
        }
    };
    let find_text = |needle: &str| texts.iter().find(|(_, t)| t.contains(needle)).map(|(id, _)| *id);
    for path in [axl, txl].into_iter().flatten() {
        let is_tweak = path.to_string_lossy().contains("TweakXL");
        let text = fs::read_to_string(&path).unwrap_or_default();
        // Most errors come while the game loads; only ones after that (two minutes past the
        // log's first line) happened during play and can point at a crash.
        let playing_from = text.lines().find_map(line_time).map(|t| t + 120).unwrap_or(u64::MAX);
        for line in text.lines() {
            if !line.contains("[error]") && !(is_tweak && line.contains("[warning]")) {
                continue;
            }
            let t = line_time(line);
            let msg = line.split_once("] [error] ").map(|x| x.1).or_else(|| line.split_once("] [warning] ").map(|x| x.1)).unwrap_or(line).trim();
            let mut who = None;
            if is_tweak {
                // "Items.Preset_Unity_Agent: Unknown property …": the record's name.
                if let Some(record) = msg.split(':').next().filter(|r| r.contains('.') && !r.contains(' ')) {
                    let short = record.to_lowercase();
                    who = find_text(&short).or_else(|| short.split_once('.').and_then(|(_, rest)| find_text(rest)));
                }
            } else {
                for p in quoted(msg) {
                    let p = p.to_lowercase();
                    who = by_hash.get(&fnv(&p)).copied().or_else(|| find_text(&p)).or_else(|| find_text(&p.replace('\\', "/")));
                    if who.is_some() {
                        break;
                    }
                }
            }
            if let Some(id) = who {
                note(id, msg, t.filter(|t| *t >= playing_from));
            }
        }
    }
    let mut out: Vec<AssetIssue> = found.into_values().collect();
    out.sort_by(|a, b| b.near_crash.cmp(&a.near_crash).then(b.count.cmp(&a.count)));
    out
}

#[cfg(test)]
mod loader_tests {
    use super::*;

    #[test]
    fn finds_smapi_root_causes() {
        let log = "[17:03:01 INFO  SMAPI] SMAPI 4.5.2 with Stardew Valley 1.6.15 build 24356 on Microsoft Windows 10
[17:03:02 ERROR SMAPI]    Skipped mods
[17:03:02 ERROR SMAPI]    --------------------------------------------------
[17:03:02 ERROR SMAPI]       These mods could not be added to your game.
[17:03:02 ERROR SMAPI]
[17:03:02 ERROR SMAPI]       - Content Patcher 2.3.0 because it's no longer compatible. Please check for a new version at https://www.nexusmods.com/stardewvalley/mods/1915 or https://smapi.io/mods
[17:03:02 ERROR SMAPI]       - Hat Shop 2.0.0 because it needs the 'Content Patcher' mod, which couldn't be loaded.
[17:03:02 ERROR SMAPI]       - (CP) Hat Shop 1.1.2 because it needs the 'Hat Shop' mod, which couldn't be loaded.
[17:03:02 ERROR SMAPI]       - Odd Mod 1.0 because it needs mods which aren't installed (SpaceCore, Thing).
[17:03:02 INFO  SMAPI] Loaded 3 mods:
[17:03:04 ALERT SMAPI] You can update 2 mods:
[17:03:04 ALERT SMAPI]    Content Patcher 2.9.1: https://www.nexusmods.com/stardewvalley/mods/1915 (you have 2.3.0)
[17:03:04 ALERT SMAPI]    Anything Anywhere 1.8.2-unofficial.7: https://smapi.io/mods#anything (you have 1.8.1)
[17:03:04 INFO  SMAPI] Type 'help' for help.
[17:03:28 ERROR SMAPI] An error occurred in the game's draw loop: ArgumentNullException
[17:03:28 ERROR SMAPI] An error occurred in the game's draw loop: ArgumentNullException
[17:03:29 ERROR Dynamic Reflections] Issue with Harmony patching";
        let mut owners = Owners::new();
        owners.insert("mods/contentpatcher/manifest.json".into(), (1915, "Content Patcher".into()));
        let s = smapi_report(log, &owners);
        assert_eq!(s.game_version.as_deref(), Some("1.6.15"));
        assert_eq!(s.smapi_version.as_deref(), Some("4.5.2"));
        assert_eq!(s.skipped, 4);
        assert_eq!(s.broken.len(), 2, "Hat Shop and its add-on are summed up under Content Patcher");
        assert_eq!(s.broken[0].name, "Content Patcher");
        assert_eq!(s.broken[0].kind, "old");
        assert_eq!(s.broken[0].mod_id, Some(1915));
        assert_eq!(s.broken[0].dependents, vec!["Hat Shop", "(CP) Hat Shop"]);
        assert_eq!(s.broken[1].kind, "missing");
        assert_eq!(s.broken[1].needs, vec!["SpaceCore", "Thing"]);
        assert_eq!(s.updates.len(), 2);
        assert_eq!(s.updates[0].nexus_id, Some(1915));
        assert_eq!(s.updates[1].nexus_id, None);
        assert_eq!(s.errors[0], ("SMAPI".to_string(), 2));
    }

    /// Run with SMAPI_LOG=<path> to see what a real log comes to.
    #[test]
    #[ignore]
    fn reads_a_real_smapi_log() {
        let text = fs::read_to_string(std::env::var("SMAPI_LOG").unwrap()).unwrap();
        let s = smapi_report(&text, &Owners::new());
        println!("{}", serde_json::to_string_pretty(&s).unwrap());
    }

    #[test]
    fn reads_script_extender_and_smapi_logs() {
        let d = tempfile::tempdir().unwrap();
        let game = d.path().join("Skyrim Special Edition");
        let pfx = d.path().join("pfx");
        let skse = pfx.join("drive_c/users/steamuser/Documents/My Games/Skyrim Special Edition/SKSE");
        fs::create_dir_all(&skse).unwrap();
        fs::write(skse.join("skse64.log"), "checking plugin C:\\Game\\Data\\SKSE\\Plugins\\good.dll\nplugin good.dll (00000001 Good 00000001) loaded correctly\nplugin Old.dll (00000001 Old 00000001) disabled, address library needs to be updated\n").unwrap();
        let smapi = pfx.join("drive_c/users/steamuser/AppData/Roaming/StardewValley/ErrorLogs");
        fs::create_dir_all(&smapi).unwrap();
        fs::write(smapi.join("SMAPI-latest.txt"), "[12:00:00 ERROR SMAPI] Skipped mods\n[12:00:00 ERROR SMAPI]    - Better Barns 1.2.0 because it needs mods which aren't installed (SpaceCore).\n").unwrap();
        let mut owners = Owners::new();
        owners.insert("data/skse/plugins/old.dll".into(), (7, "Old Plugin".into()));
        owners.insert("data/skse/plugins/good.dll".into(), (8, "Good Plugin".into()));
        owners.insert("mods/better barns/manifest.json".into(), (9, "Better Barns".into()));
        let got = loader_issues(&game, &owners, Some(&pfx));
        let ids: Vec<u64> = got.iter().map(|(i, _)| i.mod_id).collect();
        assert_eq!(ids, vec![7, 9]);
        assert!(got[0].0.detail.contains("address library"));
        assert!(got[1].0.detail.contains("SpaceCore"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traces_log_errors_to_mods() {
        let dir = tempfile::tempdir().unwrap();
        let g = dir.path().join("Cyberpunk 2077");
        fs::create_dir_all(g.join("red4ext/logs")).unwrap();
        fs::create_dir_all(g.join("r6/logs")).unwrap();
        fs::write(
            g.join("red4ext/logs/red4ext-1.log"),
            "[info] [RED4ext] Loading plugin from 'S:\\steamapps\\common\\Cyberpunk 2077\\red4ext\\plugins\\XUtils\\XUtilsNative.dll'...\n[warning] [RED4ext] XUtilsNative (version: 1.0.0) is incompatible with the current patch. The requested runtime of the plugin is 3.0.80\n",
        )
        .unwrap();
        fs::write(
            g.join("r6/logs/redscript_rCURRENT.log"),
            "[ERROR - x] [UNRESOLVED_IMPORT] At S:\\steamapps\\common\\Cyberpunk 2077\\r6\\scripts\\FactionsEvolved\\FE_Config.reds:6:1:\nimport NightCityTrafficOverhaul.Tags.*\n[ERROR - x] [UNRESOLVED_IMPORT] At S:\\steamapps\\common\\Cyberpunk 2077\\r6\\scripts\\XUtils\\Camera\\XUtilsCamera.reds:4:1:\nimport XUtils.Native.*\n[ERROR - x] [UNRESOLVED_IMPORT] At S:\\steamapps\\common\\Cyberpunk 2077\\r6\\scripts\\Lonely\\a.reds:1:1:\nimport Codeware.*\n",
        )
        .unwrap();
        let mut owners = Owners::new();
        owners.insert("red4ext/plugins/xutils/xutilsnative.dll".into(), (30296, "XUtils".into()));
        owners.insert("r6/scripts/xutils/camera/xutilscamera.reds".into(), (30296, "XUtils".into()));
        owners.insert("r6/scripts/factionsevolved/fe_config.reds".into(), (25873, "Factions Evolved".into()));
        owners.insert("r6/scripts/nightcitytrafficoverhaul/tags.reds".into(), (25552, "Night City Traffic Overhaul".into()));
        owners.insert("r6/scripts/lonely/a.reds".into(), (7, "Lonely".into()));
        let none = HashMap::new();
        let log = read(&g, &owners, &none, None, 0).unwrap();
        let got: Vec<(u64, &str, Option<u64>)> = log.issues.iter().map(|i| (i.mod_id, i.kind.as_str(), i.other_id)).collect();
        assert!(got.contains(&(30296, "oldPlugin", None)), "{got:?}");
        assert!(got.contains(&(25873, "needsNewer", Some(25552))), "{got:?}");
        assert!(got.contains(&(7, "needsMod", None)), "{got:?}");
        assert!(!got.iter().any(|(id, k, _)| *id == 30296 && *k != "oldPlugin"), "its own native half isn't a second problem");
        assert!(read(&g, &owners, &none, None, u64::MAX).unwrap().before_change, "logs from before the mods went in are marked");
        assert!(!log.before_change);
    }

    #[test]
    fn traces_asset_errors_and_crashes() {
        let dir = tempfile::tempdir().unwrap();
        let g = dir.path().join("Cyberpunk 2077");
        let lib = dir.path().join("lib");
        fs::create_dir_all(g.join("r6/logs")).unwrap();
        fs::create_dir_all(g.join("red4ext/plugins/ArchiveXL")).unwrap();
        fs::create_dir_all(g.join("red4ext/plugins/TweakXL")).unwrap();
        fs::create_dir_all(&lib).unwrap();
        // An archive holding the eye mesh: a header, then an index with one entry.
        let mut a = b"RDAR".to_vec();
        a.extend(12u32.to_le_bytes());
        a.extend(40u64.to_le_bytes());
        a.extend((28u32 + 56).to_le_bytes());
        a.resize(40, 0);
        let mut idx = vec![0u8; 16];
        idx.extend(1u32.to_le_bytes());
        idx.resize(28, 0);
        idx.extend(fnv("protossvoid\\cosplayeyesccxl\\meshes\\patch.mesh").to_le_bytes());
        idx.resize(28 + 56, 0);
        a.extend(idx);
        assert_eq!(archive_hashes(&{ let p = lib.join("eyes.archive"); fs::write(&p, &a).unwrap(); p }), vec![fnv("protossvoid\\cosplayeyesccxl\\meshes\\patch.mesh")]);
        fs::write(lib.join("outfit.xl"), "factories:\n  - base\\xl_stuff\\glamor_biker_outfit.csv\n").unwrap();
        fs::write(lib.join("unity.yaml"), "Items.Preset_Unity_Agent:\n  headshotBonus: 2\n").unwrap();
        fs::write(g.join("r6/logs/redscript_rCURRENT.log"), "ok\n").unwrap();
        fs::write(
            g.join("red4ext/plugins/ArchiveXL/ArchiveXL-1.log"),
            "[2026-11-22 09:00:36.000] [380] [error] [FactoryIndex] Factory \"base\\xl_stuff\\glamor_biker_outfit.csv\" doesn't exist. Skipped.\n[2026-11-22 09:07:39.653] [708] [error] [DynamicMesh] Material \"eyelashes_MAT\" of \"protossvoid\\cosplayeyesccxl\\meshes\\patch.mesh\" is not defined\n",
        )
        .unwrap();
        fs::write(g.join("red4ext/plugins/TweakXL/TweakXL-1.log"), "[2026-11-22 09:00:57.848] [728] [error] Items.Preset_Unity_Agent: Unknown property headshotBonus.\n").unwrap();
        let mut owners = Owners::new();
        let mut sources = HashMap::new();
        for (rel, id, name, file) in [("archive/pc/mod/eyes.archive", 1, "Eyes", "eyes.archive"), ("archive/pc/mod/outfit.xl", 2, "Biker", "outfit.xl"), ("r6/tweaks/unity.yaml", 3, "Unity", "unity.yaml")] {
            owners.insert(rel.to_string(), (id, name.to_string()));
            sources.insert(rel.to_string(), (rel.to_string(), lib.join(file)));
        }
        // A crash at 09:10 local: the eye errors (3 minutes before) are near it, the others aren't.
        let crash_local = crate::requirements::unix_time("2026-11-22T09:10:29Z");
        let got = asset_issues(&g, &owners, &sources, crash_local);
        let ids: Vec<(u64, bool)> = got.iter().map(|a| (a.mod_id, a.near_crash)).collect();
        assert_eq!(ids[0], (1, true), "{got:?}");
        assert!(ids.contains(&(2, false)) && ids.contains(&(3, false)), "{got:?}");
    }
}
