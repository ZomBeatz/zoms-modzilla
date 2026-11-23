//! What this computer has for Zom's Nexus, since Linux distros differ: the tray library, the
//! notifier, a rar unpacker, xdg-mime for Nexus Mods links, Steam. Each optional piece is
//! found before it's used, and Settings > This computer says how to add what's missing, with
//! the command for the person's own distro.

use serde::Serialize;
use std::path::{Path, PathBuf};

/// A program on the PATH.
pub fn has_program(name: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file())).unwrap_or(false)
}

/// The appindicator library the tray icon needs, if it's installed (or inside the AppImage).
pub fn tray_library() -> Option<PathBuf> {
    let names = ["libayatana-appindicator3.so.1", "libappindicator3.so.1"];
    let mut dirs: Vec<PathBuf> = ["/usr/lib", "/usr/lib64", "/lib", "/lib64", "/usr/lib/x86_64-linux-gnu", "/lib/x86_64-linux-gnu", "/usr/lib/aarch64-linux-gnu", "/usr/local/lib"].iter().map(PathBuf::from).collect();
    if let Some(ld) = std::env::var_os("LD_LIBRARY_PATH") {
        dirs.extend(std::env::split_paths(&ld));
    }
    if let Some(app) = std::env::var_os("APPDIR") {
        let app = PathBuf::from(app);
        dirs.extend([app.join("usr/lib"), app.join("usr/lib64"), app.join("usr/lib/x86_64-linux-gnu")]);
    }
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.exists())
}

/// The distro family, from /etc/os-release: "arch", "debian", "fedora", "suse", "immutable"
/// (SteamOS, Bazzite and other read-only systems), or "other".
pub fn family(os_release: &str) -> &'static str {
    let field = |k: &str| {
        os_release
            .lines()
            .find_map(|l| l.strip_prefix(k).and_then(|v| v.strip_prefix('=')))
            .map(|v| v.trim_matches('"').to_ascii_lowercase())
            .unwrap_or_default()
    };
    let words = format!("{} {}", field("ID"), field("ID_LIKE"));
    let has = |w: &str| words.split_whitespace().any(|x| x == w);
    // Read-only systems: packages can't be added with the package manager.
    if has("steamos") || has("bazzite") || has("chimeraos") || words.contains("silverblue") || words.contains("kinoite") || has("bluefin") || has("aurora") {
        "immutable"
    } else if has("arch") || has("cachyos") || has("manjaro") || has("endeavouros") {
        "arch"
    } else if has("debian") || has("ubuntu") || has("linuxmint") || has("pop") {
        "debian"
    } else if has("fedora") || has("rhel") || has("nobara") || has("bazzite") {
        "fedora"
    } else if has("suse") || has("opensuse") || words.contains("opensuse") {
        "suse"
    } else {
        "other"
    }
}

