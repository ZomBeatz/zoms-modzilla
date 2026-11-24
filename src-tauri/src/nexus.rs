//! The Nexus Mods API (v1), with the person's own API key.
//!
//! The key comes from nexusmods.com > Settings > API keys. Nexus Mods limits how many calls a
//! key can make a day and an hour; every answer says how many are left, and the app shows it.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

const API: &str = "https://api.nexusmods.com/v1";
pub const APP_NAME: &str = "Zom's Nexus";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimit {
    pub daily_remaining: Option<u64>,
    pub hourly_remaining: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub user_id: u64,
    pub name: String,
    pub is_premium: bool,
    pub profile_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModInfo {
    pub mod_id: u64,
    pub game: String,
    pub name: String,
    pub summary: Option<String>,
    pub version: Option<String>,
    pub author: Option<String>,
    pub picture_url: Option<String>,
    pub adult: bool,
    #[serde(default)]
    pub endorsements: Option<u64>,
    #[serde(default)]
    pub downloads: Option<u64>,
    #[serde(default)]
    pub updated: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub file_id: u64,
    pub name: String,
    pub file_name: String,
    pub version: Option<String>,
    pub category: Option<String>,
    pub size: u64,
    pub uploaded: Option<u64>,
    pub is_primary: bool,
    pub description: Option<String>,
}

/// What the API returns, before it's tidied into the types above.
mod raw {
    use serde::Deserialize;

    #[derive(Deserialize)]
    pub struct User {
        pub user_id: u64,
        pub name: String,
        #[serde(default)]
        pub is_premium: bool,
        pub profile_url: Option<String>,
    }

    #[derive(Deserialize)]
    pub struct Mod {
        pub mod_id: u64,
        pub domain_name: Option<String>,
        pub name: Option<String>,
        pub summary: Option<String>,
        pub version: Option<String>,
        pub author: Option<String>,
        pub picture_url: Option<String>,
        #[serde(default)]
        pub contains_adult_content: bool,
        pub status: Option<String>,
        pub endorsement_count: Option<u64>,
        pub mod_downloads: Option<u64>,
        pub updated_timestamp: Option<u64>,
    }

    #[derive(Deserialize)]
    pub struct File {
        pub file_id: u64,
        pub name: Option<String>,
        pub file_name: String,
        pub version: Option<String>,
        pub category_name: Option<String>,
        pub size_in_bytes: Option<u64>,
        pub size_kb: Option<u64>,
        pub uploaded_timestamp: Option<u64>,
        #[serde(default)]
        pub is_primary: bool,
        pub description: Option<String>,
    }

    #[derive(Deserialize)]
    pub struct Files {
        pub files: Vec<File>,
        #[serde(default)]
        pub file_updates: Vec<FileUpdate>,
    }

    #[derive(Deserialize)]
    pub struct FileUpdate {
        pub old_file_id: u64,
        pub new_file_id: u64,
    }

    #[derive(Deserialize)]
    pub struct Game {
        pub id: u64,
    }

    #[derive(Deserialize)]
    pub struct Link {
        #[serde(rename = "URI")]
        pub uri: String,
    }

    #[derive(Deserialize)]
    pub struct Message {
        pub message: Option<String>,
    }
}

impl From<raw::File> for FileInfo {
    fn from(f: raw::File) -> Self {
        FileInfo {
            file_id: f.file_id,
            name: f.name.unwrap_or_else(|| f.file_name.clone()),
            size: f.size_in_bytes.or(f.size_kb.map(|k| k * 1024)).unwrap_or(0),
            file_name: f.file_name,
            version: f.version.filter(|v| !v.is_empty()),
            category: f.category_name,
            uploaded: f.uploaded_timestamp,
            is_primary: f.is_primary,
            description: f.description,
        }
    }
}

/// Why a call didn't work, in words for the person, plus whether it means "needs a fresh link".
#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// A game on Nexus Mods.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NexusGame {
    pub domain: String,
    pub name: String,
    pub mods: u64,
}

