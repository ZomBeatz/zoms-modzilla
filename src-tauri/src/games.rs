//! Games on this computer: the ones Steam has installed (native or through Proton) that are on
//! Nexus Mods, plus any you add by hand.

use serde::Serialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

/// Nexus Mods game name, the name people know, and its Steam app number.
pub const KNOWN: &[(&str, &str, u32)] = &[
    ("skyrimspecialedition", "Skyrim Special Edition", 489830),
    ("skyrim", "Skyrim", 72850),
    ("fallout4", "Fallout 4", 377160),
    ("newvegas", "Fallout: New Vegas", 22380),
    ("fallout3", "Fallout 3", 22300),
    ("fallout3", "Fallout 3", 22370),
    ("oblivion", "Oblivion", 22330),
    ("oblivionremastered", "Oblivion Remastered", 2623190),
    ("morrowind", "Morrowind", 22320),
    ("starfield", "Starfield", 1716740),
    ("cyberpunk2077", "Cyberpunk 2077", 1091500),
    ("baldursgate3", "Baldur's Gate 3", 1086940),
    ("witcher3", "The Witcher 3", 292030),
    ("stardewvalley", "Stardew Valley", 413150),
    ("eldenring", "Elden Ring", 1245620),
    ("darksouls3", "Dark Souls III", 374320),
    ("sekiro", "Sekiro", 814380),
    ("monsterhunterworld", "Monster Hunter: World", 582010),
    ("monsterhunterwilds", "Monster Hunter Wilds", 2246340),
    ("mountandblade2bannerlord", "Mount & Blade II: Bannerlord", 261550),
    ("valheim", "Valheim", 892970),
    ("nomanssky", "No Man's Sky", 275850),
    ("kingdomcomedeliverance", "Kingdom Come: Deliverance", 379430),
    ("kingdomcomedeliverance2", "Kingdom Come: Deliverance II", 1771300),
    ("palworld", "Palworld", 1623730),
    ("hogwartslegacy", "Hogwarts Legacy", 990080),
    ("subnautica", "Subnautica", 264710),
    ("rimworld", "RimWorld", 294100),
    ("bladeandsorcery", "Blade & Sorcery", 629730),
    ("xcom2", "XCOM 2", 268500),
    ("citiesskylines", "Cities: Skylines", 255710),
    ("stalker2heartofchornobyl", "S.T.A.L.K.E.R. 2", 1643320),
    // Not here: games whose mods go in through their own tool (Terraria's tModLoader, Mass
    // Effect Legendary Edition's ME3Tweaks), since Zom's Modzilla can't put their mods in place.
    // They can still be added by hand, for downloading.
    ("helldivers2", "Helldivers 2", 553850),
    ("finalfantasy7rebirth", "Final Fantasy VII Rebirth", 2909400),
    ("clairobscurexpedition33", "Clair Obscur: Expedition 33", 1903340),
    ("icarus", "Icarus", 1149460),
    ("bayonetta", "Bayonetta", 460790),
];

pub fn known_app(domain: &str) -> Option<u32> {
    KNOWN.iter().find(|(d, _, _)| *d == domain).map(|(_, _, a)| *a)
}

pub fn known_name(domain: &str) -> Option<&'static str> {
    KNOWN.iter().find(|(d, _, _)| *d == domain).map(|(_, n, _)| *n)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Game {
    pub domain: String,
    pub name: String,
    /// Where the game is installed.
    pub path: Option<String>,
    /// "steam" when Steam has it, "added" when you added it.
    pub source: String,
    pub steam_app_id: Option<u32>,
    /// The Proton prefix (the game's Windows folders) when it runs through Proton.
    pub proton_prefix: Option<String>,
    /// Where mods go under the install folder; None when it isn't known yet.
    pub mod_folder: Option<String>,
    /// The start command for a game that isn't in Steam.
    pub launch: Option<String>,
    /// The profile whose mods are in the game folder now.
    pub active_profile: Option<String>,
    /// Free space on the drive the game is installed on, in bytes.
    pub free_space: Option<u64>,
    /// The size of that drive, in bytes.
    pub total_space: Option<u64>,
    /// When Steam last updated the game (Unix time), from its appmanifest.
    pub last_updated: Option<u64>,
    /// When its mods go in through the game's own tool instead of a folder: what to say.
    pub mod_tool: Option<String>,
    /// The library is on another drive, so mods are copied into this game, not linked.
    pub mods_copied: bool,
    /// Where the game keeps its saves (set by the person, or known), for saves per profile.
    pub save_folder: Option<String>,
    /// Steam keeps this game's saves in its cloud (so it puts saves back into an emptied
    /// save folder, which defeats saves per profile).
    pub steam_cloud: bool,
    /// The Wine/Proton build it runs with, when known ("Proton 9.0-302", "GE-Proton9-20").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proton: Option<String>,
}

