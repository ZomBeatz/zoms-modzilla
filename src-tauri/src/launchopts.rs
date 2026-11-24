//! Mod loaders for Windows games often come in as a stand-in DLL next to the game (winmm.dll for
//! RED4ext, version.dll for Cyber Engine Tweaks, dxgi.dll for ReShade, dinput8.dll …). Under
//! Proton, Wine uses its own copy of such a DLL unless the game's Steam launch options say
//! otherwise (`WINEDLLOVERRIDES="winmm,version=n,b" %command%`), so the loader silently never
//! runs. This finds those DLLs, reads the launch options, and says what to paste.
//!
//! Steam's settings are only read here, never written: Steam rewrites them while it runs.

use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// DLL names mod loaders stand in for.
const PROXIES: &[&str] = &["winmm", "version", "dxgi", "d3d11", "d3d9", "dinput8", "xinput1_3", "xinput1_4", "dsound", "winhttp", "bink2w64", "dwmapi", "opengl32"];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchFix {
    /// DLLs in the game folder that the launch options don't tell Wine to use.
    pub missing: Vec<String>,
    pub current: String,
    /// The launch options to paste instead.
    pub suggested: String,
}

/// Stand-in DLLs in the game folder (its top and up to three folders down).
pub fn proxy_dlls(root: &Path) -> Vec<String> {
    let mut found = vec![];
    let mut stack = vec![(root.to_path_buf(), 0)];
    while let Some((d, depth)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let Ok(t) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().to_lowercase();
            if t.is_dir() && depth < 3 && !name.starts_with('.') {
                stack.push((e.path(), depth + 1));
            } else if t.is_file() {
                if let Some(stem) = name.strip_suffix(".dll") {
                    if PROXIES.contains(&stem) && !found.iter().any(|f| f == stem) {
                        found.push(stem.to_string());
                    }
                }
            }
        }
    }
    found.sort();
    found
}

/// The game's launch options in Steam (the newest localconfig.vdf that has them).
pub fn launch_options(app_id: u32) -> Option<String> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = crate::games::steam_roots(&home)
        .into_iter()
        .flat_map(|r| fs::read_dir(r.join("userdata")).into_iter().flatten().flatten().map(|u| u.path().join("config/localconfig.vdf")))
        .filter_map(|p| Some((fs::metadata(&p).ok()?.modified().ok()?, p)))
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0));
    files.iter().find_map(|(_, p)| from_vdf(&fs::read_to_string(p).ok()?, app_id))
}

/// LaunchOptions in the app's block of a localconfig.vdf.
pub fn from_vdf(vdf: &str, app_id: u32) -> Option<String> {
    app_value(vdf, app_id, "LaunchOptions")
}

/// A value in the app's block of one of Steam's .vdf files.
pub fn app_value(vdf: &str, app_id: u32, name: &str) -> Option<String> {
    let want = format!("\"{name}\"");
    let key = format!("\"{app_id}\"");
    let mut at = 0;
    while let Some(i) = vdf[at..].find(&key) {
        let start = at + i + key.len();
        at = start;
        // A block: the key, then "{" on the next line.
        let rest = &vdf[start..];
        let Some(open) = rest.find(|c: char| !c.is_whitespace()) else { break };
        if !rest[open..].starts_with('{') {
            continue;
        }
        let mut depth = 0;
        let mut end = rest.len();
        for (j, c) in rest[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + j;
                        break;
                    }
                }
                _ => {}
            }
        }
        let block = &rest[open..end];
        if let Some(k) = block.find(&want) {
            let after = block[k + want.len()..].trim_start();
            let after = after.strip_prefix('"')?;
            let mut out = String::new();
            let mut chars = after.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => out.extend(chars.next()),
                    '"' => break,
                    c => out.push(c),
                }
            }
            return Some(out);
        }
    }
    None
}

/// The DLLs WINEDLLOVERRIDES tells Wine to use the game's own copy of ("n" or "n,b").
fn overridden(launch: &str) -> (Vec<String>, Option<(usize, usize)>) {
    let Some(i) = launch.find("WINEDLLOVERRIDES=") else { return (vec![], None) };
    let v = &launch[i + 17..];
    let (value, len) = if let Some(q) = v.strip_prefix('"') { let e = q.find('"').unwrap_or(q.len()); (&q[..e], e + 2) } else { let e = v.find(' ').unwrap_or(v.len()); (&v[..e], e) };
    let mut names = vec![];
    for entry in value.split(';') {
        if let Some((dlls, mode)) = entry.split_once('=') {
            if mode.trim().starts_with('n') {
                names.extend(dlls.split(',').map(|d| d.trim().to_lowercase()).filter(|d| !d.is_empty()));
            }
        }
    }
    (names, Some((i, 17 + len)))
}

