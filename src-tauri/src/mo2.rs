//! Bring a Mod Organizer 2 setup across: its mods as MO2 installed them (installer choices
//! included), in the order of one of its profiles, switched on and off the same way. Files are
//! hard-linked from MO2's folders when they're on the same drive (no extra space), copied when
//! not. MO2's folders are only read, never changed. Nothing is asked of Nexus Mods: the mod and
//! file numbers come from MO2's own notes.

use crate::deploy::ProfileMod;
use crate::library::{self, StoredFile, StoredMod};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    /// The instance folder (the one with `mods` and `profiles` in it).
    pub folder: String,
    pub profiles: Vec<String>,
    pub mods: usize,
    /// MO2's name for the game ("SkyrimSE"), when it says.
    pub game_name: Option<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Imported {
    pub mods: Vec<ProfileMod>,
    /// Brought across now (the rest were in the library already).
    pub added: usize,
    /// In MO2's list but missing from its mods folder.
    pub skipped: Vec<String>,
    pub bytes_copied: u64,
}

/// `[Section]` → key → value, for MO2's simple ini files.
fn ini(text: &str) -> HashMap<String, HashMap<String, String>> {
    let mut out: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut sec = String::new();
    for line in text.lines().map(str::trim) {
        if let Some(s) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            sec = s.to_lowercase();
        } else if let Some((k, v)) = line.split_once('=') {
            out.entry(sec.clone()).or_default().insert(k.trim().to_lowercase(), v.trim().trim_matches('"').to_string());
        }
    }
    out
}

/// The instance folder from what was picked: the folder itself, or ModOrganizer.ini in it.
fn instance(picked: &Path) -> Result<PathBuf, String> {
    let dir = if picked.is_file() { picked.parent().unwrap_or(picked).to_path_buf() } else { picked.to_path_buf() };
    for d in [Some(dir.clone()), dir.parent().map(Path::to_path_buf)].into_iter().flatten() {
        if d.join("mods").is_dir() && d.join("profiles").is_dir() {
            return Ok(d);
        }
    }
    Err("That doesn't look like a Mod Organizer 2 folder: it needs “mods” and “profiles” folders in it (where ModOrganizer.ini is).".into())
}

pub fn look(picked: &Path) -> Result<Found, String> {
    let dir = instance(picked)?;
    let mut profiles: Vec<String> = fs::read_dir(dir.join("profiles")).map_err(|e| e.to_string())?.flatten().filter(|e| e.path().join("modlist.txt").is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    profiles.sort();
    if profiles.is_empty() {
        return Err("That Mod Organizer 2 folder has no profiles with a mod list.".into());
    }
    let mods = fs::read_dir(dir.join("mods")).map(|r| r.flatten().filter(|e| e.path().is_dir()).count()).unwrap_or(0);
    let game_name = fs::read_to_string(dir.join("ModOrganizer.ini")).ok().and_then(|t| ini(&t).get("general")?.get("gamename").cloned());
    Ok(Found { folder: dir.to_string_lossy().into_owned(), profiles, mods, game_name })
}

/// A profile's mods, lowest priority first (MO2 lists the winner first; Zom's Nexus last), with
/// whether each is switched on. Separators and MO2's "unmanaged" entries (DLC) are left out.
pub fn modlist(text: &str) -> Vec<(String, bool)> {
    let mut out: Vec<(String, bool)> = text
        .lines()
        .filter_map(|l| {
            let l = l.trim_end_matches('\r');
            let (on, name) = match l.chars().next()? {
                '+' => (true, &l[1..]),
                '-' => (false, &l[1..]),
                _ => return None,
            };
            (!name.ends_with("_separator") && !name.is_empty()).then(|| (name.to_string(), on))
        })
        .collect();
    out.reverse();
    out
}

/// A safe folder name from MO2's list (never a path).
fn plain(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\']) && name != "." && name != ".."
}

fn stable(s: &str) -> u64 {
    // FNV-1a: the same name gets the same number every time.
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3)) % 900_000_000_000
}

/// Copy (or link) a tree, leaving out MO2's meta.ini at the top. Gives the bytes copied.
fn bring(from: &Path, to: &Path, link: bool) -> Result<u64, String> {
    let mut copied = 0;
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        for e in fs::read_dir(from.join(&rel)).map_err(|e| e.to_string())?.flatten() {
            let Ok(t) = e.file_type() else { continue };
            let r = rel.join(e.file_name());
            if t.is_dir() {
                fs::create_dir_all(to.join(&r)).map_err(|e| e.to_string())?;
                stack.push(r);
            } else if t.is_file() && !(rel.as_os_str().is_empty() && e.file_name().eq_ignore_ascii_case("meta.ini")) {
                let dest = to.join(&r);
                if !link || fs::hard_link(e.path(), &dest).is_err() {
                    copied += fs::copy(e.path(), &dest).map_err(|err| format!("Couldn't copy {}: {err}", e.path().display()))?;
                }
            }
        }
    }
    Ok(copied)
}

