//! Is there a newer Zom's Modzilla? Asks GitHub for the newest release of the app's repository, at
//! most once a day, and only when the setting is on (it is at first). Installed from Flathub,
//! Flathub does the updating, so the app doesn't ask. Nothing about you is sent: only the app's
//! own name and version, as every request does.

use serde::{Deserialize, Serialize};
use std::{fs, path::Path, time::Duration};

/// Where releases are published.
pub const REPO: &str = "ZomBeatz/zoms-modzilla";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    /// The release's page (notes and files).
    pub url: String,
    pub notes: String,
    /// The AppImage to download, when the release has one.
    pub app_image: Option<String>,
}

/// "v0.3.1" or "0.3.1" → [0, 3, 1].
fn ver(v: &str) -> Vec<u64> {
    v.trim().trim_start_matches(['v', 'V']).split(['.', '-', '+']).take(3).map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn newer(theirs: &str, ours: &str) -> bool {
    ver(theirs) > ver(ours)
}

/// GitHub's answer for the newest release, read.
pub fn from_github(v: &serde_json::Value) -> Option<Release> {
    if v["draft"].as_bool() == Some(true) || v["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = v["tag_name"].as_str()?;
    Some(Release {
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        url: v["html_url"].as_str().filter(|u| u.starts_with("https://github.com/")).unwrap_or("https://github.com/ZomBeatz/zoms-modzilla/releases").to_string(),
        notes: v["body"].as_str().unwrap_or("").chars().take(4000).collect(),
        app_image: v["assets"].as_array().into_iter().flatten().filter_map(|a| a["browser_download_url"].as_str()).find(|u| u.ends_with(".AppImage") && u.starts_with("https://github.com/")).map(String::from),
    })
}

/// A newer release than this one, asked about at most once a day (the answer is kept).
pub async fn check(http: &reqwest::Client, cache: &Path, ours: &str) -> Option<Release> {
    let file = cache.join("release.json");
    let fresh = fs::metadata(&file).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|a| a < Duration::from_secs(86_400));
    let found: Option<Release> = if fresh {
        fs::read(&file).ok().and_then(|b| serde_json::from_slice(&b).ok())
    } else {
        let r = http.get(format!("https://api.github.com/repos/{REPO}/releases/latest")).header("accept", "application/vnd.github+json").timeout(Duration::from_secs(20)).send().await.ok()?;
        let found = if r.status().is_success() { r.json::<serde_json::Value>().await.ok().and_then(|v| from_github(&v)) } else { None };
        let _ = fs::create_dir_all(cache);
        let _ = fs::write(&file, serde_json::to_vec(&found).unwrap_or_default());
        found
    };
    found.filter(|r| newer(&r.version, ours))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_and_reads_releases() {
        assert!(newer("v0.3.0", "0.2.9") && newer("1.0.0", "0.9.12") && !newer("0.2.0", "0.2.0") && !newer("v0.1.9", "0.2.0"));
        let v = serde_json::json!({ "tag_name": "v0.3.0", "html_url": "https://github.com/ZomBeatz/zoms-modzilla/releases/tag/v0.3.0", "body": "Notes", "draft": false, "prerelease": false,
            "assets": [{ "browser_download_url": "https://github.com/ZomBeatz/zoms-modzilla/releases/download/v0.3.0/Zoms-Modzilla_0.3.0_amd64.AppImage" }, { "browser_download_url": "https://evil.example/x.AppImage" }] });
        let r = from_github(&v).unwrap();
        assert_eq!(r.version, "0.3.0");
        assert!(r.app_image.unwrap().starts_with("https://github.com/"));
        assert!(from_github(&serde_json::json!({ "tag_name": "v9", "prerelease": true })).is_none());
    }
}
