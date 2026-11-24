//! Thunderstore (thunderstore.io), next to Nexus Mods: the same library and profiles, without
//! Overwolf. Which games it has, where each kind of file goes and which package is the mod
//! loader all come from Thunderstore's public "ecosystem" file, the one r2modman uses. A game's
//! mod list is a handful of gzipped files on its CDN, fetched at most once an hour and searched
//! here. Downloads are open (no account), and a mod's requirements come with it.
//!
//! Mods from Thunderstore get numbers far above Nexus Mods' (and above the ones for your own
//! files), so they're never looked up on Nexus Mods or put in a Nexus Mods pack.

use crate::library::{self, StoredFile, StoredMod};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime},
};

/// Thunderstore mods' numbers start here.
pub const IDS: u64 = 2 * library::LOCAL_IDS;
const SCHEMA: &str = "https://thunderstore.io/api/experimental/schema/dev/latest/";
/// A gzipped list, or a part of one, is never bigger than this.
const MAX_LIST: u64 = 256 * 1024 * 1024;

fn fnv(s: &str) -> u64 {
    s.to_lowercase().bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3)) % library::LOCAL_IDS
}
/// The library number for a package ("Owner-Name").
pub fn mod_id(full: &str) -> u64 {
    IDS + fnv(full)
}
/// The library number for one version of it ("Owner-Name-1.2.3").
pub fn file_id(full_version: &str) -> u64 {
    IDS + fnv(full_version)
}

/// Where a kind of file goes (from the ecosystem file).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rule {
    pub route: String,
    pub exts: Vec<String>,
    /// Each package gets its own folder there ("BepInEx/plugins/Owner-Name/").
    pub subdir: bool,
    pub default: bool,
}

/// A game on Thunderstore.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TsGame {
    pub community: String,
    pub name: String,
    pub loader: String,
    /// Zom's Nexus can put this loader's mods in.
    pub supported: bool,
    /// r2modman's folder name for the game (its profiles live under it).
    #[serde(skip_serializing)]
    pub r2_folder: String,
    #[serde(skip_serializing)]
    pub rules: Vec<Rule>,
    /// Loader packages ("bbepis-BepInExPack") → the folder in them that goes in the game folder.
    #[serde(skip_serializing)]
    pub loaders: HashMap<String, String>,
}

fn get_gz(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut out = vec![];
    if bytes.starts_with(&[0x1f, 0x8b]) {
        GzDecoder::new(bytes).take(MAX_LIST).read_to_end(&mut out).map_err(|e| format!("Thunderstore's list didn't unpack: {e}"))?;
    } else {
        out = bytes.to_vec();
    }
    Ok(out)
}

async fn fetch(http: &reqwest::Client, url: &str) -> Result<Vec<u8>, String> {
    let r = http.get(url).send().await.map_err(|e| format!("Couldn't reach Thunderstore: {e}"))?;
    if !r.status().is_success() {
        return Err(format!("Thunderstore answered {}.", r.status()));
    }
    if r.content_length().is_some_and(|n| n > MAX_LIST) {
        return Err("Thunderstore's list is far bigger than expected.".into());
    }
    let b = r.bytes().await.map_err(|e| format!("Couldn't read Thunderstore's answer: {e}"))?;
    get_gz(&b)
}

fn fresh(p: &Path, max: Duration) -> bool {
    fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok()).is_some_and(|age| age < max)
}

/// The ecosystem file, kept a day (an older copy is used when Thunderstore can't be reached).
async fn schema(http: &reqwest::Client, cache: &Path) -> Result<Value, String> {
    let file = cache.join("thunderstore/schema.json");
    if !fresh(&file, Duration::from_secs(86_400)) {
        match fetch(http, SCHEMA).await {
            Ok(b) => {
                let _ = fs::create_dir_all(file.parent().unwrap());
                let _ = fs::write(&file, &b);
            }
            Err(e) if !file.is_file() => return Err(e),
            Err(_) => {}
        }
    }
    serde_json::from_slice(&fs::read(&file).map_err(|e| e.to_string())?).map_err(|e| format!("Thunderstore's game list didn't read: {e}"))
}

