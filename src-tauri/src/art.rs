//! Game artwork: a tall cover, a wide hero banner and the game's logo.
//!
//! Looked for in this order, and kept in the app's cache folder once found:
//! 1. Steam's own copy on this computer (`appcache/librarycache/<app id>/`, sometimes one
//!    folder deeper), so installed Steam games need nothing from the internet.
//! 2. Steam's image server, for Steam games whose art isn't on this computer yet.
//! 3. The game's tile on Nexus Mods (covers only), for every other game.
//!
//! The page gets each picture as a data: address, so nothing outside the app is loaded by it.

use crate::nexus::Nexus;
use base64::Engine;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Cover,
    Hero,
    Logo,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "cover" => Some(Kind::Cover),
            "hero" => Some(Kind::Hero),
            "logo" => Some(Kind::Logo),
            _ => None,
        }
    }
    /// Steam's file name for this picture on its image server.
    fn steam_file(self) -> &'static str {
        self.steam_files()[0]
    }
    /// Every name Steam has kept this picture under on this computer (newer Steam calls the
    /// tall cover library_capsule.jpg).
    fn steam_files(self) -> &'static [&'static str] {
        match self {
            Kind::Cover => &["library_600x900.jpg", "library_capsule.jpg"],
            Kind::Hero => &["library_hero.jpg"],
            Kind::Logo => &["logo.png"],
        }
    }
    fn name(self) -> &'static str {
        match self {
            Kind::Cover => "cover",
            Kind::Hero => "hero",
            Kind::Logo => "logo",
        }
    }
}

/// Steam's art for an app: in its folder, or one folder deeper (newer Steam keeps some there).
pub fn steam_local(home: &Path, app: u32, kind: Kind) -> Option<PathBuf> {
    for want in kind.steam_files() {
        for root in crate::games::steam_roots(home) {
            let dir = root.join("appcache/librarycache").join(app.to_string());
            let direct = dir.join(want);
            if direct.is_file() {
                return Some(direct);
            }
            if let Ok(rd) = fs::read_dir(&dir) {
                for e in rd.flatten() {
                    let p = e.path().join(want);
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
            // Older Steam: one flat folder, "<app>_library_600x900.jpg".
            let flat = root.join("appcache/librarycache").join(format!("{app}_{want}"));
            if flat.is_file() {
                return Some(flat);
            }
        }
    }
    None
}

fn mime(bytes: &[u8]) -> &'static str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        _ => "image/jpeg",
    }
}

pub fn data_url(bytes: &[u8]) -> String {
    format!("data:{};base64,{}", mime(bytes), base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// A picture is only kept when it really is one, and not something huge.
fn looks_like_image(b: &[u8]) -> bool {
    b.len() > 200 && b.len() < 8_000_000 && (b.starts_with(&[0xFF, 0xD8]) || b.starts_with(b"\x89PNG") || (b.len() > 12 && &b[8..12] == b"WEBP"))
}

async fn fetch(nexus: &Nexus, url: &str) -> Option<Vec<u8>> {
    let res = nexus.http().get(url).send().await.ok()?;
    if !res.status().is_success() {
        return None;
    }
    let b = res.bytes().await.ok()?.to_vec();
    looks_like_image(&b).then_some(b)
}

/// The picture's bytes, from the cache or found and cached now. `missing` remembers what
/// couldn't be found, so it isn't looked for again on every visit.
pub async fn find(cache: &Path, nexus: &Nexus, domain: &str, steam_app: Option<u32>, kind: Kind) -> Option<Vec<u8>> {
    let dir = cache.join("art");
    let file = dir.join(format!("{}-{}", crate::library::safe_name(domain), kind.name()));
    let missing = file.with_extension("missing");
    if let Ok(b) = fs::read(&file) {
        return Some(b);
    }
    if missing.exists() {
        // Try again after a week; new games get their art late.
        let fresh = fs::metadata(&missing).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|d| d.as_secs() < 7 * 86_400).unwrap_or(false);
        if fresh {
            return None;
        }
    }
    let mut found = None;
    if let Some(app) = steam_app {
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            found = steam_local(&home, app, kind).and_then(|p| fs::read(p).ok()).filter(|b| looks_like_image(b));
        }
        if found.is_none() {
            found = fetch(nexus, &format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app}/{}", kind.steam_file())).await;
        }
    }
    if found.is_none() && kind == Kind::Cover && nexus.has_key() {
        if let Ok(id) = nexus.game_id(domain).await {
            found = fetch(nexus, &format!("https://staticdelivery.nexusmods.com/Images/games/4_3/tile_{id}.jpg")).await;
        }
    }
    let _ = fs::create_dir_all(&dir);
    match &found {
        Some(b) => {
            let _ = fs::write(&file, b);
            let _ = fs::remove_file(&missing);
        }
        None => {
            let _ = fs::write(&missing, b"");
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_steam_art_in_either_layout() {
        let home = tempfile::tempdir().unwrap();
        let cache = home.path().join(".local/share/Steam/appcache/librarycache");
        fs::create_dir_all(home.path().join(".local/share/Steam/steamapps")).unwrap();
        fs::create_dir_all(cache.join("10")).unwrap();
        fs::write(cache.join("10/library_600x900.jpg"), b"x").unwrap();
        fs::create_dir_all(cache.join("20/abc123")).unwrap();
        fs::write(cache.join("20/abc123/logo.png"), b"x").unwrap();
        assert!(steam_local(home.path(), 10, Kind::Cover).is_some());
        assert!(steam_local(home.path(), 20, Kind::Logo).unwrap().ends_with("abc123/logo.png"));
        assert!(steam_local(home.path(), 20, Kind::Hero).is_none());
        assert!(steam_local(home.path(), 30, Kind::Cover).is_none());
        fs::create_dir_all(cache.join("40/def456")).unwrap();
        fs::write(cache.join("40/def456/library_capsule.jpg"), b"x").unwrap();
        assert!(steam_local(home.path(), 40, Kind::Cover).unwrap().ends_with("def456/library_capsule.jpg"));
    }

    #[test]
    fn data_urls_name_the_type() {
        assert!(data_url(b"\x89PNG....").starts_with("data:image/png;base64,"));
        assert!(data_url(&[0xFF, 0xD8, 1]).starts_with("data:image/jpeg;base64,"));
        assert!(!looks_like_image(b"<html>nope</html>"));
    }
}
