//! Links the Nexus Mods website hands to a mod manager, and mod page addresses people paste.
//!
//! The **Mod manager download** button on a file opens a link like
//! `nxm://skyrimspecialedition/mods/266/files/12345?key=abc&expires=1700000000&user_id=1`.
//! The key and expiry let a free account fetch that one file through the API. Premium accounts
//! can fetch any file without them.

use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NxmLink {
    pub game: String,
    pub mod_id: u64,
    pub file_id: u64,
    pub key: Option<String>,
    pub expires: Option<u64>,
}

/// A game's Nexus Mods name is lowercase letters, digits and a few separators.
fn valid_domain(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn parse_nxm(link: &str) -> Result<NxmLink, String> {
    let url = Url::parse(link.trim()).map_err(|_| "That isn't a Nexus Mods download link.".to_string())?;
    if url.scheme() != "nxm" {
        return Err("That isn't a Nexus Mods download link (it should start with nxm://).".into());
    }
    let game = url.host_str().unwrap_or("").to_ascii_lowercase();
    if !valid_domain(&game) {
        return Err("The link doesn't name a game.".into());
    }
    let parts: Vec<&str> = url.path().trim_matches('/').split('/').collect();
    let (mod_id, file_id) = match parts.as_slice() {
        ["mods", m, "files", f] => (m.parse::<u64>().ok(), f.parse::<u64>().ok()),
        _ => (None, None),
    };
    let (Some(mod_id), Some(file_id)) = (mod_id, file_id) else {
        return Err("The link doesn't name a mod and file. Collections links aren't supported yet.".into());
    };
    let mut key = None;
    let mut expires = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "key" if !v.is_empty() => key = Some(v.into_owned()),
            "expires" => expires = v.parse().ok(),
            _ => {}
        }
    }
    Ok(NxmLink { game, mod_id, file_id, key, expires })
}

/// A mod page address: `https://www.nexusmods.com/skyrimspecialedition/mods/266` (any tab,
/// with or without `www`). Gives the game and the mod's number.
pub fn parse_mod_page(link: &str) -> Result<(String, u64), String> {
    let url = Url::parse(link.trim()).map_err(|_| "Paste a mod page address from nexusmods.com.".to_string())?;
    let host = url.host_str().unwrap_or("");
    if !(host == "nexusmods.com" || host.ends_with(".nexusmods.com")) {
        return Err("Paste a mod page address from nexusmods.com.".into());
    }
    let parts: Vec<&str> = url.path().trim_matches('/').split('/').collect();
    // New-style addresses put the game under /games/: nexusmods.com/games/stardewvalley/mods/1.
    let parts = if parts.first() == Some(&"games") { &parts[1..] } else { &parts[..] };
    match parts {
        [game, "mods", id, ..] if valid_domain(game) => {
            let id = id.parse::<u64>().map_err(|_| "That address doesn't have a mod number.".to_string())?;
            Ok((game.to_ascii_lowercase(), id))
        }
        _ => Err("That address isn't a mod page. Open the mod on nexusmods.com and copy its address.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_free_account_link() {
        let l = parse_nxm("nxm://SkyrimSpecialEdition/mods/266/files/12345?key=abc_D-1&expires=1700000000&user_id=9").unwrap();
        assert_eq!(l.game, "skyrimspecialedition");
        assert_eq!((l.mod_id, l.file_id), (266, 12345));
        assert_eq!(l.key.as_deref(), Some("abc_D-1"));
        assert_eq!(l.expires, Some(1_700_000_000));
    }

    #[test]
    fn reads_a_premium_link_without_key() {
        let l = parse_nxm("nxm://stardewvalley/mods/2400/files/9").unwrap();
        assert_eq!(l.key, None);
        assert_eq!(l.expires, None);
    }

    #[test]
    fn refuses_other_links() {
        assert!(parse_nxm("https://example.com/mods/1/files/2").is_err());
        assert!(parse_nxm("nxm://skyrim/collections/abc/revisions/3").is_err());
        assert!(parse_nxm("nxm://skyrim/mods/x/files/2").is_err());
        assert!(parse_nxm("nxm://../mods/1/files/2").is_err());
    }

    #[test]
    fn reads_mod_pages() {
        assert_eq!(parse_mod_page("https://www.nexusmods.com/skyrimspecialedition/mods/266?tab=files").unwrap(), ("skyrimspecialedition".into(), 266));
        assert_eq!(parse_mod_page("https://nexusmods.com/games/stardewvalley/mods/1").unwrap(), ("stardewvalley".into(), 1));
        assert!(parse_mod_page("https://evil.com/skyrim/mods/1").is_err());
        assert!(parse_mod_page("https://www.nexusmods.com/skyrim/users/1").is_err());
    }
}