/// The game in the ecosystem file: by Steam app number, or by name.
pub fn game_in(schema: &Value, steam: Option<u32>, domain: &str, name: &str) -> Option<TsGame> {
    let games = schema.get("games")?.as_object()?;
    let squash = |s: &str| s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase();
    let (label, g) = games.iter().find(|(_, g)| {
        steam.is_some_and(|id| g["distributions"].as_array().is_some_and(|d| d.iter().any(|x| x["platform"] == "steam" && x["identifier"].as_str() == Some(&id.to_string()))))
    }).or_else(|| games.iter().find(|(label, g)| squash(label) == squash(domain) || g["meta"]["displayName"].as_str().is_some_and(|n| squash(n) == squash(name))))?;
    let r = g["r2modman"].as_array()?.first()?;
    let community = r["packageIndex"].as_str().and_then(|u| u.split("/c/").nth(1)).and_then(|s| s.split('/').next()).unwrap_or(label).to_string();
    let loader = r["packageLoader"].as_str().unwrap_or("none").to_string();
    // Install rules, with sub-routes flattened ("MelonLoader" › "Managed" = "MelonLoader/Managed").
    fn rules_of(list: &Value, base: &str, out: &mut Vec<Rule>, zipped: &mut bool) {
        for x in list.as_array().into_iter().flatten() {
            let route = x["route"].as_str().unwrap_or("");
            let route = if base.is_empty() { route.to_string() } else { format!("{base}/{route}") };
            let how = x["trackingMethod"].as_str().unwrap_or("");
            *zipped |= how == "package-zip";
            out.push(Rule {
                route: route.clone(),
                exts: x["defaultFileExtensions"].as_array().map(|e| e.iter().filter_map(|v| v.as_str().map(str::to_lowercase)).collect()).unwrap_or_default(),
                subdir: matches!(how, "subdir" | "subdir-no-flatten"),
                default: x["isDefaultLocation"].as_bool().unwrap_or(false),
            });
            rules_of(&x["subRoutes"], &route, out, zipped);
        }
    }
    let mut rules = vec![];
    let mut zipped = false;
    rules_of(&r["installRules"], "", &mut rules, &mut zipped);
    let loader_name = r["packageLoader"].as_str().unwrap_or("none");
    // Loaders r2modman installs by its own rules, not the ecosystem file's.
    if rules.is_empty() {
        let rule = |route: &str, subdir: bool| Rule { route: route.into(), exts: vec![], subdir, default: true };
        match loader_name {
            // Mods laid out as they go in the game folder.
            "recursive-melonloader" => rules.push(rule("", false)),
            "gdweave" => rules.push(rule("GDWeave/mods", true)),
            "return-of-modding" => rules.push(rule("ReturnOfModding/plugins", true)),
            _ => {}
        }
    }
    let loaders = schema["modloaderPackages"].as_array().map(|a| a.iter().filter(|p| p["loader"].as_str() == Some(&loader)).filter_map(|p| Some((p["packageId"].as_str()?.to_lowercase(), p["rootFolder"].as_str().unwrap_or("").to_string()))).collect()).unwrap_or_default();
    let r2_folder = r["internalFolderName"].as_str().unwrap_or("").to_string();
    // Zom's Nexus can put mods in when it knows where each file goes, and mods are folders
    // (not zips that go in whole, like Godot's).
    let supported = !rules.is_empty() && !zipped && loader != "none";
    Some(TsGame { community, name: g["meta"]["displayName"].as_str().unwrap_or(label).to_string(), supported, loader, r2_folder, rules, loaders })
}

pub async fn game(http: &reqwest::Client, cache: &Path, steam: Option<u32>, domain: &str, name: &str) -> Result<Option<TsGame>, String> {
    Ok(game_in(&schema(http, cache).await?, steam, domain, name))
}

/// One package, with what's needed of each version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    pub full_name: String,
    pub owner: String,
    pub name: String,
    pub url: String,
    pub rating: u64,
    pub deprecated: bool,
    pub nsfw: bool,
    pub categories: Vec<String>,
    pub updated: String,
    /// Newest first.
    pub versions: Vec<Version>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub number: String,
    pub description: String,
    pub icon: String,
    pub dependencies: Vec<String>,
    pub download_url: String,
    pub downloads: u64,
    pub size: u64,
}