/// What to change, or None when the launch options already cover every stand-in DLL.
pub fn fix(launch: &str, dlls: &[String]) -> Option<LaunchFix> {
    let (have, span) = overridden(launch);
    let missing: Vec<String> = dlls.iter().filter(|d| !have.contains(d)).cloned().collect();
    if missing.is_empty() {
        return None;
    }
    let mut all = have.clone();
    all.extend(missing.iter().cloned());
    let setting = format!("WINEDLLOVERRIDES=\"{}=n,b\"", all.join(","));
    let suggested = match span {
        Some((i, len)) => format!("{}{}{}", &launch[..i], setting, &launch[i + len..]),
        None if launch.trim().is_empty() => format!("{setting} %command%"),
        None if launch.contains("%command%") => format!("{setting} {launch}"),
        None => format!("{setting} %command% {launch}"),
    };
    Some(LaunchFix { missing, current: launch.to_string(), suggested })
}

/// Stardew Valley's Linux build with SMAPI in its folder: Steam has to start SMAPI's launcher
/// instead of the game's (the Windows build doesn't need this: SMAPI stands in for the game's
/// launcher there).
pub fn smapi_fix(launch: &str, root: &Path) -> Option<LaunchFix> {
    let smapi = root.join("StardewModdingAPI");
    if !smapi.is_file() || root.join("Stardew Valley.exe").is_file() || launch.contains("StardewModdingAPI") {
        return None;
    }
    let run = format!("\"{}\" %command%", smapi.display());
    let suggested = if launch.contains("%command%") { launch.replacen("%command%", &run, 1) } else { format!("{run} {launch}").trim().to_string() };
    Some(LaunchFix { missing: vec!["SMAPI".into()], current: launch.to_string(), suggested })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_what_the_launch_options_leave_out() {
        let dlls = vec!["version".to_string(), "winmm".to_string()];
        let f = fix(r#"WINEDLLOVERRIDES="version=n,b" %command% --launcher-skip"#, &dlls).unwrap();
        assert_eq!(f.missing, vec!["winmm"]);
        assert_eq!(f.suggested, r#"WINEDLLOVERRIDES="version,winmm=n,b" %command% --launcher-skip"#);
        assert!(fix(r#"WINEDLLOVERRIDES="winmm,version=n,b" %command%"#, &dlls).is_none());
        assert_eq!(fix("", &dlls).unwrap().suggested, r#"WINEDLLOVERRIDES="version,winmm=n,b" %command%"#);
        assert_eq!(fix("--launcher-skip", &dlls).unwrap().suggested, r#"WINEDLLOVERRIDES="version,winmm=n,b" %command% --launcher-skip"#);

        let vdf = "\"apps\"\n{\n\t\"1091500\"\n\t{\n\t\t\"LastPlayed\"\t\t\"1\"\n\t\t\"LaunchOptions\"\t\t\"WINEDLLOVERRIDES=\\\"version=n,b\\\" %command%\"\n\t}\n\t\"20\"\n\t{\n\t}\n}\n\"HintAppsToPreload\"\t\t\"1091500\"\n";
        assert_eq!(from_vdf(vdf, 1091500).as_deref(), Some(r#"WINEDLLOVERRIDES="version=n,b" %command%"#));
        assert_eq!(from_vdf(vdf, 20), None);
        let shared = "\"apps\"\n{\n\t\"1091500\"\n\t{\n\t\t\"cloudenabled\"\t\t\"0\"\n\t}\n}\n";
        assert_eq!(app_value(shared, 1091500, "cloudenabled").as_deref(), Some("0"));

        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("bin/x64")).unwrap();
        fs::write(dir.path().join("bin/x64/winmm.dll"), b"x").unwrap();
        fs::write(dir.path().join("bin/x64/Version.dll"), b"x").unwrap();
        fs::write(dir.path().join("bin/x64/game.dll"), b"x").unwrap();
        assert_eq!(proxy_dlls(dir.path()), vec!["version", "winmm"]);
    }
}
