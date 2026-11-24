//! A game's Proton prefix (its Windows folders): installing what mods need in it (Visual C++,
//! .NET, DirectX parts) with Protontricks or winetricks, and running modding tools in it
//! (xEdit, BodySlide, Nemesis, Pandora, DynDOLOD...), so they see the profile's mods in the
//! game folder like on Windows. Files a tool makes in the mod folder can be kept as a mod of
//! your own; files that were already there are never touched.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// What can be installed in a prefix: (winetricks verb, what it is, which mods want it).
pub const VERBS: &[(&str, &str, &str)] = &[
    ("vcrun2022", "Visual C++ 2015–2022", "Script extender plugins, RED4ext, Cyber Engine Tweaks, most DLL mods"),
    ("vcrun2013", "Visual C++ 2013", "Older tools (Wrye Bash, some patchers)"),
    ("vcrun2012", "Visual C++ 2012", "Older tools"),
    ("vcrun2010", "Visual C++ 2010", "Older games' mods"),
    ("dotnet48", ".NET Framework 4.8", "Many modding tools (Nemesis, zEdit, older installers)"),
    ("dotnetdesktop6", ".NET 6 Desktop", "Newer tools (Synthesis, some patchers)"),
    ("dotnetdesktop8", ".NET 8 Desktop", "The newest tools"),
    ("d3dcompiler_47", "DirectX shader compiler", "ReShade, ENB, some UI mods"),
    ("d3dx9", "DirectX 9 extras", "Older games and their mods"),
    ("xact", "XAudio (XACT)", "Sound mods in older games"),
    ("corefonts", "Windows fonts", "Tools whose text shows as boxes"),
];

pub fn verb_ok(v: &str) -> bool {
    VERBS.iter().any(|(n, ..)| *n == v)
}

/// What a loader error or a tool's message says is missing, as verbs.
pub fn suggest(text: &str) -> Vec<&'static str> {
    let t = text.to_lowercase();
    let mut out = vec![];
    let mut add = |v: &'static str| {
        if !out.contains(&v) {
            out.push(v)
        }
    };
    for (needle, verb) in [
        ("msvcp140", "vcrun2022"),
        ("vcruntime140", "vcrun2022"),
        ("msvcp120", "vcrun2013"),
        ("msvcr120", "vcrun2013"),
        ("msvcp110", "vcrun2012"),
        ("msvcp100", "vcrun2010"),
        ("msvcr100", "vcrun2010"),
        ("mscoree", "dotnet48"),
        (".net framework", "dotnet48"),
        ("microsoft.windowsdesktop.app", "dotnetdesktop6"),
        ("hostfxr", "dotnetdesktop6"),
        ("d3dcompiler_47", "d3dcompiler_47"),
        ("d3dx9_", "d3dx9"),
        ("xactengine", "xact"),
        ("x3daudio", "xact"),
    ] {
        if t.contains(needle) {
            add(verb);
        }
    }
    out
}

/// The way to reach a prefix.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Via {
    /// Protontricks installed on the computer.
    Protontricks,
    /// Protontricks from Flathub.
    ProtontricksFlatpak,
    /// winetricks with WINEPREFIX (games that aren't in Steam).
    Winetricks,
}

fn works(c: &mut Command) -> bool {
    c.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

pub fn protontricks() -> Option<Via> {
    if works(crate::host::command("protontricks").arg("--version")) {
        return Some(Via::Protontricks);
    }
    if works(crate::host::command("flatpak").args(["info", "com.github.Matoking.protontricks"])) {
        return Some(Via::ProtontricksFlatpak);
    }
    None
}

pub fn winetricks() -> bool {
    works(crate::host::command("winetricks").arg("--version"))
}

/// The way to reach this game's prefix: Protontricks for Steam games, winetricks for others.
pub fn via(steam_app: Option<u32>) -> Option<Via> {
    match steam_app {
        Some(_) => protontricks(),
        None => winetricks().then_some(Via::Winetricks),
    }
}

/// What winetricks has put in the prefix already (its own log).
pub fn installed(prefix: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_to_string(prefix.join("winetricks.log")).unwrap_or_default().lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty() && !l.starts_with('-')).collect();
    out.sort();
    out.dedup();
    out
}

/// The command that installs `verb` in the prefix (no window: -q).
pub fn install_cmd(via: &Via, app: Option<u32>, prefix: &Path, verb: &str) -> Result<Command, String> {
    if !verb_ok(verb) {
        return Err("Zom's Modzilla doesn't know that one.".into());
    }
    Ok(match via {
        Via::Protontricks => {
            let mut c = crate::host::command("protontricks");
            c.arg(app.ok_or("Protontricks is for Steam games.")?.to_string()).args(["-q", verb]);
            c
        }
        Via::ProtontricksFlatpak => {
            let mut c = crate::host::command("flatpak");
            c.args(["run", "com.github.Matoking.protontricks"]).arg(app.ok_or("Protontricks is for Steam games.")?.to_string()).args(["-q", verb]);
            c
        }
        Via::Winetricks => {
            let mut c = crate::host::command("env");
            c.arg(format!("WINEPREFIX={}", prefix.display())).args(["winetricks", "-q", verb]);
            c
        }
    })
}

