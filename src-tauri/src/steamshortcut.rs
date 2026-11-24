//! Zom's Nexus as a game in Steam (a "non-Steam game"), so a Steam Deck can open it from Game
//! Mode with the controller. Steam keeps these in `userdata/<you>/config/shortcuts.vdf`, a binary
//! file it rewrites while it runs, so it's only changed while Steam is closed: the whole file is
//! read, every other entry kept exactly as it was, Zom's Nexus's own entry added or updated, and
//! the file before kept beside it (`shortcuts.vdf.zoms-nexus-backup`). Nothing else of Steam's is
//! touched apart from the artwork for the entry (grid folder, files named by its number).

use std::{
    fs,
    path::{Path, PathBuf},
};

pub const NAME: &str = "Zom's Nexus";

/// A value in Valve's binary key-value format.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Map(Vec<(String, Node)>),
    Str(String),
    Int(u32),
    /// Other kinds (64-bit numbers, floats…), kept byte for byte.
    Raw(u8, Vec<u8>),
}

fn cstr(b: &[u8], at: &mut usize) -> Result<String, String> {
    let end = b[*at..].iter().position(|&c| c == 0).ok_or("Steam's shortcuts file is cut short.")?;
    let s = String::from_utf8_lossy(&b[*at..*at + end]).into_owned();
    *at += end + 1;
    Ok(s)
}

fn map(b: &[u8], at: &mut usize) -> Result<Vec<(String, Node)>, String> {
    let mut out = vec![];
    loop {
        let kind = *b.get(*at).ok_or("Steam's shortcuts file is cut short.")?;
        *at += 1;
        if kind == 0x08 {
            return Ok(out);
        }
        let key = cstr(b, at)?;
        let v = match kind {
            0x00 => Node::Map(map(b, at)?),
            0x01 => Node::Str(cstr(b, at)?),
            0x02 => {
                let v = b.get(*at..*at + 4).ok_or("Steam's shortcuts file is cut short.")?;
                *at += 4;
                Node::Int(u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
            }
            0x03 | 0x04 | 0x06 => {
                let v = b.get(*at..*at + 4).ok_or("Steam's shortcuts file is cut short.")?.to_vec();
                *at += 4;
                Node::Raw(kind, v)
            }
            0x07 | 0x0a => {
                let v = b.get(*at..*at + 8).ok_or("Steam's shortcuts file is cut short.")?.to_vec();
                *at += 8;
                Node::Raw(kind, v)
            }
            k => return Err(format!("Steam's shortcuts file has something Zom's Nexus doesn't know (kind {k}), so it's left alone.")),
        };
        out.push((key, v));
    }
}

/// Read a whole shortcuts file: the top map.
pub fn parse(b: &[u8]) -> Result<Vec<(String, Node)>, String> {
    if b.is_empty() {
        return Ok(vec![("shortcuts".into(), Node::Map(vec![]))]);
    }
    let mut at = 0;
    let top = map(b, &mut at)?;
    Ok(top)
}

fn write_map(out: &mut Vec<u8>, m: &[(String, Node)]) {
    for (k, v) in m {
        let key = |kind: u8, out: &mut Vec<u8>| {
            out.push(kind);
            out.extend_from_slice(k.as_bytes());
            out.push(0);
        };
        match v {
            Node::Map(inner) => {
                key(0x00, out);
                write_map(out, inner);
            }
            Node::Str(s) => {
                key(0x01, out);
                out.extend_from_slice(s.as_bytes());
                out.push(0);
            }
            Node::Int(n) => {
                key(0x02, out);
                out.extend_from_slice(&n.to_le_bytes());
            }
            Node::Raw(kind, bytes) => {
                key(*kind, out);
                out.extend_from_slice(bytes);
            }
        }
    }
    out.push(0x08);
}

pub fn write(top: &[(String, Node)]) -> Vec<u8> {
    let mut out = vec![];
    write_map(&mut out, top);
    out
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// The number Steam gives a non-Steam game (its artwork files are named by it).
pub fn app_id(exe: &str, name: &str) -> u32 {
    crc32(format!("{exe}{name}").as_bytes()) | 0x8000_0000
}

fn get<'a>(m: &'a [(String, Node)], key: &str) -> Option<&'a Node> {
    m.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).map(|(_, v)| v)
}

