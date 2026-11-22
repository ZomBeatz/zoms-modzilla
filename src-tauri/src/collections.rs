//! Collections: curated lists of mods on Nexus Mods. Adding one puts every Nexus Mods file in it
//! in the download queue and makes a profile with them.
//!
//! Collections are only in the newer GraphQL API (v2). Their files download like any other:
//! straight away for Premium accounts, and with a click on the website for free accounts (as
//! Nexus Mods asks).

use crate::nexus::{ApiError, Nexus};
use serde::Serialize;
use serde_json::{json, Value};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Address {
    pub game: String,
    pub slug: String,
    pub revision: Option<u64>,
}

/// `https://www.nexusmods.com/games/<game>/collections/<slug>[/revisions/<n>]`, the older
/// `nexusmods.com/<game>/collections/<slug>`, or `nxm://<game>/collections/<slug>/revisions/<n>`.
pub fn parse(link: &str) -> Result<Address, String> {
    let url = Url::parse(link.trim()).map_err(|_| "Paste a collection's address from nexusmods.com.".to_string())?;
    let mut parts: Vec<String> = url.path().trim_matches('/').split('/').map(String::from).collect();
    match url.scheme() {
        "nxm" => parts.insert(0, url.host_str().unwrap_or("").to_ascii_lowercase()),
        "https" | "http" => {
            let host = url.host_str().unwrap_or("");
            if !(host == "nexusmods.com" || host.ends_with(".nexusmods.com")) {
                return Err("Paste a collection's address from nexusmods.com.".into());
            }
            if parts.first().map(String::as_str) == Some("games") {
                parts.remove(0);
            }
        }
        _ => return Err("Paste a collection's address from nexusmods.com.".into()),
    }
    let ok = |s: &str| !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    match parts.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        [game, "collections", slug, rest @ ..] if ok(game) && ok(slug) => {
            let revision = match rest {
                ["revisions", n, ..] => Some(n.parse().map_err(|_| "That collection address has an odd revision number.".to_string())?),
                _ => None,
            };
            Ok(Address { game: game.to_ascii_lowercase(), slug: slug.to_string(), revision })
        }
        _ => Err("That address isn't a collection. Open the collection on nexusmods.com and copy its address.".into()),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionFile {
    pub game: String,
    pub mod_id: u64,
    pub file_id: u64,
    pub mod_name: String,
    pub name: String,
    pub version: Option<String>,
    pub size: u64,
    pub optional: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub game: String,
    pub slug: String,
    pub revision: Option<u64>,
    pub name: String,
    pub author: Option<String>,
    pub summary: Option<String>,
    pub picture_url: Option<String>,
    pub files: Vec<CollectionFile>,
}

const QUERY: &str = "query ZomsNexusCollection($slug: String!, $revision: Int, $adult: Boolean) {
  collectionRevision(slug: $slug, revision: $revision, viewAdultContent: $adult) {
    revisionNumber
    collection { name summary tileImage { url } user { name } game { domainName } }
    modFiles {
      fileId
      optional
      file { fileId name version sizeInBytes mod { modId name game { domainName } } }
    }
  }
}";

fn num(v: &Value) -> Option<u64> {
    v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

fn text(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(String::from)
}

/// Read the answer. Tolerant: a file missing its details is skipped, not fatal.
pub fn read(addr: &Address, data: &Value) -> Result<Collection, String> {
    let rev = data.get("collectionRevision").filter(|v| !v.is_null()).ok_or("Nexus Mods couldn't find that collection. It may be hidden, removed, or for adults only.")?;
    let c = &rev["collection"];
    let mut files = vec![];
    for mf in rev["modFiles"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        let f = &mf["file"];
        let (Some(file_id), Some(mod_id)) = (num(&f["fileId"]).or_else(|| num(&mf["fileId"])), num(&f["mod"]["modId"])) else { continue };
        let game = text(&f["mod"]["game"]["domainName"]).unwrap_or_else(|| addr.game.clone()).to_ascii_lowercase();
        files.push(CollectionFile {
            game,
            mod_id,
            file_id,
            mod_name: text(&f["mod"]["name"]).unwrap_or_else(|| format!("Mod {mod_id}")),
            name: text(&f["name"]).unwrap_or_else(|| format!("File {file_id}")),
            version: text(&f["version"]),
            size: num(&f["sizeInBytes"]).unwrap_or(0),
            optional: mf["optional"].as_bool().unwrap_or(false),
        });
    }
    Ok(Collection {
        game: text(&c["game"]["domainName"]).unwrap_or_else(|| addr.game.clone()),
        slug: addr.slug.clone(),
        revision: num(&rev["revisionNumber"]).or(addr.revision),
        name: text(&c["name"]).unwrap_or_else(|| addr.slug.clone()),
        author: text(&c["user"]["name"]),
        summary: text(&c["summary"]),
        picture_url: text(&c["tileImage"]["url"]),
        files,
    })
}

pub async fn fetch(nexus: &Nexus, addr: &Address) -> Result<Collection, ApiError> {
    let data = nexus.graphql(QUERY, json!({ "slug": addr.slug, "revision": addr.revision, "adult": true })).await?;
    read(addr, &data).map_err(|message| ApiError { status: 404, message })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_collection_addresses() {
        let a = parse("https://www.nexusmods.com/games/skyrimspecialedition/collections/qdurkx").unwrap();
        assert_eq!(a, Address { game: "skyrimspecialedition".into(), slug: "qdurkx".into(), revision: None });
        assert_eq!(parse("https://next.nexusmods.com/stardewvalley/collections/tckf0m/revisions/12").unwrap().revision, Some(12));
        assert_eq!(parse("nxm://SkyrimSpecialEdition/collections/abc123/revisions/5").unwrap(), Address { game: "skyrimspecialedition".into(), slug: "abc123".into(), revision: Some(5) });
        assert!(parse("https://www.nexusmods.com/skyrimspecialedition/mods/266").is_err());
        assert!(parse("https://evil.example/skyrim/collections/abc").is_err());
    }

    #[test]
    fn reads_an_answer_and_skips_broken_files() {
        let addr = parse("nxm://skyrimspecialedition/collections/abc123/revisions/5").unwrap();
        let data = json!({ "collectionRevision": {
            "revisionNumber": 5,
            "collection": { "name": "Snowy North", "summary": "Winter", "user": { "name": "Maya" }, "game": { "domainName": "skyrimspecialedition" }, "tileImage": null },
            "modFiles": [
                { "fileId": 70, "optional": false, "file": { "fileId": "70", "name": "Main", "version": "1.0", "sizeInBytes": "12345", "mod": { "modId": 7, "name": "Lanterns", "game": { "domainName": "skyrimspecialedition" } } } },
                { "fileId": 80, "optional": true, "file": { "fileId": 80, "name": "Opt", "mod": { "modId": 8, "name": "Opt mod" } } },
                { "fileId": 90, "optional": false, "file": null }
            ]
        }});
        let c = read(&addr, &data).unwrap();
        assert_eq!(c.name, "Snowy North");
        assert_eq!(c.files.len(), 2);
        assert_eq!((c.files[0].mod_id, c.files[0].file_id, c.files[0].size), (7, 70, 12345));
        assert!(c.files[1].optional);
        assert!(read(&addr, &json!({ "collectionRevision": null })).is_err());
    }
}