/// The command that runs a Windows program in the game's prefix, with the game's Proton.
pub fn launch_cmd(via: &Via, app: Option<u32>, prefix: &Path, exe: &Path, args: &[String]) -> Result<Command, String> {
    let mut c = match via {
        Via::Protontricks => {
            let mut c = crate::host::command("protontricks-launch");
            c.args(["--appid", &app.ok_or("This is for Steam games.")?.to_string()]);
            c
        }
        Via::ProtontricksFlatpak => {
            let mut c = crate::host::command("flatpak");
            c.args(["run", "--command=protontricks-launch", "com.github.Matoking.protontricks", "--appid", &app.ok_or("This is for Steam games.")?.to_string()]);
            c
        }
        Via::Winetricks => {
            let mut c = crate::host::command("env");
            c.arg(format!("WINEPREFIX={}", prefix.display())).arg("wine");
            c
        }
    };
    c.arg(exe).args(args);
    if let Some(dir) = exe.parent() {
        c.current_dir(dir);
    }
    Ok(c)
}

/// A modding tool you run in a game's prefix.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub id: String,
    pub game: String,
    pub name: String,
    /// The tool's .exe on this computer.
    pub exe: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// Well-known tools, for their names and arguments: (exe file name, name, games, arguments).
/// xEdit takes its game from its file name, or from -<game>.
pub const KNOWN_TOOLS: &[(&str, &str, &[&str], &[&str])] = &[
    ("sseedit.exe", "SSEEdit", &["skyrimspecialedition"], &[]),
    ("fo4edit.exe", "FO4Edit", &["fallout4"], &[]),
    ("fnvedit.exe", "FNVEdit", &["newvegas"], &[]),
    ("tes5edit.exe", "TES5Edit", &["skyrim"], &[]),
    ("xedit.exe", "xEdit", &["skyrimspecialedition"], &["-SSE"]),
    ("xedit64.exe", "xEdit", &["skyrimspecialedition"], &["-SSE"]),
    ("bodyslide x64.exe", "BodySlide", &["skyrimspecialedition", "fallout4"], &[]),
    ("bodyslide.exe", "BodySlide", &["skyrim", "skyrimspecialedition"], &[]),
    ("nemesis unlimited behavior engine.exe", "Nemesis", &["skyrimspecialedition"], &[]),
    ("pandora behaviour engine+.exe", "Pandora", &["skyrimspecialedition"], &[]),
    ("dyndolodx64.exe", "DynDOLOD", &["skyrimspecialedition"], &["-sse"]),
    ("texgenx64.exe", "TexGen", &["skyrimspecialedition"], &["-sse"]),
    ("wrye bash.exe", "Wrye Bash", &["skyrimspecialedition", "fallout4", "oblivion"], &[]),
    ("cao.exe", "Cathedral Assets Optimizer", &["skyrimspecialedition", "fallout4"], &[]),
];

pub fn known_tool(exe: &Path) -> Option<(&'static str, Vec<String>)> {
    let name = exe.file_name()?.to_string_lossy().to_lowercase();
    KNOWN_TOOLS.iter().find(|(f, ..)| *f == name).map(|(_, n, _, a)| (*n, a.iter().map(|s| s.to_string()).collect()))
}

fn tools_file(data: &Path) -> PathBuf {
    data.join("tools.json")
}

pub fn tools(data: &Path) -> Vec<Tool> {
    fs::read(tools_file(data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

pub fn save_tools(data: &Path, all: &[Tool]) -> Result<(), String> {
    fs::create_dir_all(data).map_err(|e| e.to_string())?;
    crate::settings::write_atomic(&tools_file(data), &serde_json::to_vec_pretty(all).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

/// Every file under `dir`, relative to it.
pub fn files_under(dir: &Path) -> HashSet<String> {
    let mut out = HashSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            let Ok(t) = e.file_type() else { continue };
            if t.is_dir() {
                if p.file_name().is_some_and(|n| n == crate::deploy::BACKUP || n == crate::legacy::OLD_BACKUP) {
                    continue;
                }
                stack.push(p);
            } else if t.is_file() {
                if let Ok(r) = p.strip_prefix(dir) {
                    out.insert(r.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    out
}

/// What a tool made: files under the mod folder now that weren't there before it ran.
pub fn made(mod_dir: &Path, before: &HashSet<String>) -> Vec<String> {
    let mut v: Vec<String> = files_under(mod_dir).into_iter().filter(|f| !before.contains(f)).collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_from_errors_and_finds_what_a_tool_made() {
        assert_eq!(suggest("err: MSVCP140.dll was not found; also mscoree.dll"), vec!["vcrun2022", "dotnet48"]);
        assert!(suggest("all fine").is_empty());
        assert!(verb_ok("vcrun2022") && !verb_ok("vcrun2022; rm -rf ~"));
        let t = tempfile::tempdir().unwrap();
        fs::write(t.path().join("winetricks.log"), "vcrun2022\ndotnet48\nvcrun2022\n").unwrap();
        assert_eq!(installed(t.path()), vec!["dotnet48", "vcrun2022"]);
        let data = t.path().join("Data");
        fs::create_dir_all(data.join("meshes")).unwrap();
        fs::write(data.join("Skyrim.esm"), "x").unwrap();
        let before = files_under(&data);
        fs::write(data.join("Skyrim.esm"), "cleaned").unwrap();
        fs::write(data.join("meshes/made.hkx"), "new").unwrap();
        assert_eq!(made(&data, &before), vec!["meshes/made.hkx"], "files already there are never taken");
        let (name, args) = known_tool(Path::new("/x/DynDOLODx64.exe")).unwrap();
        assert_eq!((name, args), ("DynDOLOD", vec!["-sse".to_string()]));
        let c = install_cmd(&Via::Winetricks, None, Path::new("/p"), "vcrun2022").unwrap();
        assert_eq!(c.get_args().map(|a| a.to_string_lossy().into_owned()).collect::<Vec<_>>().last().unwrap(), "vcrun2022");
        assert!(install_cmd(&Via::Winetricks, None, Path::new("/p"), "bad").is_err());
    }
}