/// Add Zom's Nexus to a shortcuts file (or update its entry). Gives the new file and the app number.
pub fn add(file: &[u8], exe: &str, start_dir: &str, icon: &str) -> Result<(Vec<u8>, u32, bool), String> {
    let mut top = parse(file)?;
    if !top.iter().any(|(k, _)| k.eq_ignore_ascii_case("shortcuts")) {
        top.push(("shortcuts".into(), Node::Map(vec![])));
    }
    let Some((_, Node::Map(list))) = top.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case("shortcuts")) else { return Err("Steam's shortcuts file isn't the shape expected, so it's left alone.".into()) };
    let quoted = format!("\"{exe}\"");
    let id = app_id(&quoted, NAME);
    // Ours: same name, or the same program.
    let ours = list.iter().position(|(_, e)| matches!(e, Node::Map(m) if matches!(get(m, "AppName"), Some(Node::Str(n)) if n == NAME) || matches!(get(m, "Exe"), Some(Node::Str(x)) if x == &quoted)));
    let entry = Node::Map(vec![
        ("appid".into(), Node::Int(id)),
        ("AppName".into(), Node::Str(NAME.into())),
        ("Exe".into(), Node::Str(quoted)),
        ("StartDir".into(), Node::Str(format!("\"{start_dir}\""))),
        ("icon".into(), Node::Str(icon.into())),
        ("ShortcutPath".into(), Node::Str(String::new())),
        ("LaunchOptions".into(), Node::Str(String::new())),
        ("IsHidden".into(), Node::Int(0)),
        ("AllowDesktopConfig".into(), Node::Int(1)),
        ("AllowOverlay".into(), Node::Int(1)),
        ("OpenVR".into(), Node::Int(0)),
        ("Devkit".into(), Node::Int(0)),
        ("DevkitGameID".into(), Node::Str(String::new())),
        ("DevkitOverrideAppID".into(), Node::Int(0)),
        ("LastPlayTime".into(), Node::Int(0)),
        ("FlatpakAppID".into(), Node::Str(String::new())),
        ("tags".into(), Node::Map(vec![("0".into(), Node::Str("Tools".into()))])),
    ]);
    let updated = match ours {
        Some(i) => {
            // Keep Steam's own notes on it (play time, its tags).
            if let (Node::Map(old), Node::Map(new)) = (&list[i].1, &entry) {
                let mut merged = new.clone();
                for (k, v) in old {
                    if matches!(k.as_str(), "LastPlayTime" | "tags" | "IsHidden") {
                        if let Some(slot) = merged.iter_mut().find(|(n, _)| n == k) {
                            slot.1 = v.clone();
                        }
                    }
                }
                list[i].1 = Node::Map(merged);
            }
            true
        }
        None => {
            let next = list.iter().filter_map(|(k, _)| k.parse::<u32>().ok()).max().map(|n| n + 1).unwrap_or(0);
            list.push((next.to_string(), entry));
            false
        }
    };
    Ok((write(&top), id, updated))
}

/// Steam's user folder in use: the one whose settings changed last.
pub fn user_dir() -> Option<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME")?);
    let mut users: Vec<(std::time::SystemTime, PathBuf)> = crate::games::steam_roots(&home)
        .into_iter()
        .flat_map(|r| fs::read_dir(r.join("userdata")).into_iter().flatten().flatten().map(|e| e.path()))
        .filter(|u| u.file_name().is_some_and(|n| n != "0" && n.to_string_lossy().chars().all(|c| c.is_ascii_digit())))
        .filter_map(|u| Some((fs::metadata(u.join("config/localconfig.vdf")).ok()?.modified().ok()?, u)))
        .collect();
    users.sort_by(|a, b| b.0.cmp(&a.0));
    users.into_iter().next().map(|(_, u)| u)
}

/// Steam is running (it rewrites the file when it closes).
pub fn steam_running() -> bool {
    crate::host::processes().iter().any(|p| matches!(p.comm.as_str(), "steam" | "steamwebhelper"))
}

/// The program Steam should start: the AppImage when running from one, else this program.
pub fn program() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_file()).or_else(|| std::env::current_exe().ok())
}