fn package_of(v: &Value) -> Option<Package> {
    Some(Package {
        full_name: v["full_name"].as_str()?.to_string(),
        owner: v["owner"].as_str().unwrap_or("").to_string(),
        name: v["name"].as_str().unwrap_or("").to_string(),
        url: v["package_url"].as_str().unwrap_or("").to_string(),
        rating: v["rating_score"].as_u64().unwrap_or(0),
        deprecated: v["is_deprecated"].as_bool().unwrap_or(false),
        nsfw: v["has_nsfw_content"].as_bool().unwrap_or(false),
        categories: v["categories"].as_array().map(|c| c.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
        updated: v["date_updated"].as_str().unwrap_or("").to_string(),
        versions: v["versions"]
            .as_array()?
            .iter()
            .enumerate()
            .map(|(i, x)| Version {
                number: x["version_number"].as_str().unwrap_or("").to_string(),
                // Only the newest version's words and picture are shown: big communities have
                // tens of thousands of mods with many versions each.
                description: if i == 0 { x["description"].as_str().unwrap_or("").to_string() } else { String::new() },
                icon: if i == 0 { x["icon"].as_str().unwrap_or("").to_string() } else { String::new() },
                dependencies: x["dependencies"].as_array().map(|d| d.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default(),
                download_url: x["download_url"].as_str().unwrap_or("").to_string(),
                downloads: x["downloads"].as_u64().unwrap_or(0),
                size: x["file_size"].as_u64().unwrap_or(0),
            })
            .collect(),
    })
}

static LISTS: Mutex<Vec<(String, SystemTime, std::sync::Arc<Vec<Package>>)>> = Mutex::new(Vec::new());

/// A community's packages, kept an hour (in memory and on disk).
pub async fn listing(http: &reqwest::Client, cache: &Path, community: &str) -> Result<std::sync::Arc<Vec<Package>>, String> {
    if let Some((_, _, l)) = LISTS.lock().unwrap().iter().find(|(c, t, _)| c == community && SystemTime::now().duration_since(*t).unwrap_or_default() < Duration::from_secs(3600)) {
        return Ok(l.clone());
    }
    let safe = library::safe_name(community);
    let file = cache.join("thunderstore").join(format!("{safe}.json"));
    let list: Vec<Package> = if fresh(&file, Duration::from_secs(3600)) {
        serde_json::from_slice(&fs::read(&file).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?
    } else {
        let got = async {
            let index: Vec<String> = serde_json::from_slice(&fetch(http, &format!("https://thunderstore.io/c/{community}/api/v1/package-listing-index/")).await?).map_err(|e| format!("Thunderstore's list didn't read: {e}"))?;
            let mut all = vec![];
            for url in index.iter().filter(|u| u.starts_with("https://")) {
                let part: Vec<Value> = serde_json::from_slice(&fetch(http, url).await?).map_err(|e| format!("Thunderstore's list didn't read: {e}"))?;
                all.extend(part.iter().filter_map(package_of));
            }
            Ok::<_, String>(all)
        }
        .await;
        match got {
            Ok(all) => {
                let _ = fs::create_dir_all(file.parent().unwrap());
                let _ = fs::write(&file, serde_json::to_vec(&all).unwrap_or_default());
                all
            }
            Err(e) => match fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
                Some(old) => old,
                None => return Err(e),
            },
        }
    };
    let list = std::sync::Arc::new(list);
    let mut l = LISTS.lock().unwrap();
    l.retain(|(c, _, _)| c != community);
    l.push((community.to_string(), SystemTime::now(), list.clone()));
    Ok(list)
}

/// A search result.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub full_name: String,
    pub owner: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub version: String,
    pub downloads: u64,
    pub rating: u64,
    pub size: u64,
    pub url: String,
    pub deprecated: bool,
    pub nsfw: bool,
    pub modpack: bool,
    pub categories: Vec<String>,
    pub updated: String,
    /// How many other packages it brings with it.
    pub needs: usize,
}

fn found(p: &Package) -> Option<Found> {
    let v = p.versions.first()?;
    Some(Found {
        full_name: p.full_name.clone(),
        owner: p.owner.clone(),
        name: p.name.clone(),
        description: v.description.clone(),
        icon: v.icon.clone(),
        version: v.number.clone(),
        downloads: p.versions.iter().map(|v| v.downloads).sum(),
        rating: p.rating,
        size: v.size,
        url: p.url.clone(),
        deprecated: p.deprecated,
        nsfw: p.nsfw,
        modpack: p.categories.iter().any(|c| c.eq_ignore_ascii_case("modpacks")),
        categories: p.categories.clone(),
        updated: p.updated.clone(),
        needs: v.dependencies.len(),
    })
}

/// Packages matching every word of `q` (name, owner, description), most popular first.
/// `sort`: "popular" (downloads), "rating", "new" (last updated).
pub fn search(list: &[Package], q: &str, sort: &str, adult: bool, offset: usize, limit: usize) -> (Vec<Found>, usize) {
    let words: Vec<String> = q.to_lowercase().split_whitespace().map(String::from).collect();
    let mut hits: Vec<Found> = list
        .iter()
        .filter(|p| adult || !p.nsfw)
        .filter(|p| {
            let hay = format!("{} {} {}", p.name.replace('_', " "), p.owner, p.versions.first().map(|v| v.description.as_str()).unwrap_or("")).to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .filter_map(found)
        .collect();
    let named = |f: &Found| words.first().is_some_and(|w| f.name.to_lowercase().replace('_', " ").contains(w.as_str()));
    match sort {
        "rating" => hits.sort_by(|a, b| b.rating.cmp(&a.rating)),
        "new" => hits.sort_by(|a, b| b.updated.cmp(&a.updated)),
        _ => hits.sort_by(|a, b| named(b).cmp(&named(a)).then(a.deprecated.cmp(&b.deprecated)).then(b.downloads.cmp(&a.downloads))),
    }
    let total = hits.len();
    (hits.into_iter().skip(offset).take(limit).collect(), total)
}

/// "Owner-Name-1.2.3" → ("Owner-Name", "1.2.3").
pub fn split_dep(dep: &str) -> Option<(&str, &str)> {
    let (full, ver) = dep.rsplit_once('-')?;
    ver.chars().next().filter(char::is_ascii_digit)?;
    Some((full, ver))
}

/// Compare two version numbers ("1.10.0" after "1.9.2").
pub fn ver_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    ver_key(a).cmp(&ver_key(b))
}

fn ver_key(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// A package and everything it needs, requirements first. Each package once, at the newest
/// version anything asks for. Unknown ones are listed apart.
pub fn resolve<'a>(list: &'a [Package], full: &str, version: Option<&str>) -> (Vec<(&'a Package, &'a Version)>, Vec<String>) {
    let by: HashMap<String, &Package> = list.iter().map(|p| (p.full_name.to_lowercase(), p)).collect();
    let mut want: BTreeMap<String, String> = BTreeMap::new(); // full lowercase → version
    let mut order: Vec<String> = vec![];
    let mut missing = vec![];
    fn visit(full: &str, ver: Option<&str>, by: &HashMap<String, &Package>, want: &mut BTreeMap<String, String>, order: &mut Vec<String>, missing: &mut Vec<String>, depth: usize) {
        let key = full.to_lowercase();
        let Some(p) = by.get(&key) else {
            if !missing.iter().any(|m| m == full) {
                missing.push(full.to_string());
            }
            return;
        };
        let pick = ver.filter(|v| p.versions.iter().any(|x| x.number == *v)).map(String::from).or_else(|| p.versions.first().map(|v| v.number.clone())).unwrap_or_default();
        if let Some(have) = want.get(&key) {
            if ver_key(have) >= ver_key(&pick) {
                return;
            }
        }
        want.insert(key.clone(), pick.clone());
        if depth > 30 {
            return;
        }
        if let Some(v) = p.versions.iter().find(|x| x.number == pick) {
            for d in &v.dependencies {
                if let Some((f, dv)) = split_dep(d) {
                    visit(f, Some(dv), by, want, order, missing, depth + 1);
                }
            }
        }
        order.retain(|o| o != &key);
        order.push(key);
    }
    visit(full, version, &by, &mut want, &mut order, &mut missing, 0);
    let out: Vec<(&Package, &Version)> = order
        .iter()
        .filter_map(|k| {
            let p: &Package = by.get(k)?;
            let want_v = want.get(k)?;
            let v = p.versions.iter().find(|x| &x.number == want_v)?;
            Some((p, v))
        })
        .collect();
    (out, missing)
}

/// A newer version of a Thunderstore mod in the library.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TsUpdate {
    pub full_name: String,
    pub name: String,
    pub have: String,
    pub newest: String,
    pub deprecated: bool,
}

/// Which of `stored` ("Owner-Name", version you have) have a newer version.
pub fn updates(list: &[Package], stored: &[(String, String)]) -> Vec<TsUpdate> {
    let by: HashMap<String, &Package> = list.iter().map(|p| (p.full_name.to_lowercase(), p)).collect();
    let mut out: Vec<TsUpdate> = stored
        .iter()
        .filter_map(|(full, have)| {
            let p = by.get(&full.to_lowercase())?;
            let newest = &p.versions.first()?.number;
            (ver_key(newest) > ver_key(have)).then(|| TsUpdate { full_name: p.full_name.clone(), name: p.name.replace('_', " "), have: have.clone(), newest: newest.clone(), deprecated: p.deprecated })
        })
        .collect();
    out.sort_by_key(|a| a.name.to_lowercase());
    out.dedup_by(|a, b| a.full_name == b.full_name);
    out
}

/// A mod pack's newer version: its (mod, file) list now, and the mods the old one had that the
/// new one doesn't.
pub fn pack_change(list: &[Package], full: &str, old: &str) -> Option<(String, Vec<(u64, u64)>, Vec<u64>)> {
    let p = list.iter().find(|p| p.full_name.eq_ignore_ascii_case(full))?;
    let newest = p.versions.first()?.number.clone();
    let ids = |chain: Vec<(&Package, &Version)>| -> Vec<(u64, u64)> { chain.into_iter().filter(|(x, _)| !x.full_name.eq_ignore_ascii_case(full)).map(|(x, v)| (mod_id(&x.full_name), file_id(&format!("{}-{}", x.full_name, v.number)))).collect() };
    let now = ids(resolve(list, full, Some(&newest)).0);
    let before = ids(resolve(list, full, Some(old)).0);
    let gone = before.iter().map(|(m, _)| *m).filter(|m| !now.iter().any(|(n, _)| n == m)).collect();
    Some((newest, now, gone))
}

/// The mods in an r2modman profile (export.r2x or mods.yml): ("Owner-Name", version, on).
pub fn r2_mods(yaml: &str) -> Vec<(String, Option<String>, bool)> {
    let mut out: Vec<(String, Option<String>, bool)> = vec![];
    let mut cur: Option<(String, [Option<u64>; 3], bool)> = None;
    let mut in_version = false;
    let mut item_indent = 0;
    let flush = |cur: &mut Option<(String, [Option<u64>; 3], bool)>, out: &mut Vec<(String, Option<String>, bool)>| {
        if let Some((name, v, on)) = cur.take() {
            if !name.is_empty() {
                let ver = match v {
                    [Some(a), Some(b), Some(c)] => Some(format!("{a}.{b}.{c}")),
                    _ => None,
                };
                out.push((name, ver, on));
            }
        }
    };
    for raw in yaml.lines() {
        let indent = raw.len() - raw.trim_start().len();
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("- ") {
            flush(&mut cur, &mut out);
            cur = Some((String::new(), [None; 3], true));
            item_indent = indent + 2;
            in_version = false;
            if let Some((k, v)) = rest.split_once(':') {
                if k.trim() == "name" {
                    cur.as_mut().unwrap().0 = v.trim().trim_matches(['"', '\'']).to_string();
                }
            }
            continue;
        }
        let Some(c) = cur.as_mut() else { continue };
        let Some((k, v)) = line.split_once(':') else { continue };
        let (k, v) = (k.trim(), v.trim().trim_matches(['"', '\'']));
        if indent <= item_indent {
            in_version = matches!(k, "version" | "versionNumber");
            match k {
                "name" => c.0 = v.to_string(),
                "enabled" => c.2 = v != "false",
                _ => {}
            }
        } else if in_version {
            let n = v.parse().ok();
            match k {
                "major" => c.1[0] = n,
                "minor" => c.1[1] = n,
                "patch" => c.1[2] = n,
                _ => {}
            }
        }
    }
    flush(&mut cur, &mut out);
    out.retain(|(n, _, _)| n.contains('-'));
    out
}

/// r2modman's profiles for a game on this computer: (name, folder).
pub fn r2_profiles(g: &TsGame) -> Vec<(String, PathBuf)> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return vec![] };
    if g.r2_folder.is_empty() {
        return vec![];
    }
    let mut out = vec![];
    for base in [home.join(".config/r2modmanPlus-local"), home.join(".var/app/com.github.ebkr.r2modman/config/r2modmanPlus-local")] {
        for e in fs::read_dir(base.join(&g.r2_folder).join("profiles")).into_iter().flatten().flatten() {
            if e.path().join("mods.yml").is_file() {
                out.push((e.file_name().to_string_lossy().into_owned(), e.path()));
            }
        }
    }
    out.sort();
    out
}