pub fn import(lib: &Path, game: &str, picked: &Path, profile: &str, now: u64) -> Result<Imported, String> {
    let dir = instance(picked)?;
    if !plain(profile) {
        return Err("Pick one of the setup's profiles.".into());
    }
    let list = modlist(&fs::read_to_string(dir.join("profiles").join(profile).join("modlist.txt")).map_err(|e| format!("Couldn't read that profile's mod list: {e}"))?);
    let gdir = library::game_dir(lib, game)?;
    fs::create_dir_all(&gdir).map_err(|e| e.to_string())?;
    let link = crate::deploy::same_drive(&dir, &gdir);
    if !link {
        let need: u64 = list.iter().filter(|(n, _)| plain(n)).map(|(n, _)| library::dir_size(&dir.join("mods").join(n))).sum();
        library::ensure_space(&gdir, need, "copying the Mod Organizer 2 mods into the library")?;
    }
    let have = library::list(lib);
    let mut out = Imported::default();
    for (name, on) in list {
        let src = dir.join("mods").join(&name);
        if !plain(&name) || !src.is_dir() {
            out.skipped.push(name);
            continue;
        }
        let meta = fs::read_to_string(src.join("meta.ini")).map(|t| ini(&t)).unwrap_or_default();
        let general = meta.get("general").cloned().unwrap_or_default();
        let files = meta.get("installedfiles").cloned().unwrap_or_default();
        let num = |v: Option<&String>| v.and_then(|s| s.parse::<u64>().ok()).filter(|n| *n > 0);
        let mod_id = num(general.get("modid")).unwrap_or(library::LOCAL_IDS + stable(&name));
        let file_id = num(files.get("1\\fileid")).unwrap_or(library::LOCAL_IDS + stable(&format!("{name}/file")));
        out.mods.push(ProfileMod { mod_id, file_id, enabled: on });
        if have.iter().any(|m| m.game == game && m.mod_id == mod_id && m.files.iter().any(|f| f.file_id == file_id)) {
            continue;
        }
        let mdir = library::mod_dir(lib, game, mod_id, &name)?;
        let folder = library::safe_name(&format!("{name} (from MO2)"));
        let built = mdir.join(&folder);
        fs::create_dir_all(&built).map_err(|e| e.to_string())?;
        out.bytes_copied += bring(&src, &built, link)?;
        // Laid out like the game's mod folder already, the way MO2 installed it.
        fs::write(built.join(crate::fomod::MARK), "").map_err(|e| e.to_string())?;
        let mut m = library::read_meta(&mdir).unwrap_or(StoredMod {
            game: game.to_string(),
            mod_id,
            name: name.clone(),
            author: None,
            version: general.get("version").cloned().filter(|v| !v.is_empty()),
            summary: None,
            picture_url: None,
            files: vec![],
            local: library::is_local(mod_id),
            adult: false,
            folder: String::new(),
            source: None,
        });
        m.files.retain(|f| f.file_id != file_id);
        m.files.push(StoredFile {
            file_id,
            name: "From Mod Organizer 2".into(),
            file_name: general.get("installationfile").map(|f| f.rsplit(['/', '\\']).next().unwrap_or(f).to_string()).unwrap_or_else(|| name.clone()),
            version: general.get("version").cloned().filter(|v| !v.is_empty()),
            category: None,
            size: library::dir_size(&built),
            downloaded_at: now,
            archive: None,
            unpacked: Some(folder.clone()),
            uploaded: None,
            verified: None,
            installed: Some(crate::fomod::Installed { folder, choices: vec![] }),
            has_installer: false,
        });
        library::write_meta(&mdir, &m)?;
        out.added += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brings_a_profile_across_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        let mo2 = tmp.path().join("MO2");
        let lib = tmp.path().join("lib");
        fs::create_dir_all(mo2.join("profiles/Default")).unwrap();
        fs::write(mo2.join("profiles/Default/modlist.txt"), "# This file was automatically generated by Mod Organizer.\r\n+Winner\r\n-Off One\r\n+Extras_separator\r\n*DLC: Dawnguard\r\n+Loser\r\n+Gone\r\n").unwrap();
        for (n, meta) in [("Winner", "[General]\nmodid=100\nversion=2.0\ninstallationFile=winner-100-2-0.7z\n[installedFiles]\n1\\modid=100\n1\\fileid=555\n"), ("Off One", "[General]\nmodid=0\n"), ("Loser", "[General]\nmodid=200\n[installedFiles]\n1\\fileid=777\n")] {
            fs::create_dir_all(mo2.join("mods").join(n).join("textures")).unwrap();
            fs::write(mo2.join("mods").join(n).join("meta.ini"), meta).unwrap();
            fs::write(mo2.join("mods").join(n).join("textures/a.dds"), n).unwrap();
        }
        assert!(look(tmp.path()).unwrap_err().contains("Mod Organizer 2"));
        assert_eq!(look(&mo2.join("profiles")).unwrap().folder, mo2.to_string_lossy(), "a folder inside it works too");
        let f = look(&mo2).unwrap();
        assert_eq!((f.profiles.clone(), f.mods), (vec!["Default".to_string()], 3));
        let r = import(&lib, "skyrimspecialedition", &mo2, "Default", 1).unwrap();
        assert_eq!(r.skipped, vec!["Gone"]);
        assert_eq!(r.added, 3);
        let ids: Vec<(u64, bool)> = r.mods.iter().map(|m| (m.mod_id, m.enabled)).collect();
        assert_eq!(ids[0], (200, true), "lowest priority first");
        assert!(library::is_local(ids[1].0) && !ids[1].1);
        assert_eq!(ids[2], (100, true));
        assert_eq!(r.mods[2].file_id, 555);
        // In the library, laid out as MO2 had it, without its meta.ini.
        let all = library::list(&lib);
        let w = all.iter().find(|m| m.mod_id == 100).unwrap();
        let dir = crate::deploy::unpacked_dir(w, &w.files[0]).unwrap();
        assert!(dir.join("textures/a.dds").is_file() && !dir.join("meta.ini").exists());
        assert_eq!(crate::deploy::entries(&dir, "Data").iter().map(|e| e.0.clone()).collect::<Vec<_>>(), vec!["Data/textures/a.dds"]);
        // Twice: nothing new.
        assert_eq!(import(&lib, "skyrimspecialedition", &mo2, "Default", 2).unwrap().added, 0);
        // MO2's folder untouched.
        assert!(mo2.join("mods/Winner/meta.ini").is_file());
    }
}