/// The command that installs these packages (named for this family) on it.
fn install(family: &str, what: &str) -> Option<String> {
    let pkg = match (family, what) {
        ("arch", "notify") => "libnotify",
        ("arch", "rar") => "libarchive",
        ("arch", "tray") => "libayatana-appindicator",
        ("arch", "links") => "xdg-utils",
        ("debian", "notify") => "libnotify-bin",
        ("debian", "rar") => "libarchive-tools",
        ("debian", "tray") => "libayatana-appindicator3-1",
        ("debian", "links") => "xdg-utils",
        ("fedora", "notify") => "libnotify",
        ("fedora", "rar") => "bsdtar",
        ("fedora", "tray") => "libayatana-appindicator-gtk3",
        ("fedora", "links") => "xdg-utils",
        ("suse", "notify") => "libnotify-tools",
        ("suse", "rar") => "bsdtar",
        ("suse", "tray") => "libayatana-appindicator3-1",
        ("suse", "links") => "xdg-utils",
        ("arch", "keyring") => "libsecret",
        ("debian", "keyring") => "libsecret-tools",
        ("fedora", "keyring") => "libsecret",
        ("suse", "keyring") => "libsecret-tools",
        _ => return None,
    };
    Some(match family {
        "arch" => format!("sudo pacman -S {pkg}"),
        "debian" => format!("sudo apt install {pkg}"),
        "fedora" => format!("sudo dnf install {pkg}"),
        _ => format!("sudo zypper install {pkg}"),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub what: String,
    pub ok: bool,
    /// What it's for, or what's missing without it.
    pub detail: String,
    /// The command to install it, for this distro, when it's missing.
    pub fix: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub distro: String,
    pub items: Vec<Item>,
}

pub fn check(tray: bool, links: bool, steam_libraries: usize, library: &Path) -> Report {
    let os = std::fs::read_to_string("/etc/os-release").unwrap_or_default();
    let fam = family(&os);
    let distro = os.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")).map(|v| v.trim_matches('"').to_string()).unwrap_or_else(|| "Linux".into());
    let fix = |w: &str, ok: bool| if ok { None } else { install(fam, w) };
    let rar = has_program("unrar") || has_program("bsdtar");
    let notify = has_program("notify-send");
    let xdg = has_program("xdg-mime");
    let lib_ok = std::fs::create_dir_all(library).is_ok() && writable(library);
    let items = vec![
        Item {
            what: "Library folder".into(),
            ok: lib_ok,
            detail: if lib_ok { format!("Mods are kept in {}.", library.display()) } else { format!("Zom's Nexus can't write to {}. Pick another folder in Settings > Storage.", library.display()) },
            fix: None,
        },
        Item {
            what: "Steam".into(),
            ok: steam_libraries > 0,
            detail: if steam_libraries > 0 { format!("{steam_libraries} Steam library folders searched for games.") } else { "Steam wasn't found, so games are added by hand on the Games page.".into() },
            fix: None,
        },
        Item {
            what: "Nexus Mods links".into(),
            ok: xdg && links,
            detail: if !xdg { "xdg-mime is missing, so the website's Mod manager download button can't reach Zom's Nexus.".into() } else if links { "The website's Mod manager download button opens Zom's Nexus.".into() } else { "Not set up yet: Settings > Nexus Mods links > Open them with Zom's Nexus.".into() },
            fix: fix("links", xdg),
        },
        Item {
            what: "Tray icon".into(),
            ok: tray,
            detail: if tray { "Closing the window while downloading keeps it going in the tray.".into() } else { "No tray library, so closing the window while downloading minimizes it instead.".into() },
            fix: fix("tray", tray),
        },
        Item {
            what: "Notifications".into(),
            ok: notify,
            detail: if notify { "Tells you when downloads finish in the background.".into() } else { "notify-send is missing, so finished downloads aren't announced.".into() },
            fix: fix("notify", notify),
        },
        Item {
            what: "Rar files".into(),
            ok: rar,
            detail: if rar { "Mods packed as .rar can be unpacked (zip and 7z always can).".into() } else { "Mods packed as .rar can't be unpacked until bsdtar or unrar is installed (zip and 7z always can).".into() },
            fix: fix("rar", rar),
        },
        Item {
            what: "Keyring".into(),
            ok: has_program("secret-tool"),
            detail: if has_program("secret-tool") { "Your API key can be kept in the desktop keyring (Settings > Nexus Mods account).".into() } else { "secret-tool is missing, so the API key is kept in a file only you can read.".into() },
            fix: fix("keyring", has_program("secret-tool")),
        },
    ];
    let mut items = items;
    if fam == "immutable" {
        // On SteamOS and friends the system can't take packages: say what to do instead.
        for i in items.iter_mut().filter(|i| !i.ok && i.what != "Library folder" && i.what != "Steam") {
            i.detail.push_str(" This system is read-only (SteamOS, Bazzite…), so it can't be added with the package manager; it's optional.");
        }
    }
    Report { distro, items }
}

/// Whether `dir` is somewhere a program can write (the library folder, a game folder).
pub fn writable(dir: &Path) -> bool {
    let probe = dir.join(".zoms-nexus-write-test");
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_distro_families() {
        assert_eq!(family("NAME=\"CachyOS Linux\"\nID=cachyos\nID_LIKE=arch\n"), "arch");
        assert_eq!(family("ID=linuxmint\nID_LIKE=\"ubuntu debian\"\n"), "debian");
        assert_eq!(family("ID=bazzite\nID_LIKE=\"fedora\"\n"), "immutable", "Bazzite is read-only");
        assert_eq!(family("ID=nobara\nID_LIKE=\"fedora\"\n"), "fedora");
        assert_eq!(family("ID=\"opensuse-tumbleweed\"\nID_LIKE=\"opensuse suse\"\n"), "suse");
        assert_eq!(family("ID=nixos\n"), "other");
        assert_eq!(family("ID=steamos\nID_LIKE=arch\n"), "immutable");
        assert_eq!(install("immutable", "rar"), None);
        assert_eq!(install("debian", "rar").as_deref(), Some("sudo apt install libarchive-tools"));
        assert_eq!(install("other", "rar"), None);
    }
}