/// An r2modman profile code's contents: its mod list (export.r2x) and config files
/// (relative path → bytes), from Thunderstore's profile store.
pub async fn r2_code(http: &reqwest::Client, code: &str) -> Result<(String, Vec<(String, Vec<u8>)>), String> {
    use base64::Engine;
    let code = code.trim();
    if code.is_empty() || code.len() > 64 || !code.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err("That isn't an r2modman profile code (it looks like 0193a1b2-…).".into());
    }
    let body = fetch(http, &format!("https://thunderstore.io/api/experimental/legacyprofile/get/{code}/")).await.map_err(|_| "Thunderstore doesn't know that code (they expire after a while). Ask for a new one.".to_string())?;
    let text = String::from_utf8_lossy(&body);
    let b64: String = text.trim().strip_prefix("#r2modman").unwrap_or(&text).split_whitespace().collect();
    let zip_bytes = base64::engine::general_purpose::STANDARD.decode(b64.as_bytes()).map_err(|_| "That profile code's contents didn't read.".to_string())?;
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes)).map_err(|e| format!("That profile code's contents didn't read: {e}"))?;
    let mut list = String::new();
    let mut files = vec![];
    let mut total = 0u64;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = f.enclosed_name() else { continue };
        if f.is_dir() {
            continue;
        }
        let rel = rel.to_string_lossy().replace('\\', "/");
        let mut buf = vec![];
        (&mut f).take(4 * 1024 * 1024).read_to_end(&mut buf).map_err(|e| e.to_string())?;
        total += buf.len() as u64;
        if total > 64 * 1024 * 1024 {
            return Err("That profile code holds far more than settings.".into());
        }
        if rel.eq_ignore_ascii_case("export.r2x") {
            list = String::from_utf8_lossy(&buf).into_owned();
        } else if crate::modsettings::is_setting(&rel) {
            files.push((rel, buf));
        }
    }
    if list.is_empty() {
        return Err("That profile code has no mod list in it.".into());
    }
    Ok((list, files))
}