/// Whether Steam keeps this game's saves in its cloud: it has cloud files for it, and the
/// game's own switch (Properties › General, kept in sharedconfig.vdf as cloudenabled "0") isn't
/// off.
pub fn steam_cloud(app_id: u32) -> bool {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return false };
    let users: Vec<PathBuf> = steam_roots(&home).into_iter().flat_map(|r| fs::read_dir(r.join("userdata")).into_iter().flatten().flatten().map(|u| u.path())).collect();
    users.iter().any(|u| {
        let has_files = fs::read_to_string(u.join(app_id.to_string()).join("remotecache.vdf")).map(|t| t.contains("\"root\"")).unwrap_or(false);
        let switched_off = fs::read_to_string(u.join("7/remote/sharedconfig.vdf")).ok().and_then(|t| crate::launchopts::app_value(&t, app_id, "cloudenabled")).as_deref() == Some("0");
        has_files && !switched_off
    })
}

/// A Steam library folder (one per drive Steam installs games on), and what's in it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibrary {
    pub path: String,
    /// How many games and tools Steam has installed there.
    pub installed: usize,
    /// The ones Zom's Modzilla knows are on Nexus Mods.
    pub nexus_games: Vec<String>,
    pub free_space: Option<u64>,
    pub total_space: Option<u64>,
}

/// Every Steam folder that might be in use: the usual one, the old links to it, Flatpak and Snap.
pub fn steam_roots(home: &Path) -> Vec<PathBuf> {
    [
        ".local/share/Steam",
        ".steam/steam",
        ".steam/root",
        ".var/app/com.valvesoftware.Steam/.local/share/Steam",
        "snap/steam/common/.local/share/Steam",
    ]
    .iter()
    .map(|p| home.join(p))
    .filter(|p| p.join("steamapps").is_dir())
    .collect()
}

/// The `"path"` lines of libraryfolders.vdf: every drive Steam installs games on.
pub fn library_paths(vdf: &str) -> Vec<PathBuf> {
    vdf.lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix("\"path\"")?.trim();
            let v = rest.strip_prefix('"')?.strip_suffix('"')?;
            Some(PathBuf::from(v.replace("\\\\", "\\")))
        })
        .collect()
}

/// `"installdir"` from an appmanifest_*.acf.
pub fn install_dir(acf: &str) -> Option<String> {
    acf_value(acf, "installdir")
}

/// A top-level `"key" "value"` from an appmanifest_*.acf.
pub fn acf_value(acf: &str, key: &str) -> Option<String> {
    let want = format!("\"{key}\"");
    acf.lines().find_map(|l| {
        let rest = l.trim().strip_prefix(&want)?.trim();
        Some(rest.strip_prefix('"')?.strip_suffix('"')?.to_string())
    })
}

/// Every Steam library folder on this computer: Steam's own folder (native, Flatpak or Snap)
/// and each drive listed in its libraryfolders.vdf.
pub fn steam_library_dirs() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return vec![] };
    let mut libs: Vec<PathBuf> = vec![];
    let mut seen = HashSet::new();
    for root in steam_roots(&home) {
        let mut found = vec![root.clone()];
        if let Ok(vdf) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            found.extend(library_paths(&vdf));
        }
        for lib in found {
            let real = fs::canonicalize(&lib).unwrap_or(lib.clone());
            if real.join("steamapps").is_dir() && seen.insert(real.clone()) {
                libs.push(real);
            }
        }
    }
    libs
}

/// A game Steam has installed (any game, not only ones Zom's Modzilla knows).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamApp {
    pub app_id: u32,
    pub name: String,
    pub path: String,
}