/// Told the new numbers whenever an answer from Nexus Mods changes them.
pub type LimitsListener = Arc<dyn Fn(&RateLimit) + Send + Sync>;

#[derive(Clone)]
pub struct Nexus {
    http: reqwest::Client,
    key: Arc<Mutex<Option<String>>>,
    pub limits: Arc<Mutex<RateLimit>>,
    listener: Arc<Mutex<Option<LimitsListener>>>,
    base: String,
}

pub fn user_agent() -> String {
    format!("ZomsNexus/{APP_VERSION} (Linux; +https://docs.zombeatzmedia.com/nexus/)")
}

impl Nexus {
    pub fn new(key: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(user_agent())
            .connect_timeout(std::time::Duration::from_secs(20))
            .build()
            .expect("HTTP client");
        Nexus { http, key: Arc::new(Mutex::new(key)), limits: Arc::default(), listener: Arc::default(), base: API.to_string() }
    }

    /// Talk to another server instead (the tests' stand-in for Nexus Mods).
    #[cfg(test)]
    pub fn with_base(mut self, base: &str) -> Self {
        self.base = base.to_string();
        // The stand-in closes every connection, so never reuse one.
        self.http = reqwest::Client::builder().user_agent(user_agent()).pool_max_idle_per_host(0).build().unwrap();
        self
    }

    /// Hear about each change to the requests left (every answer carries the numbers, so the
    /// count stays current without asking Nexus Mods for it).
    pub fn on_limits(&self, f: LimitsListener) {
        *self.listener.lock().unwrap() = Some(f);
    }