/// Where a package's file goes in the game folder, by the game's rules (r2modman's way), or
/// None for files that stay out (the package's own manifest, icon, readme).
pub fn place(g: &TsGame, full: &str, rel: &str) -> Option<String> {
    let rel = rel.replace('\\', "/");
    let low = rel.to_lowercase();
    if matches!(low.as_str(), "manifest.json" | "icon.png" | "readme.md" | "changelog.md" | "license" | "license.md" | "license.txt") {
        return None;
    }
    // Matching by path parts, ignoring capitals (never cutting strings by byte position, which
    // non-English file names could break).
    let parts: Vec<&str> = rel.split('/').filter(|p| !p.is_empty()).collect();
    let after = |prefix: &str| -> Option<String> {
        let want: Vec<&str> = prefix.split('/').filter(|p| !p.is_empty()).collect();
        (parts.len() > want.len() && want.iter().zip(&parts).all(|(a, b)| a.eq_ignore_ascii_case(b))).then(|| parts[want.len()..].join("/"))
    };
    // The loader itself: its root folder goes in the game folder.
    if let Some(root) = g.loaders.get(&full.to_lowercase()) {
        return after(root);
    }
    let under = |rule: &Rule, rest: &str| {
        let base = if rule.subdir { format!("{}/{full}", rule.route) } else { rule.route.clone() };
        let base = base.trim_matches('/');
        if base.is_empty() { rest.to_string() } else { format!("{base}/{rest}") }
    };
    // "BepInEx/plugins/…" written out in full.
    for rule in g.rules.iter().filter(|r| !r.route.is_empty()) {
        if let Some(rest) = after(&rule.route) {
            return Some(under(rule, &rest));
        }
    }
    // "plugins/…", "config/…": the last part of a route.
    if let Some((first, rest)) = rel.split_once('/') {
        if let Some(rule) = g.rules.iter().find(|r| !r.route.is_empty() && r.route.rsplit('/').next().is_some_and(|last| last.eq_ignore_ascii_case(first))) {
            return Some(under(rule, rest));
        }
    }
    // By file type (".mm.dll" before ".dll"), else where mods go by default.
    let mut by_ext: Vec<(&Rule, &String)> = g.rules.iter().flat_map(|r| r.exts.iter().map(move |e| (r, e))).collect();
    by_ext.sort_by_key(|(_, e)| std::cmp::Reverse(e.len()));
    let rule = by_ext.iter().find(|(_, e)| low.ends_with(e.as_str())).map(|(r, _)| *r).or_else(|| g.rules.iter().find(|r| r.default))?;
    Some(under(rule, &rel))
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let Ok(rd) = fs::read_dir(dir.join(&rel)) else { continue };
        for e in rd.flatten() {
            let Ok(t) = e.file_type() else { continue };
            let r = rel.join(e.file_name());
            if t.is_dir() {
                stack.push(r);
            } else if t.is_file() {
                out.push(r);
            }
        }
    }
    out
}

/// Lay an unpacked package out the way it goes in the game folder (moving its files).
pub fn lay_out(g: &TsGame, full: &str, unpacked: &Path, built: &Path) -> Result<usize, String> {
    let mut n = 0;
    for rel in walk(unpacked) {
        let r = rel.to_string_lossy().replace('\\', "/");
        let Some(dest) = place(g, full, &r) else { continue };
        let to = built.join(&dest);
        if !to.starts_with(built) || Path::new(&dest).components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
            continue;
        }
        fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::rename(unpacked.join(&rel), &to).or_else(|_| fs::copy(unpacked.join(&rel), &to).map(|_| ())).map_err(|e| e.to_string())?;
        n += 1;
    }
    fs::write(built.join(crate::fomod::MARK), "").map_err(|e| e.to_string())?;
    fs::write(built.join(crate::deploy::GAME_ROOT_MARK), "").map_err(|e| e.to_string())?;
    Ok(n)
}

