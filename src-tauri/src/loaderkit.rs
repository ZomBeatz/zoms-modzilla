//! One-click loaders that aren't on Nexus Mods: UE4SS (Lua and Blueprint mods for Unreal Engine
//! games, from its GitHub releases) and ReShade (from reshade.me, with the standard shaders from
//! GitHub). Each becomes a mod of your own in the library, built for the game's folder, so a
//! profile puts it in and takes it out like any other mod. Under Proton both need a Steam
//! launch option to load (WINEDLLOVERRIDES); `launchopts` finds that and says what to paste.

use serde::Serialize;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

/// Where an Unreal Engine game's real program is: `<Game>/Binaries/Win64` (with *-Shipping.exe).
pub fn unreal_dir(root: &Path) -> Option<String> {
    for e in fs::read_dir(root).ok()?.flatten() {
        let win = e.path().join("Binaries/Win64");
        let has = fs::read_dir(&win).into_iter().flatten().flatten().any(|f| {
            let n = f.file_name().to_string_lossy().to_lowercase();
            n.ends_with("-shipping.exe")
        });
        if has {
            return Some(format!("{}/Binaries/Win64", e.file_name().to_string_lossy()));
        }
    }
    None
}

/// The folder (under the game folder) with the game's program: Unreal's, or the top.
pub fn exe_dir(root: &Path) -> String {
    unreal_dir(root).unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub id: &'static str,
    pub name: &'static str,
    pub what: &'static str,
    /// The DLL Wine has to be told to use (for the launch options).
    pub dll: &'static str,
}

pub fn offers(root: &Path) -> Vec<Offer> {
    let mut v = vec![];
    if unreal_dir(root).is_some() {
        v.push(Offer { id: "ue4ss", name: "UE4SS", what: "Lua and Blueprint mods for Unreal Engine games, and the mods that need it.", dll: "dwmapi" });
    }
    v.push(Offer { id: "reshade-dxgi", name: "ReShade (DirectX 10–12)", what: "Post-processing shaders: colour, sharpening, ambient occlusion.", dll: "dxgi" });
    v.push(Offer { id: "reshade-d3d9", name: "ReShade (DirectX 9)", what: "ReShade for older DirectX 9 games.", dll: "d3d9" });
    v
}

/// Unpack a zip's files into `to`: each one's path made by `place` (None skips it).
pub fn unzip(bytes: &[u8], to: &Path, place: impl Fn(&Path) -> Option<PathBuf>) -> Result<usize, String> {
    let mut z = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("The download isn't a zip: {e}"))?;
    let mut n = 0;
    let mut total = 0u64;
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        if f.is_dir() {
            continue;
        }
        let Some(rel) = f.enclosed_name() else { continue };
        let Some(dest) = place(&rel) else { continue };
        if dest.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
            continue;
        }
        let out = to.join(dest);
        fs::create_dir_all(out.parent().unwrap()).map_err(|e| e.to_string())?;
        let mut b = vec![];
        f.by_ref().take(512 * 1024 * 1024).read_to_end(&mut b).map_err(|e| e.to_string())?;
        total += b.len() as u64;
        if total > 2 * 1024 * 1024 * 1024 {
            return Err("The download unpacks to far more than a loader should.".into());
        }
        fs::write(&out, b).map_err(|e| e.to_string())?;
        n += 1;
    }
    Ok(n)
}

/// Make the built folder a mod's files go in from: `<mod>/unpacked`, marked for the game folder.
pub fn built(dir: &Path) -> Result<PathBuf, String> {
    let u = dir.join("unpacked");
    fs::create_dir_all(&u).map_err(|e| e.to_string())?;
    fs::write(u.join(crate::fomod::MARK), "").map_err(|e| e.to_string())?;
    fs::write(u.join(crate::deploy::GAME_ROOT_MARK), "").map_err(|e| e.to_string())?;
    Ok(u)
}

/// The UE4SS zip in a GitHub release (not the developer build or the extras).
pub fn ue4ss_asset(release: &serde_json::Value) -> Option<(String, String)> {
    let tag = release["tag_name"].as_str()?.to_string();
    let url = release["assets"].as_array()?.iter().filter_map(|a| a["browser_download_url"].as_str()).find(|u| {
        let n = u.rsplit('/').next().unwrap_or("");
        n.starts_with("UE4SS_") && n.ends_with(".zip") && u.starts_with("https://github.com/")
    })?;
    Some((url.to_string(), tag))
}

/// The newest ReShade setup's file name on reshade.me's front page.
pub fn reshade_setup(page: &str) -> Option<String> {
    let at = page.find("/downloads/ReShade_Setup_")?;
    let rest = &page[at + "/downloads/".len()..];
    let end = rest.find(".exe")?;
    let name = &rest[..end + 4];
    let ver = name.strip_prefix("ReShade_Setup_")?.strip_suffix(".exe")?;
    (ver.chars().all(|c| c.is_ascii_digit() || c == '.') && !ver.is_empty()).then(|| name.to_string())
}

pub const RESHADE_INI: &str = "[GENERAL]\r\nEffectSearchPaths=.\\reshade-shaders\\Shaders\\**\r\nTextureSearchPaths=.\\reshade-shaders\\Textures\\**\r\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_unreal_games_and_the_right_downloads() {
        let t = tempfile::tempdir().unwrap();
        assert!(unreal_dir(t.path()).is_none());
        assert_eq!(offers(t.path()).len(), 2);
        fs::create_dir_all(t.path().join("Pal/Binaries/Win64")).unwrap();
        fs::write(t.path().join("Pal/Binaries/Win64/Palworld-Win64-Shipping.exe"), "").unwrap();
        assert_eq!(unreal_dir(t.path()).as_deref(), Some("Pal/Binaries/Win64"));
        assert_eq!(offers(t.path())[0].id, "ue4ss");
        let rel = serde_json::json!({ "tag_name": "v3.0.1", "assets": [
            { "browser_download_url": "https://github.com/UE4SS-RE/RE-UE4SS/releases/download/v3.0.1/zDEV-UE4SS_v3.0.1.zip" },
            { "browser_download_url": "https://github.com/UE4SS-RE/RE-UE4SS/releases/download/v3.0.1/UE4SS_v3.0.1.zip" } ] });
        assert_eq!(ue4ss_asset(&rel).unwrap().0, "https://github.com/UE4SS-RE/RE-UE4SS/releases/download/v3.0.1/UE4SS_v3.0.1.zip");
        assert_eq!(reshade_setup(r#"<a href="/downloads/ReShade_Setup_6.4.1.exe">Download</a>"#).as_deref(), Some("ReShade_Setup_6.4.1.exe"));
        assert_eq!(reshade_setup(r#"<a href="/downloads/ReShade_Setup_../../x.exe">"#), None);
        // Unpacking stays inside.
        let mut buf = std::io::Cursor::new(vec![]);
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            z.start_file("dwmapi.dll", zip::write::SimpleFileOptions::default()).unwrap();
            std::io::Write::write_all(&mut z, b"dll").unwrap();
            z.start_file("ue4ss/UE4SS.dll", zip::write::SimpleFileOptions::default()).unwrap();
            z.finish().unwrap();
        }
        let out = t.path().join("out");
        assert_eq!(unzip(buf.get_ref(), &out, |r| Some(Path::new("Pal/Binaries/Win64").join(r))).unwrap(), 2);
        assert!(out.join("Pal/Binaries/Win64/ue4ss/UE4SS.dll").exists());
    }
}