/// Everything Steam has installed, in every library folder (tools like Proton left out).
pub fn steam_installed() -> Vec<SteamApp> {
    let mut out = vec![];
    for lib in steam_library_dirs() {
        let Ok(rd) = fs::read_dir(lib.join("steamapps")) else { continue };
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            let Some(id) = n.strip_prefix("appmanifest_").and_then(|x| x.strip_suffix(".acf")).and_then(|x| x.parse::<u32>().ok()) else { continue };
            let Ok(acf) = fs::read_to_string(e.path()) else { continue };
            let (Some(name), Some(dir)) = (acf_value(&acf, "name"), install_dir(&acf)) else { continue };
            let low = name.to_lowercase();
            if low.starts_with("proton") || low.starts_with("steam linux runtime") || low.starts_with("steamworks") {
                continue;
            }
            let path = lib.join("steamapps/common").join(dir);
            if path.is_dir() {
                out.push(SteamApp { app_id: id, name, path: path.to_string_lossy().into_owned() });
            }
        }
    }
    out.sort_by_key(|a| a.name.to_lowercase());
    out
}

/// The Proton folder for a Steam game installed at `path` (…/steamapps/common/<game>).
pub fn proton_prefix_for(path: &Path, app_id: u32) -> Option<PathBuf> {
    let steamapps = path.parent()?.parent()?;
    let pfx = steamapps.join("compatdata").join(app_id.to_string()).join("pfx");
    pfx.is_dir().then_some(pfx)
}

/// The Steam library folders, with how many games each has and which are Nexus Mods games.
pub fn steam_libraries() -> Vec<SteamLibrary> {
    steam_library_dirs()
        .into_iter()
        .map(|lib| {
            let manifests: Vec<String> = fs::read_dir(lib.join("steamapps"))
                .map(|rd| rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("appmanifest_") && n.ends_with(".acf")).collect())
                .unwrap_or_default();
            let mut nexus_games: Vec<String> = KNOWN.iter().filter(|(_, _, app)| manifests.iter().any(|m| m == &format!("appmanifest_{app}.acf"))).map(|(_, n, _)| n.to_string()).collect();
            nexus_games.dedup();
            let disk = crate::settings::disk(&lib);
            SteamLibrary {
                path: lib.to_string_lossy().into_owned(),
                installed: manifests.len(),
                nexus_games,
                free_space: disk.map(|d| d.0),
                total_space: disk.map(|d| d.1),
            }
        })
        .collect()
}

pub fn steam_games() -> Vec<Game> {
    let libs = steam_library_dirs();
    let mut games = vec![];
    let mut have = HashSet::new();
    for lib in &libs {
        for (domain, name, app) in KNOWN {
            let Ok(acf) = fs::read_to_string(lib.join(format!("steamapps/appmanifest_{app}.acf"))) else { continue };
            if !have.insert(*domain) {
                continue;
            }
            let path = install_dir(&acf).map(|d| lib.join("steamapps/common").join(d)).filter(|p| p.is_dir());
            let pfx = lib.join(format!("steamapps/compatdata/{app}/pfx"));
            games.push(Game {
                domain: domain.to_string(),
                name: name.to_string(),
                path: path.map(|p| p.to_string_lossy().into_owned()),
                source: "steam".into(),
                steam_app_id: Some(*app),
                proton_prefix: pfx.is_dir().then(|| pfx.to_string_lossy().into_owned()),
                mod_folder: crate::deploy::default_mod_folder(domain).map(String::from),
                launch: None,
                active_profile: None,
                free_space: None,
                total_space: None,
                last_updated: acf_value(&acf, "LastUpdated").and_then(|v| v.parse().ok()).filter(|&t: &u64| t > 0),
                mod_tool: None,
                mods_copied: false,
                save_folder: None,
                steam_cloud: false,
                proton: None,
            });
        }
    }
    games.sort_by(|a, b| a.name.cmp(&b.name));
    games
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_steam_files() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"/home/alex/.local/share/Steam\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"/mnt/games/SteamLibrary\"\n\t\t\"label\"\t\t\"\"\n\t}\n}";
        assert_eq!(library_paths(vdf), vec![PathBuf::from("/home/alex/.local/share/Steam"), PathBuf::from("/mnt/games/SteamLibrary")]);
        let acf = "\"AppState\"\n{\n\t\"appid\"\t\t\"489830\"\n\t\"installdir\"\t\t\"Skyrim Special Edition\"\n}";
        assert_eq!(install_dir(acf).as_deref(), Some("Skyrim Special Edition"));
        let acf2 = "\"AppState\"\n{\n\t\"LastUpdated\"\t\t\"1763512345\"\n}";
        assert_eq!(acf_value(acf2, "LastUpdated").as_deref(), Some("1763512345"));
    }
}