/// Download one package version into the library, laid out for the game. Already there: nothing.
pub async fn fetch_into(http: &reqwest::Client, lib: &Path, game: &str, g: &TsGame, p: &Package, v: &Version, keep_archive: bool, now: u64) -> Result<bool, String> {
    let (mid, fid) = (mod_id(&p.full_name), file_id(&format!("{}-{}", p.full_name, v.number)));
    if library::list(lib).iter().any(|m| m.game == game && m.mod_id == mid && m.files.iter().any(|f| f.file_id == fid)) {
        return Ok(false);
    }
    if !v.download_url.starts_with("https://thunderstore.io/") {
        return Err(format!("{} has an unexpected download address.", p.full_name));
    }
    let dir = library::mod_dir(lib, game, mid, &p.name.replace('_', " "))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    library::ensure_space(&dir, v.size.saturating_mul(3), &format!("downloading {}", p.full_name))?;
    let file_name = format!("{}-{}.zip", p.full_name, v.number);
    let archive = dir.join(library::safe_name(&file_name));
    let mut r = http.get(&v.download_url).send().await.map_err(|e| format!("Couldn't download {}: {e}", p.full_name))?;
    if !r.status().is_success() {
        return Err(format!("Thunderstore answered {} for {}.", r.status(), p.full_name));
    }
    {
        use tokio::io::AsyncWriteExt;
        let mut f = tokio::fs::File::create(&archive).await.map_err(|e| e.to_string())?;
        let mut got: u64 = 0;
        while let Some(chunk) = r.chunk().await.map_err(|e| format!("Downloading {} stopped: {e}", p.full_name))? {
            got += chunk.len() as u64;
            if v.size > 0 && got > v.size + 1024 * 1024 {
                let _ = tokio::fs::remove_file(&archive).await;
                return Err(format!("{} is bigger than Thunderstore says.", p.full_name));
            }
            f.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        f.flush().await.map_err(|e| e.to_string())?;
    }
    let (g2, full, archive2, dir2) = (g.clone(), p.full_name.clone(), archive.clone(), dir.clone());
    let built_name = library::safe_name(&format!("{}-{} (laid out)", p.full_name, v.number));
    let bn = built_name.clone();
    tokio::task::spawn_blocking(move || -> Result<(), String> {
        let raw = library::unpack(&archive2)?;
        let built = dir2.join(&bn);
        let _ = fs::remove_dir_all(&built);
        fs::create_dir_all(&built).map_err(|e| e.to_string())?;
        lay_out(&g2, &full, &dir2.join(&raw), &built)?;
        let _ = fs::remove_dir_all(dir2.join(&raw));
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())??;
    if !keep_archive {
        let _ = fs::remove_file(&archive);
    }
    let mut m = library::read_meta(&dir).unwrap_or(StoredMod {
        game: game.to_string(),
        mod_id: mid,
        name: p.name.replace('_', " "),
        author: Some(p.owner.clone()),
        version: None,
        summary: None,
        picture_url: None,
        files: vec![],
        local: true,
        adult: p.nsfw,
        folder: String::new(),
        source: None,
    });
    m.version = Some(v.number.clone());
    // An older version's words and picture aren't kept: the newest one's stand in.
    let newest = p.versions.first().unwrap_or(v);
    m.summary = Some(if v.description.is_empty() { newest.description.clone() } else { v.description.clone() });
    m.picture_url = Some(if v.icon.is_empty() { newest.icon.clone() } else { v.icon.clone() });
    m.source = Some(format!("thunderstore:{}", p.full_name));
    m.files.retain(|f| f.file_id != fid);
    m.files.push(StoredFile {
        file_id: fid,
        name: format!("{} {}", p.name.replace('_', " "), v.number),
        file_name,
        version: Some(v.number.clone()),
        category: Some("main".into()),
        size: v.size,
        downloaded_at: now,
        archive: keep_archive.then(|| library::safe_name(&format!("{}-{}.zip", p.full_name, v.number))),
        unpacked: None,
        uploaded: None,
        verified: None,
        installed: Some(crate::fomod::Installed { folder: built_name, choices: vec![] }),
        has_installer: false,
    });
    library::write_meta(&dir, &m)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bep() -> TsGame {
        let rule = |route: &str, exts: &[&str], subdir: bool, default: bool| Rule { route: route.into(), exts: exts.iter().map(|s| s.to_string()).collect(), subdir, default };
        TsGame {
            community: "valheim".into(),
            name: "Valheim".into(),
            loader: "bepinex".into(),
            supported: true,
            r2_folder: "Valheim".into(),
            rules: vec![rule("BepInEx/plugins", &[".dll"], true, true), rule("BepInEx/core", &[], true, false), rule("BepInEx/patchers", &[], true, false), rule("BepInEx/monomod", &[".mm.dll"], true, false), rule("BepInEx/config", &[], false, false)],
            loaders: HashMap::from([("denikson-bepinexpack_valheim".into(), "BepInExPack_Valheim".into())]),
        }
    }

    #[test]
    fn places_files_like_r2modman() {
        let g = bep();
        let p = |rel: &str| place(&g, "Owner-Cool", rel);
        assert_eq!(p("Cool.dll").as_deref(), Some("BepInEx/plugins/Owner-Cool/Cool.dll"));
        assert_eq!(p("plugins/Sub/Cool.dll").as_deref(), Some("BepInEx/plugins/Owner-Cool/Sub/Cool.dll"));
        assert_eq!(p("config/cool.cfg").as_deref(), Some("BepInEx/config/cool.cfg"));
        assert_eq!(p("BepInEx/patchers/x.dll").as_deref(), Some("BepInEx/patchers/Owner-Cool/x.dll"));
        assert_eq!(p("Cool.mm.dll").as_deref(), Some("BepInEx/monomod/Owner-Cool/Cool.mm.dll"));
        assert_eq!(p("assets/thing.bundle").as_deref(), Some("BepInEx/plugins/Owner-Cool/assets/thing.bundle"));
        assert_eq!(p("manifest.json"), None);
        // The loader: its folder goes in the game folder.
        assert_eq!(place(&g, "denikson-BepInExPack_Valheim", "BepInExPack_Valheim/winhttp.dll").as_deref(), Some("winhttp.dll"));
        assert_eq!(place(&g, "denikson-BepInExPack_Valheim", "README.md"), None);
        // Names with letters whose lower case is a different length never break it.
        assert_eq!(p("\u{212A}onfig/x.cfg").as_deref(), Some("BepInEx/plugins/Owner-Cool/\u{212A}onfig/x.cfg"));
        assert_eq!(p("BepInEx/plugins/İ.dll").as_deref(), Some("BepInEx/plugins/Owner-Cool/İ.dll"));
    }

    fn pkg(full: &str, vers: &[(&str, &[&str])]) -> Package {
        let (owner, name) = full.split_once('-').unwrap();
        Package { full_name: full.into(), owner: owner.into(), name: name.into(), url: String::new(), rating: 0, deprecated: false, nsfw: false, categories: vec![], updated: String::new(), versions: vers.iter().map(|(n, d)| Version { number: n.to_string(), description: format!("{name} does things"), icon: String::new(), dependencies: d.iter().map(|s| s.to_string()).collect(), download_url: String::new(), downloads: 5, size: 1 }).collect() }
    }

    #[test]
    fn brings_requirements_first_once_each() {
        let list = vec![
            pkg("A-Mod", &[("2.0.0", &["B-Lib-1.0.0", "C-Core-1.0.0"])]),
            pkg("B-Lib", &[("1.1.0", &["C-Core-1.2.0"]), ("1.0.0", &["C-Core-1.0.0"])]),
            pkg("C-Core", &[("1.2.0", &[]), ("1.0.0", &[])]),
        ];
        let (got, missing) = resolve(&list, "A-Mod", None);
        let names: Vec<(String, String)> = got.iter().map(|(p, v)| (p.full_name.clone(), v.number.clone())).collect();
        assert_eq!(names, vec![("C-Core".into(), "1.0.0".into()), ("B-Lib".into(), "1.0.0".into()), ("A-Mod".into(), "2.0.0".into())]);
        assert!(missing.is_empty());
        let (_, missing) = resolve(&[pkg("A-Mod", &[("1.0.0", &["Z-Gone-1.0.0"])])], "A-Mod", None);
        assert_eq!(missing, vec!["Z-Gone"]);
        assert_eq!(split_dep("Owner-Some-Name-1.2.3"), Some(("Owner-Some-Name", "1.2.3")));
        let (hits, n) = search(&list, "lib", "popular", false, 0, 10);
        assert_eq!((n, hits[0].full_name.as_str()), (1, "B-Lib"));
        assert!(mod_id("A-Mod") >= IDS && library::is_local(mod_id("A-Mod")));
    }

    #[test]
    fn reads_r2modman_profiles() {
        let r2x = "profileName: Chill\nmods:\n- name: denikson-BepInExPack_Valheim\n  version:\n    major: 5\n    minor: 4\n    patch: 2333\n  enabled: true\n- name: ValheimModding-Jotunn\n  version:\n    major: 2\n    minor: 30\n    patch: 2\n  enabled: false\n";
        assert_eq!(r2_mods(r2x), vec![("denikson-BepInExPack_Valheim".into(), Some("5.4.2333".into()), true), ("ValheimModding-Jotunn".into(), Some("2.30.2".into()), false)]);
        let yml = "- manifestVersion: 2\n  name: ValheimModding-Jotunn\n  authorName: ValheimModding\n  versionNumber:\n    major: 2\n    minor: 30\n    patch: 2\n  enabled: true\n  dependencies:\n    - denikson-BepInExPack_Valheim-5.4.2333\n";
        assert_eq!(r2_mods(yml), vec![("ValheimModding-Jotunn".into(), Some("2.30.2".into()), true)]);
    }

    #[test]
    fn finds_updates_and_pack_changes() {
        let list = vec![
            pkg("A-Mod", &[("2.0.0", &[]), ("1.0.0", &[])]),
            pkg("P-Pack", &[("2.0.0", &["A-Mod-2.0.0", "C-New-1.0.0"]), ("1.0.0", &["A-Mod-1.0.0", "B-Old-1.0.0"])]),
            pkg("B-Old", &[("1.0.0", &[])]),
            pkg("C-New", &[("1.0.0", &[])]),
        ];
        let u = updates(&list, &[("A-Mod".into(), "1.0.0".into()), ("B-Old".into(), "1.0.0".into())]);
        assert_eq!(u.iter().map(|x| (x.full_name.as_str(), x.newest.as_str())).collect::<Vec<_>>(), vec![("A-Mod", "2.0.0")]);
        let (v, now, gone) = pack_change(&list, "P-Pack", "1.0.0").unwrap();
        assert_eq!(v, "2.0.0");
        assert!(now.contains(&(mod_id("A-Mod"), file_id("A-Mod-2.0.0"))) && now.contains(&(mod_id("C-New"), file_id("C-New-1.0.0"))));
        assert_eq!(gone, vec![mod_id("B-Old")]);
    }

    #[test]
    fn other_loaders_place_files_too() {
        let schema: Value = serde_json::json!({
            "games": {
                "mel": { "meta": { "displayName": "Mel" }, "distributions": [], "r2modman": [{ "packageIndex": "https://thunderstore.io/c/mel/api/v1/package-listing-index/", "packageLoader": "melonloader",
                    "installRules": [
                        { "route": "Mods", "defaultFileExtensions": [".dll"], "trackingMethod": "state", "isDefaultLocation": false, "subRoutes": [] },
                        { "route": "UserData/ModManager", "defaultFileExtensions": [], "trackingMethod": "subdir-no-flatten", "isDefaultLocation": true, "subRoutes": [] },
                        { "route": "UserLibs", "defaultFileExtensions": [".lib.dll"], "trackingMethod": "state", "isDefaultLocation": false, "subRoutes": [] },
                        { "route": "MelonLoader", "defaultFileExtensions": [], "trackingMethod": "state", "isDefaultLocation": false, "subRoutes": [{ "route": "Managed", "defaultFileExtensions": [".managed.dll"], "trackingMethod": "state", "isDefaultLocation": false, "subRoutes": [] }] }
                    ] }] },
                "rec": { "meta": { "displayName": "Rec" }, "distributions": [], "r2modman": [{ "packageIndex": "https://thunderstore.io/c/rec/api/v1/package-listing-index/", "packageLoader": "recursive-melonloader", "installRules": [] }] },
                "god": { "meta": { "displayName": "God" }, "distributions": [], "r2modman": [{ "packageIndex": "https://thunderstore.io/c/god/api/v1/package-listing-index/", "packageLoader": "godotml", "installRules": [{ "route": "mods", "trackingMethod": "package-zip", "isDefaultLocation": true }] }] }
            },
            "modloaderPackages": [{ "packageId": "LavaGang-MelonLoader", "rootFolder": "", "loader": "melonloader" }]
        });
        let mel = game_in(&schema, None, "mel", "").unwrap();
        assert!(mel.supported);
        assert_eq!(place(&mel, "A-Gun", "Gun.dll").as_deref(), Some("Mods/Gun.dll"));
        assert_eq!(place(&mel, "A-Gun", "Helper.lib.dll").as_deref(), Some("UserLibs/Helper.lib.dll"));
        assert_eq!(place(&mel, "A-Gun", "x.managed.dll").as_deref(), Some("MelonLoader/Managed/x.managed.dll"));
        assert_eq!(place(&mel, "A-Gun", "notes/thing.json").as_deref(), Some("UserData/ModManager/A-Gun/notes/thing.json"));
        assert_eq!(place(&mel, "LavaGang-MelonLoader", "MelonLoader/net6/x.dll").as_deref(), Some("MelonLoader/net6/x.dll"));
        assert_eq!(place(&mel, "LavaGang-MelonLoader", "version.dll").as_deref(), Some("version.dll"));
        let rec = game_in(&schema, None, "rec", "").unwrap();
        assert!(rec.supported);
        assert_eq!(place(&rec, "A-B", "Mods/B.dll").as_deref(), Some("Mods/B.dll"));
        assert!(!game_in(&schema, None, "god", "").unwrap().supported, "zips that go in whole aren't yet");
    }

    #[test]
    fn finds_games_in_the_ecosystem_file() {
        let schema: Value = serde_json::json!({
            "games": { "valheim": { "meta": { "displayName": "Valheim" }, "distributions": [{ "platform": "steam", "identifier": "892970" }],
                "r2modman": [{ "packageIndex": "https://thunderstore.io/c/valheim/api/v1/package-listing-index/", "packageLoader": "bepinex",
                    "installRules": [{ "route": "BepInEx/plugins", "defaultFileExtensions": [".dll"], "trackingMethod": "subdir", "isDefaultLocation": true }] }] } },
            "modloaderPackages": [{ "packageId": "denikson-BepInExPack_Valheim", "rootFolder": "BepInExPack_Valheim", "loader": "bepinex" }]
        });
        let g = game_in(&schema, Some(892970), "x", "y").unwrap();
        assert_eq!((g.community.as_str(), g.supported, g.rules.len()), ("valheim", true, 1));
        assert!(game_in(&schema, None, "valheim", "").is_some(), "by name too");
        assert!(game_in(&schema, Some(1), "nope", "Nope").is_none());
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// Talks to thunderstore.io: `cargo test live_thunderstore -- --ignored`.
    #[tokio::test]
    #[ignore]
    async fn live_thunderstore() {
        let http = reqwest::Client::builder().user_agent(crate::nexus::user_agent()).build().unwrap();
        let cache = tempfile::tempdir().unwrap();
        let g = game(&http, cache.path(), Some(892970), "valheim", "Valheim").await.unwrap().unwrap();
        println!("{} {} {} rules {} loaders {:?}", g.name, g.community, g.loader, g.rules.len(), g.loaders);
        let list = listing(&http, cache.path(), &g.community).await.unwrap();
        let (hits, total) = search(&list, "jotunn", "popular", false, 0, 3);
        println!("{} packages; jotunn hits {total}: {:?}", list.len(), hits.iter().map(|h| (&h.full_name, h.needs)).collect::<Vec<_>>());
        let (chain, missing) = resolve(&list, &hits[0].full_name, None);
        println!("chain {:?} missing {missing:?}", chain.iter().map(|(p, v)| format!("{}-{}", p.full_name, v.number)).collect::<Vec<_>>());
        let lib = tempfile::tempdir().unwrap();
        for (p, v) in &chain {
            fetch_into(&http, lib.path(), "valheim", &g, p, v, false, 1).await.unwrap();
        }
        for m in library::list(lib.path()) {
            let dir = crate::deploy::unpacked_dir(&m, &m.files[0]).unwrap();
            let e = crate::deploy::entries(&dir, "");
            println!("{}: {} files, e.g. {:?}", m.name, e.len(), e.iter().take(3).map(|x| &x.0).collect::<Vec<_>>());
        }
    }
}