    /// Note the requests left from an answer's headers, and tell the listener if they changed.
    fn note_limits(&self, h: &reqwest::header::HeaderMap) {
        let num = |n: &str| h.get(n).and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok());
        let changed = {
            let mut l = self.limits.lock().unwrap();
            let before = (l.daily_remaining, l.hourly_remaining);
            if let Some(d) = num("x-rl-daily-remaining") {
                l.daily_remaining = Some(d);
            }
            if let Some(d) = num("x-rl-hourly-remaining") {
                l.hourly_remaining = Some(d);
            }
            ((l.daily_remaining, l.hourly_remaining) != before).then(|| l.clone())
        };
        if let Some(l) = changed {
            let f = self.listener.lock().unwrap().clone();
            if let Some(f) = f {
                f(&l);
            }
        }
    }

    /// The (mod, file) numbers of files on Nexus Mods with this MD5 checksum. Empty when Nexus
    /// Mods knows none (it answers "not found").
    pub async fn md5_search(&self, game: &str, md5: &str) -> Result<Vec<(u64, u64)>, ApiError> {
        match self.get::<serde_json::Value>(&format!("/games/{game}/mods/md5_search/{md5}.json"), &[]).await {
            Ok(v) => Ok(v
                .as_array()
                .map(|a| a.iter().filter_map(|x| Some((x["mod"]["mod_id"].as_u64()?, x["file_details"]["file_id"].as_u64()?))).collect())
                .unwrap_or_default()),
            Err(e) if e.status == 404 => Ok(vec![]),
            Err(e) => Err(e),
        }
    }

    /// Every game on Nexus Mods: (domain, name, number of mods). One request.
    pub async fn games(&self) -> Result<Vec<NexusGame>, ApiError> {
        let v: serde_json::Value = self.get("/games.json", &[]).await?;
        Ok(v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|g| {
                        Some(NexusGame {
                            domain: g["domain_name"].as_str()?.to_string(),
                            name: g["name"].as_str()?.to_string(),
                            mods: g["mods"].as_u64().unwrap_or(0),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    pub fn key(&self) -> Option<String> {
        self.key.lock().unwrap().clone()
    }

    pub fn set_key(&self, key: Option<String>) {
        *self.key.lock().unwrap() = key;
    }

    pub fn has_key(&self) -> bool {
        self.key.lock().unwrap().is_some()
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    async fn get<T: for<'de> Deserialize<'de>>(&self, path: &str, query: &[(&str, String)]) -> Result<T, ApiError> {
        self.call(reqwest::Method::GET, path, query, None).await
    }

    async fn call<T: for<'de> Deserialize<'de>>(&self, method: reqwest::Method, path: &str, query: &[(&str, String)], form: Option<&[(&str, String)]>) -> Result<T, ApiError> {
        let key = self.key.lock().unwrap().clone().ok_or(ApiError {
            status: 401,
            message: "Add your Nexus Mods API key in Settings first.".into(),
        })?;
        let mut req = self.http.request(method, format!("{}{path}", self.base)).query(query);
        if let Some(f) = form {
            req = req.form(f);
        }
        let res = req
            .header("apikey", key)
            .header("Application-Name", APP_NAME)
            .header("Application-Version", APP_VERSION)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| ApiError { status: 0, message: format!("Couldn't reach Nexus Mods. Check your internet connection, then try again. ({e})") })?;
        self.note_limits(res.headers());
        let status = res.status().as_u16();
        if res.status().is_success() {
            return res.json::<T>().await.map_err(|e| ApiError { status, message: format!("Nexus Mods sent an answer the app couldn't read. ({e})") });
        }
        let said = res.json::<raw::Message>().await.ok().and_then(|m| m.message).unwrap_or_default();
        crate::log::warn(&format!("Nexus Mods answered {status} for {path}{}", if said.is_empty() { String::new() } else { format!(": {said}") }));
        let message = match status {
            401 => "Nexus Mods didn't accept the API key. Copy it again from nexusmods.com > Settings > API keys.".to_string(),
            403 if said.starts_with("Mod not available") => "Nexus Mods doesn't show this mod any more: its author hid it, or it was removed. It can't be downloaded; its page may say what replaced it.".to_string(),
            403 => if said.is_empty() { "Nexus Mods said no to this download.".to_string() } else { said },
            404 => "Nexus Mods couldn't find that mod or file. It may have been removed or hidden.".to_string(),
            410 => "That download link has run out. Click Mod manager download on Nexus Mods again.".to_string(),
            429 => "You've used up your Nexus Mods requests for now. Downloads pick up again within the hour.".to_string(),
            _ if !said.is_empty() => said,
            _ => format!("Nexus Mods answered with an error ({status}). Try again in a minute."),
        };
        Err(ApiError { status, message })
    }

    /// A query to the newer GraphQL API (v2), which knows about collections.
    pub async fn graphql(&self, query: &str, variables: serde_json::Value) -> Result<serde_json::Value, ApiError> {
        let key = self.key.lock().unwrap().clone().ok_or(ApiError { status: 401, message: "Add your Nexus Mods API key in Settings first.".into() })?;
        let url = format!("{}/graphql", self.base.trim_end_matches("/v1").to_string() + "/v2");
        let res = self
            .http
            .post(url)
            .header("apikey", key)
            .header("Application-Name", APP_NAME)
            .header("Application-Version", APP_VERSION)
            .json(&serde_json::json!({ "query": query, "variables": variables }))
            .send()
            .await
            .map_err(|e| ApiError { status: 0, message: format!("Couldn't reach Nexus Mods. Check your internet connection, then try again. ({e})") })?;
        self.note_limits(res.headers());
        let status = res.status().as_u16();
        let body: serde_json::Value = res.json().await.map_err(|e| ApiError { status, message: format!("Nexus Mods sent an answer the app couldn't read. ({e})") })?;
        if let Some(err) = body.get("errors").and_then(|e| e.get(0)).and_then(|e| e.get("message")).and_then(|m| m.as_str()) {
            if body.get("data").map(|d| d.is_null()).unwrap_or(true) {
                return Err(ApiError { status: if status >= 400 { status } else { 400 }, message: format!("Nexus Mods said: {err}") });
            }
        }
        if status >= 400 {
            return Err(ApiError { status, message: format!("Nexus Mods answered with an error ({status}).") });
        }
        Ok(body.get("data").cloned().unwrap_or_default())
    }

    /// A collection revision's own archive (its collection.json: installer choices, notes), from
    /// the `downloadLink` the collection lookup gives. A relative link is on the API's host; an
    /// answer listing links is followed to the first. The key only goes to nexusmods.com hosts,
    /// and at most `max` bytes are read.
    pub async fn collection_archive(&self, link: &str, max: u64) -> Result<Vec<u8>, ApiError> {
        let fail = |m: String| ApiError { status: 0, message: m };
        let origin = url::Url::parse(&self.base).map(|u| u.origin().ascii_serialization()).unwrap_or_else(|_| "https://api.nexusmods.com".into());
        let mut url = if link.starts_with('/') { format!("{origin}{link}") } else { link.to_string() };
        for _ in 0..2 {
            let u = url::Url::parse(&url).map_err(|_| fail("The collection's download link isn't an address.".into()))?;
            let host = u.host_str().unwrap_or("").to_ascii_lowercase();
            let nexus = host == "nexusmods.com" || host.ends_with(".nexusmods.com") || (cfg!(test) && host == "127.0.0.1");
            if u.scheme() != "https" && !(cfg!(test) && u.scheme() == "http") {
                return Err(fail("The collection's download link isn't secure.".into()));
            }
            if !nexus && !host.ends_with(".nexus-cdn.com") && host != "nexus-cdn.com" {
                return Err(fail(format!("The collection's download link points somewhere unexpected ({host}).")));
            }
            let mut req = self.http.get(u).header("Application-Name", APP_NAME).header("Application-Version", APP_VERSION);
            if nexus {
                if let Some(k) = self.key.lock().unwrap().clone() {
                    req = req.header("apikey", k);
                }
            }
            let res = req.send().await.map_err(|e| fail(format!("Couldn't reach Nexus Mods ({e}).")))?;
            if nexus {
                self.note_limits(res.headers());
            }
            let status = res.status().as_u16();
            if status >= 400 {
                return Err(ApiError { status, message: format!("Nexus Mods didn't hand out the collection's file ({status}).") });
            }
            if res.content_length().is_some_and(|n| n > max) {
                return Err(fail("The collection's file is bigger than expected.".into()));
            }
            let json = res.headers().get("content-type").and_then(|v| v.to_str().ok()).is_some_and(|t| t.contains("json"));
            let mut body = Vec::new();
            let mut stream = res.bytes_stream();
            use futures_util::StreamExt;
            while let Some(chunk) = stream.next().await {
                body.extend_from_slice(&chunk.map_err(|e| fail(format!("The download stopped ({e}).")))?);
                if body.len() as u64 > max {
                    return Err(fail("The collection's file is bigger than expected.".into()));
                }
            }
            if !json {
                return Ok(body);
            }
            // {"download_links":[{"URI":...}]} or [{"URI":...}]: follow the first.
            let v: serde_json::Value = serde_json::from_slice(&body).map_err(|_| fail("Nexus Mods' answer about the collection's file couldn't be read.".into()))?;
            let list = v.get("download_links").unwrap_or(&v);
            url = list.get(0).and_then(|x| x.get("URI").or_else(|| x.get("uri"))).and_then(|x| x.as_str()).ok_or_else(|| fail("Nexus Mods gave no link for the collection's file.".into()))?.to_string();
        }
        Err(fail("Nexus Mods kept pointing elsewhere for the collection's file.".into()))
    }

    /// Endorse a mod (or take the endorsement back), for its version you have.
    pub async fn endorse(&self, game: &str, mod_id: u64, on: bool, version: &str) -> Result<(), ApiError> {
        let what = if on { "endorse" } else { "abstain" };
        let _: serde_json::Value = self.call(reqwest::Method::POST, &format!("/v1/games/{game}/mods/{mod_id}/{what}.json"), &[], Some(&[("version", version.to_string())])).await?;
        Ok(())
    }

    /// Track a mod on Nexus Mods (its updates show on the site), or stop tracking it.
    pub async fn track(&self, game: &str, mod_id: u64, on: bool) -> Result<(), ApiError> {
        let m = if on { reqwest::Method::POST } else { reqwest::Method::DELETE };
        let _: serde_json::Value = self.call(m, "/v1/user/tracked_mods.json", &[("domain_name", game.to_string())], Some(&[("mod_id", mod_id.to_string())])).await?;
        Ok(())
    }

    /// The mods you've endorsed and the ones you track: (game, mod) pairs. Two requests.
    pub async fn marks(&self) -> Result<(Vec<(String, u64)>, Vec<(String, u64)>), ApiError> {
        let e: Vec<serde_json::Value> = self.get("/v1/user/endorsements.json", &[]).await?;
        let t: Vec<serde_json::Value> = self.get("/v1/user/tracked_mods.json", &[]).await?;
        let pair = |v: &serde_json::Value, g: &str| Some((v.get(g)?.as_str()?.to_string(), v.get("mod_id")?.as_u64()?));
        Ok((
            e.iter().filter(|v| v.get("status").and_then(|s| s.as_str()) == Some("Endorsed")).filter_map(|v| pair(v, "domain_name")).collect(),
            t.iter().filter_map(|v| pair(v, "domain_name")).collect(),
        ))
    }

    pub async fn validate(&self) -> Result<User, ApiError> {
        let u: raw::User = self.get("/users/validate.json", &[]).await?;
        Ok(User { user_id: u.user_id, name: u.name, is_premium: u.is_premium, profile_url: u.profile_url })
    }

    /// A mod's changelog, newest version first (one request).
    pub async fn changelog(&self, game: &str, mod_id: u64) -> Result<Vec<Change>, ApiError> {
        let v: serde_json::Value = self.get(&format!("/games/{game}/mods/{mod_id}/changelogs.json"), &[]).await?;
        Ok(changes_from(&v))
    }

    pub async fn mod_info(&self, game: &str, mod_id: u64) -> Result<ModInfo, ApiError> {
        let m: raw::Mod = self.get(&format!("/games/{game}/mods/{mod_id}.json"), &[]).await?;
        Ok(tidy(m, game))
    }

    /// Nexus Mods' lists for a game: "trending", "latest_added" or "latest_updated".
    pub async fn browse(&self, game: &str, list: &str) -> Result<Vec<ModInfo>, ApiError> {
        if !matches!(list, "trending" | "latest_added" | "latest_updated") {
            return Err(ApiError { status: 400, message: "There's no list by that name.".into() });
        }
        let mods: Vec<raw::Mod> = self.get(&format!("/games/{game}/mods/{list}.json"), &[]).await?;
        Ok(mods.into_iter().filter(|m| m.name.is_some()).map(|m| tidy(m, game)).collect())
    }
}

fn tidy(m: raw::Mod, game: &str) -> ModInfo {
    {
        let mod_id = m.mod_id;
        let hidden = matches!(m.status.as_deref(), Some("hidden") | Some("removed") | Some("under_moderation"));
        ModInfo {
            mod_id: m.mod_id,
            game: m.domain_name.unwrap_or_else(|| game.to_string()),
            name: m.name.filter(|n| !n.is_empty()).unwrap_or_else(|| if hidden { format!("Mod {mod_id} (hidden)") } else { format!("Mod {mod_id}") }),
            summary: m.summary,
            version: m.version,
            author: m.author,
            picture_url: m.picture_url,
            adult: m.contains_adult_content,
            endorsements: m.endorsement_count,
            downloads: m.mod_downloads,
            updated: m.updated_timestamp,
        }
    }
}

impl Nexus {

    pub async fn files(&self, game: &str, mod_id: u64) -> Result<Vec<FileInfo>, ApiError> {
        Ok(self.files_and_updates(game, mod_id).await?.0)
    }

    /// The mod's files, and which file replaced which (old file, new file) when the author
    /// uploaded an update.
    pub async fn files_and_updates(&self, game: &str, mod_id: u64) -> Result<(Vec<FileInfo>, Vec<(u64, u64)>), ApiError> {
        let f: raw::Files = self.get(&format!("/games/{game}/mods/{mod_id}/files.json"), &[]).await?;
        let updates = f.file_updates.iter().map(|u| (u.old_file_id, u.new_file_id)).collect();
        let mut files: Vec<FileInfo> = f.files.into_iter().map(Into::into).collect();
        // Main files first, then by newest.
        let rank = |c: &Option<String>| match c.as_deref() {
            Some("MAIN") => 0,
            Some("UPDATE") => 1,
            Some("OPTIONAL") => 2,
            Some("MISCELLANEOUS") => 3,
            Some("OLD_VERSION") => 5,
            _ => 4,
        };
        files.sort_by(|a, b| rank(&a.category).cmp(&rank(&b.category)).then(b.uploaded.cmp(&a.uploaded)));
        Ok((files, updates))
    }

    /// The game's number on Nexus Mods (for its picture).
    pub async fn game_id(&self, game: &str) -> Result<u64, ApiError> {
        let g: raw::Game = self.get(&format!("/games/{game}.json"), &[]).await?;
        Ok(g.id)
    }

    pub async fn file(&self, game: &str, mod_id: u64, file_id: u64) -> Result<FileInfo, ApiError> {
        let f: raw::File = self.get(&format!("/games/{game}/mods/{mod_id}/files/{file_id}.json"), &[]).await?;
        Ok(f.into())
    }

    /// Where to download a file from. Free accounts need the key and expiry from an nxm link;
    /// Premium accounts don't. The first server Nexus Mods lists is its pick for you.
    pub async fn download_url(&self, game: &str, mod_id: u64, file_id: u64, key: Option<&str>, expires: Option<u64>) -> Result<String, ApiError> {
        let mut q = vec![];
        if let (Some(k), Some(e)) = (key, expires) {
            q.push(("key", k.to_string()));
            q.push(("expires", e.to_string()));
        }
        let links: Vec<raw::Link> = self.get(&format!("/games/{game}/mods/{mod_id}/files/{file_id}/download_link.json"), &q).await?;
        links.into_iter().next().map(|l| l.uri).ok_or(ApiError { status: 404, message: "Nexus Mods didn't offer a download server for this file.".into() })
    }
}

/// One version's notes in a mod's changelog.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub version: String,
    pub notes: Vec<String>,
}

/// Version numbers compared part by part ("1.10" after "1.9"); text parts as text.
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parts = |s: &str| s.trim_start_matches(['v', 'V']).split(['.', '-', ' ']).map(|p| p.parse::<u64>().map_err(|_| p.to_lowercase())).collect::<Vec<_>>();
    let (pa, pb) = (parts(a), parts(b));
    for (x, y) in pa.iter().zip(pb.iter()) {
        let o = match (x, y) {
            (Ok(x), Ok(y)) => x.cmp(y),
            (Ok(_), Err(_)) => std::cmp::Ordering::Greater,
            (Err(_), Ok(_)) => std::cmp::Ordering::Less,
            (Err(x), Err(y)) => x.cmp(y),
        };
        if o != std::cmp::Ordering::Equal {
            return o;
        }
    }
    pa.len().cmp(&pb.len())
}

/// Nexus Mods answers {"1.2": ["Fixed x", ...], ...} (or [] when there's none).
pub fn changes_from(v: &serde_json::Value) -> Vec<Change> {
    let mut out: Vec<Change> = v
        .as_object()
        .map(|o| {
            o.iter()
                .map(|(k, lines)| Change {
                    version: k.clone(),
                    notes: lines.as_array().map(|a| a.iter().filter_map(|l| l.as_str()).map(|l| l.trim().chars().take(600).collect::<String>()).filter(|l| !l.is_empty()).take(60).collect()).unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| version_cmp(&b.version, &a.version));
    out.truncate(40);
    out
}

#[cfg(test)]
mod changelog_tests {
    use super::*;

    #[test]
    fn sorts_versions_newest_first() {
        let v = serde_json::json!({ "1.9": ["a"], "1.10": ["b", " "], "v2.0": ["c"], "1.2": [] });
        let c = changes_from(&v);
        assert_eq!(c.iter().map(|x| x.version.as_str()).collect::<Vec<_>>(), vec!["v2.0", "1.10", "1.9", "1.2"]);
        assert_eq!(c[1].notes, vec!["b"]);
        assert!(changes_from(&serde_json::json!([])).is_empty());
    }
}
