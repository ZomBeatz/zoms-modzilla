//! Games from Heroic (GOG, Epic, Amazon) and Lutris, next to Steam's: found by name among the
//! games Zom's Nexus knows, with their install folder, Wine/Proton prefix and how to start them.
//! Only read, never changed. Also which Proton each game runs with, and the Steam Deck.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    pub title: String,
    pub path: PathBuf,
    pub prefix: Option<PathBuf>,
    /// The shell command that starts it through its launcher.
    pub launch: String,
    /// "heroic" or "lutris".
    pub source: String,
    /// Wine or Proton build it uses, when the launcher says.
    pub wine: Option<String>,
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// A prefix folder as Zom's Nexus uses it (the one with drive_c in it).
fn prefix_of(p: &Path) -> Option<PathBuf> {
    [p.join("pfx"), p.to_path_buf()].into_iter().find(|x| x.join("drive_c").is_dir())
}

fn read_json(p: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(p).ok()?).ok()
}

/// Heroic's config folders (its own install, and the Flatpak).
pub fn heroic_dirs(home: &Path) -> Vec<PathBuf> {
    [home.join(".config/heroic"), home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic")].into_iter().filter(|d| d.is_dir()).collect()
}

/// Heroic's installed games.
pub fn heroic_in(dir: &Path) -> Vec<Found> {
    let mut out = vec![];
    let flatpak = dir.to_string_lossy().contains("com.heroicgameslauncher.hgl");
    let launch = |runner: &str, app: &str| {
        let url = format!("heroic://launch?appName={app}&runner={runner}");
        if flatpak { format!("flatpak run com.heroicgameslauncher.hgl '{url}'") } else { format!("xdg-open '{url}'") }
    };
    // (app name, title, path, runner)
    let mut games: Vec<(String, Option<String>, PathBuf, &str)> = vec![];
    if let Some(v) = read_json(&dir.join("legendaryConfig/legendary/installed.json")) {
        for (app, g) in v.as_object().into_iter().flatten() {
            if let Some(p) = g["install_path"].as_str() {
                games.push((app.clone(), g["title"].as_str().map(String::from), PathBuf::from(p), "legendary"));
            }
        }
    }
    let gog_titles: Vec<(String, String)> = [dir.join("store_cache/gog_library.json"), dir.join("gog_store/library.json")]
        .iter()
        .filter_map(|p| read_json(p))
        .flat_map(|v| v["games"].as_array().cloned().unwrap_or_default())
        .filter_map(|g| Some((g["app_name"].as_str()?.to_string(), g["title"].as_str()?.to_string())))
        .collect();
    if let Some(v) = read_json(&dir.join("gog_store/installed.json")) {
        for g in v["installed"].as_array().into_iter().flatten() {
            let (Some(app), Some(p)) = (g["appName"].as_str(), g["install_path"].as_str()) else { continue };
            let title = gog_titles.iter().find(|(a, _)| a == app).map(|(_, t)| t.clone());
            games.push((app.to_string(), title, PathBuf::from(p), "gog"));
        }
    }
    if let Some(v) = read_json(&dir.join("nile_config/nile/installed.json")) {
        for g in v.as_array().into_iter().flatten() {
            let (Some(app), Some(p)) = (g["id"].as_str(), g["path"].as_str()) else { continue };
            games.push((app.to_string(), None, PathBuf::from(p), "nile"));
        }
    }
    for (app, title, path, runner) in games {
        if !path.is_dir() {
            continue;
        }
        let cfg = read_json(&dir.join("GamesConfig").join(format!("{app}.json")));
        let c = cfg.as_ref().map(|v| &v[app.as_str()]);
        let prefix = c.and_then(|c| c["winePrefix"].as_str()).map(PathBuf::from).and_then(|p| prefix_of(&p));
        let wine = c.and_then(|c| c["wineVersion"]["name"].as_str()).map(String::from);
        let title = title.unwrap_or_else(|| path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        out.push(Found { title, path, prefix, launch: launch(runner, &app), source: "heroic".into(), wine });
    }
    out
}

/// Lutris' game files (one .yml each).
pub fn lutris_dirs(home: &Path) -> Vec<PathBuf> {
    [home.join(".config/lutris/games"), home.join(".local/share/lutris/games"), home.join(".var/app/net.lutris.Lutris/config/lutris/games"), home.join(".var/app/net.lutris.Lutris/data/lutris/games")].into_iter().filter(|d| d.is_dir()).collect()
}

/// A value from Lutris' simple YAML ("  exe: /path").
fn yml(text: &str, section: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in text.lines() {
        if !line.starts_with(' ') && !line.starts_with('\t') {
            inside = line.trim_end() == format!("{section}:");
            continue;
        }
        if inside {
            if let Some(v) = line.trim().strip_prefix(&format!("{key}:")) {
                let v = v.trim().trim_matches(['\'', '"']).to_string();
                return (!v.is_empty()).then_some(v);
            }
        }
    }
    None
}

/// The game's own folder from its program's path: up past "bin", "x64", "Binaries/Win64".
pub fn game_root(exe: &Path) -> PathBuf {
    let mut d = exe.parent().map(Path::to_path_buf).unwrap_or_default();
    while d.file_name().map(|n| matches!(n.to_string_lossy().to_lowercase().as_str(), "bin" | "x64" | "x86" | "win64" | "win32" | "binaries" | "bin64" | "_retail_" | "game")).unwrap_or(false) {
        match d.parent() {
            Some(p) => d = p.to_path_buf(),
            None => break,
        }
    }
    d
}

pub fn lutris_in(dir: &Path, flatpak: bool) -> Vec<Found> {
    let mut out = vec![];
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "yml") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&p) else { continue };
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        // "the-witcher-3-wild-hunt-1700000000" → slug and title.
        let slug = stem.rsplit_once('-').filter(|(_, n)| n.chars().all(|c| c.is_ascii_digit())).map(|(s, _)| s.to_string()).unwrap_or(stem.clone());
        let title = slug.split('-').map(|w| { let mut c = w.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }).collect::<Vec<_>>().join(" ");
        let Some(exe) = yml(&text, "game", "exe") else { continue };
        let exe = PathBuf::from(exe);
        let path = yml(&text, "game", "working_dir").map(PathBuf::from).filter(|d| d.is_dir()).unwrap_or_else(|| game_root(&exe));
        if !path.is_dir() {
            continue;
        }
        let prefix = yml(&text, "game", "prefix").map(PathBuf::from).and_then(|p| prefix_of(&p));
        let wine = yml(&text, "wine", "version");
        let launch = if flatpak { format!("flatpak run net.lutris.Lutris lutris:rungame/{slug}") } else { format!("lutris lutris:rungame/{slug}") };
        out.push(Found { title, path, prefix, launch, source: "lutris".into(), wine });
    }
    out
}