/// Add (or update) the entry, with artwork. Gives whether it was there already.
pub fn install(user: &Path, exe: &Path, art: &[(&str, &[u8])]) -> Result<bool, String> {
    let cfg = user.join("config");
    fs::create_dir_all(&cfg).map_err(|e| e.to_string())?;
    let file = cfg.join("shortcuts.vdf");
    let old = fs::read(&file).unwrap_or_default();
    let exe_s = exe.to_string_lossy().into_owned();
    let dir = exe.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let grid = cfg.join("grid");
    let id = app_id(&format!("\"{exe_s}\""), NAME);
    let icon = grid.join(format!("{id}_icon.png"));
    let (new, id2, was) = add(&old, &exe_s, &dir, &icon.to_string_lossy())?;
    debug_assert_eq!(id, id2);
    // Sanity: what's written reads back, with every entry that was there.
    let back = parse(&new)?;
    let count = |t: &[(String, Node)]| match get(t, "shortcuts") {
        Some(Node::Map(m)) => m.len(),
        _ => 0,
    };
    if count(&back) < count(&parse(&old)?) {
        return Err("Adding the entry would have lost others, so Steam's file is left as it was.".into());
    }
    if !old.is_empty() {
        let keep = cfg.join("shortcuts.vdf.zoms-nexus-backup");
        fs::write(&keep, &old).map_err(|e| format!("Couldn't keep a copy of Steam's file first: {e}"))?;
    }
    crate::settings::write_atomic(&file, &new)?;
    fs::create_dir_all(&grid).map_err(|e| e.to_string())?;
    for (suffix, bytes) in art {
        let _ = fs::write(grid.join(format!("{id}{suffix}")), bytes);
    }
    Ok(was)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_itself_and_keeps_everything_else() {
        // A file with someone else's game, a 64-bit value and tags.
        let other = vec![("shortcuts".to_string(), Node::Map(vec![("0".into(), Node::Map(vec![("appid".into(), Node::Int(123)), ("AppName".into(), Node::Str("Other".into())), ("Exe".into(), Node::Str("\"/g/other\"".into())), ("big".into(), Node::Raw(0x07, vec![1, 2, 3, 4, 5, 6, 7, 8])), ("tags".into(), Node::Map(vec![]))]))]))];
        let bytes = write(&other);
        assert_eq!(parse(&bytes).unwrap(), other, "reads back exactly");
        let (new, id, was) = add(&bytes, "/apps/Zoms-Nexus.AppImage", "/apps", "/grid/x_icon.png").unwrap();
        assert!(!was);
        assert!(id & 0x8000_0000 != 0);
        let top = parse(&new).unwrap();
        let Some(Node::Map(list)) = get(&top, "shortcuts") else { panic!() };
        assert_eq!(list.len(), 2);
        assert_eq!(list[0], other[0].1.clone().as_map_first());
        assert_eq!(list[1].0, "1");
        // Again: updated, not doubled.
        let (again, id2, was2) = add(&new, "/apps/Zoms-Nexus.AppImage", "/apps", "/grid/x_icon.png").unwrap();
        assert!(was2 && id2 == id);
        let Some(Node::Map(list)) = get(&parse(&again).unwrap(), "shortcuts").cloned() else { panic!() };
        assert_eq!(list.len(), 2);
        // An empty or missing file works too.
        assert!(add(&[], "/a", "/", "").is_ok());
        assert!(parse(&[0x00, b's']).is_err());
    }

    impl Node {
        fn as_map_first(self) -> (String, Node) {
            match self {
                Node::Map(mut m) => m.remove(0),
                _ => panic!(),
            }
        }
    }

    #[test]
    fn installs_with_a_backup_and_artwork() {
        let t = tempfile::tempdir().unwrap();
        let user = t.path().join("userdata/123");
        fs::create_dir_all(user.join("config")).unwrap();
        let other = write(&[("shortcuts".to_string(), Node::Map(vec![("0".into(), Node::Map(vec![("AppName".into(), Node::Str("Other".into()))]))]))]);
        fs::write(user.join("config/shortcuts.vdf"), &other).unwrap();
        let exe = t.path().join("Zoms-Nexus.AppImage");
        let was = install(&user, &exe, &[("p.png", b"art")]).unwrap();
        assert!(!was);
        assert_eq!(fs::read(user.join("config/shortcuts.vdf.zoms-nexus-backup")).unwrap(), other, "the old file kept");
        let id = app_id(&format!("\"{}\"", exe.display()), NAME);
        assert_eq!(fs::read(user.join(format!("config/grid/{id}p.png"))).unwrap(), b"art");
        let top = parse(&fs::read(user.join("config/shortcuts.vdf")).unwrap()).unwrap();
        let Some(Node::Map(list)) = get(&top, "shortcuts") else { panic!() };
        assert_eq!(list.len(), 2);
        assert!(install(&user, &exe, &[]).unwrap(), "second time: updated");
    }

    #[test]
    fn crc_matches_steam() {
        // CRC-32 of "123456789" is CBF43926.
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    /// `SHORTCUTS=path cargo test real_shortcuts -- --ignored`: reads a real file and checks it
    /// writes back byte for byte (only reads it).
    #[test]
    #[ignore]
    fn real_shortcuts_round_trip() {
        let p = std::env::var("SHORTCUTS").unwrap();
        let b = fs::read(p).unwrap();
        let t = parse(&b).unwrap();
        assert_eq!(write(&t), b);
        let Some(Node::Map(list)) = get(&t, "shortcuts") else { panic!() };
        println!("{} entries, round trip exact", list.len());
    }
}