/// Every Heroic and Lutris game on this computer.
pub fn all() -> Vec<Found> {
    let Some(home) = home() else { return vec![] };
    let mut out: Vec<Found> = heroic_dirs(&home).iter().flat_map(|d| heroic_in(d)).collect();
    for d in lutris_dirs(&home) {
        let flatpak = d.to_string_lossy().contains("net.lutris.Lutris");
        out.extend(lutris_in(&d, flatpak));
    }
    out
}

fn squash(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

/// The known game a launcher's title is ("The Witcher 3: Wild Hunt" → "witcher3").
pub fn known_for(title: &str) -> Option<(&'static str, &'static str)> {
    let t = squash(title).trim_start_matches("the").to_string();
    crate::games::KNOWN
        .iter()
        .map(|(d, n, _)| (*d, *n, squash(n).trim_start_matches("the").to_string()))
        .filter(|(_, _, k)| k.len() >= 5 && (t == *k || t.starts_with(k.as_str())))
        // The longest name wins ("Fallout 4 VR" isn't "Fallout 4"; "Skyrim Special Edition" over "Skyrim").
        .max_by_key(|(_, _, k)| k.len())
        .map(|(d, n, _)| (d, n))
}

/// The Proton build Steam last ran a game with (from its prefix's own note).
pub fn steam_proton(prefix: &Path) -> Option<String> {
    let base = if prefix.file_name().is_some_and(|n| n == "pfx") { prefix.parent()? } else { prefix };
    let v = fs::read_to_string(base.join("version")).ok()?;
    let v = v.trim();
    (!v.is_empty()).then(|| if v.chars().next().is_some_and(|c| c.is_ascii_digit()) { format!("Proton {v}") } else { v.to_string() })
}

/// Proton builds added by hand (GE-Proton and friends), newest name last.
pub fn custom_protons() -> Vec<String> {
    let Some(home) = home() else { return vec![] };
    let mut out: Vec<String> = crate::games::steam_roots(&home)
        .into_iter()
        .chain([home.join(".steam/root"), home.join(".var/app/com.valvesoftware.Steam/data/Steam")])
        .flat_map(|r| fs::read_dir(r.join("compatibilitytools.d")).into_iter().flatten().flatten())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// What kind of computer this is.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    /// A Steam Deck (LCD or OLED).
    pub deck: bool,
    /// SteamOS (read-only system: install apps as Flatpak or AppImage).
    pub steamos: bool,
    /// Running inside Steam's Game Mode (gamescope).
    pub game_mode: bool,
    /// Installed as a Flatpak.
    pub flatpak: bool,
}

pub fn device() -> Device {
    let read = |p: &str| fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default();
    let vendor = read("/sys/devices/virtual/dmi/id/board_vendor");
    let product = read("/sys/devices/virtual/dmi/id/product_name");
    let os = read("/etc/os-release").to_lowercase();
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    Device {
        deck: vendor.eq_ignore_ascii_case("Valve") && matches!(product.as_str(), "Jupiter" | "Galileo"),
        steamos: os.lines().any(|l| l == "id=steamos" || l == "id=\"steamos\""),
        game_mode: env("XDG_CURRENT_DESKTOP").eq_ignore_ascii_case("gamescope") || env("SteamGamepadUI") == "1" || !env("GAMESCOPE_WAYLAND_DISPLAY").is_empty(),
        flatpak: Path::new("/.flatpak-info").exists(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_heroic_and_lutris() {
        let t = tempfile::tempdir().unwrap();
        let heroic = t.path().join("heroic");
        let game = t.path().join("Games/The Witcher 3 Wild Hunt GOTY");
        let pfx = t.path().join("Prefixes/Witcher");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(pfx.join("pfx/drive_c")).unwrap();
        fs::create_dir_all(heroic.join("gog_store")).unwrap();
        fs::create_dir_all(heroic.join("GamesConfig")).unwrap();
        fs::write(heroic.join("gog_store/installed.json"), format!(r#"{{"installed":[{{"appName":"1207664663","install_path":"{}","platform":"windows"}}]}}"#, game.display())).unwrap();
        fs::write(heroic.join("GamesConfig/1207664663.json"), format!(r#"{{"1207664663":{{"winePrefix":"{}","wineVersion":{{"name":"GE-Proton9-20","type":"proton"}}}}}}"#, pfx.display())).unwrap();
        let h = heroic_in(&heroic);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].title, "The Witcher 3 Wild Hunt GOTY");
        assert_eq!(h[0].prefix.as_deref(), Some(pfx.join("pfx").as_path()));
        assert_eq!(h[0].wine.as_deref(), Some("GE-Proton9-20"));
        assert!(h[0].launch.contains("heroic://launch?appName=1207664663&runner=gog"));
        assert_eq!(known_for(&h[0].title), Some(("witcher3", "The Witcher 3")));

        let lut = t.path().join("lutris");
        let sky = t.path().join("Games/skyrim/drive_c/Games/Skyrim Special Edition");
        fs::create_dir_all(&sky).unwrap();
        fs::create_dir_all(&lut).unwrap();
        fs::write(lut.join("skyrim-special-edition-1700000000.yml"), format!("game:\n  exe: {}/SkyrimSE.exe\n  prefix: {}\nwine:\n  version: lutris-GE-Proton8-26\n", sky.display(), t.path().join("Games/skyrim").display())).unwrap();
        fs::create_dir_all(t.path().join("Games/skyrim/drive_c")).unwrap();
        let l = lutris_in(&lut, false);
        assert_eq!((l[0].title.as_str(), l[0].path.as_path()), ("Skyrim Special Edition", sky.as_path()));
        assert_eq!(l[0].launch, "lutris lutris:rungame/skyrim-special-edition");
        assert_eq!(known_for(&l[0].title), Some(("skyrimspecialedition", "Skyrim Special Edition")));
        assert_eq!(game_root(Path::new("/g/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe")), PathBuf::from("/g/Cyberpunk 2077"));
        assert_eq!(known_for("Fallout 4"), Some(("fallout4", "Fallout 4")));
        assert_eq!(known_for("Some Other Game"), None);
    }

    #[test]
    fn reads_the_proton_note() {
        let t = tempfile::tempdir().unwrap();
        fs::create_dir_all(t.path().join("pfx")).unwrap();
        fs::write(t.path().join("version"), "9.0-302\n").unwrap();
        assert_eq!(steam_proton(&t.path().join("pfx")).as_deref(), Some("Proton 9.0-302"));
        fs::write(t.path().join("version"), "GE-Proton9-20").unwrap();
        assert_eq!(steam_proton(&t.path().join("pfx")).as_deref(), Some("GE-Proton9-20"));
    }
}
