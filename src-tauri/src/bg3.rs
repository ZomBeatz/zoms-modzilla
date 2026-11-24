//! Baldur's Gate 3: its .pak mods go in the game's own Mods folder in the Proton prefix
//! (`AppData/Local/Larian Studios/Baldur's Gate 3/Mods`), and only the ones listed in
//! `PlayerProfiles/Public/modsettings.lsx` load, in that order. Zom's Nexus puts a profile's
//! .pak files there (listed in `deployed/baldursgate3-paks.json`, so taking them out removes
//! exactly those) and writes the list: the game's own base module first, anything other tools
//! put there kept, then the profile's mods in load order. Each mod's details (UUID, folder,
//! version) come from its info.json, or the meta.lsx inside its .pak.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub const GAME: &str = "baldursgate3";
const LOCAL: &str = "drive_c/users/steamuser/AppData/Local/Larian Studios/Baldur's Gate 3";

/// Files of a mod that don't go in the game folder: its .pak files (they go in the Mods folder)
/// and BG3 Mod Manager's info.json (only describes them).
pub fn not_for_game_folder(rel_lower: &str) -> bool {
    rel_lower.ends_with(".pak") || rel_lower == "info.json"
}

pub fn mods_dir(prefix: &Path) -> PathBuf {
    prefix.join(LOCAL).join("Mods")
}
pub fn settings_file(prefix: &Path) -> PathBuf {
    prefix.join(LOCAL).join("PlayerProfiles/Public/modsettings.lsx")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Module {
    pub uuid: String,
    pub folder: String,
    pub name: String,
    pub version64: String,
    pub md5: String,
}

/// The details in a meta.lsx: its ModuleInfo node's attributes.
pub fn from_meta(xml: &str) -> Option<Module> {
    let doc = roxmltree::Document::parse(xml).ok()?;
    let info = doc.descendants().find(|n| n.has_tag_name("node") && n.attribute("id") == Some("ModuleInfo"))?;
    let attr = |id: &str| info.children().find(|c| c.has_tag_name("attribute") && c.attribute("id") == Some(id)).and_then(|c| c.attribute("value")).map(String::from);
    let uuid = attr("UUID")?;
    Some(Module { folder: attr("Folder").unwrap_or_default(), name: attr("Name").unwrap_or_default(), version64: attr("Version64").or_else(|| attr("Version")).unwrap_or_else(|| "36028797018963968".into()), md5: attr("MD5").unwrap_or_default(), uuid })
}

/// The mods an info.json (BG3 Mod Manager's format) describes.
pub fn from_info_json(text: &str) -> Vec<Module> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(text) else { return vec![] };
    let md5 = v["MD5"].as_str().unwrap_or("").to_string();
    v["Mods"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let s = |k: &str| m[k].as_str().map(String::from).or_else(|| m[k].as_u64().map(|n| n.to_string()));
            Some(Module { uuid: s("UUID")?, folder: s("Folder").unwrap_or_default(), name: s("Name").unwrap_or_default(), version64: s("Version64").or_else(|| s("Version")).unwrap_or_else(|| "36028797018963968".into()), md5: md5.clone() })
        })
        .collect()
}

/// meta.lsx from inside a .pak (Larian's LSPK v15-18 archive). None when it isn't one or has none.
pub fn meta_in_pak(path: &Path) -> Option<String> {
    let mut f = fs::File::open(path).ok()?;
    let mut head = [0u8; 40];
    f.read_exact(&mut head).ok()?;
    if &head[0..4] != b"LSPK" {
        return None;
    }
    let version = u32::from_le_bytes(head[4..8].try_into().ok()?);
    if !(15..=18).contains(&version) {
        return None;
    }
    let list_at = u64::from_le_bytes(head[8..16].try_into().ok()?);
    f.seek(SeekFrom::Start(list_at)).ok()?;
    let mut nums = [0u8; 8];
    f.read_exact(&mut nums).ok()?;
    let count = u32::from_le_bytes(nums[0..4].try_into().ok()?) as usize;
    let packed = u32::from_le_bytes(nums[4..8].try_into().ok()?) as usize;
    // v18 entries are 272 bytes; v15-16 are 296.
    let entry = if version >= 18 { 272 } else { 296 };
    if count == 0 || count > 1_000_000 || packed > 64 * 1024 * 1024 {
        return None;
    }
    let mut buf = vec![0u8; packed];
    f.read_exact(&mut buf).ok()?;
    let list = lz4_flex::block::decompress(&buf, count * entry).ok()?;
    for e in list.chunks_exact(entry) {
        let name_end = e[..256].iter().position(|&b| b == 0).unwrap_or(256);
        let name = String::from_utf8_lossy(&e[..name_end]);
        if !name.to_lowercase().ends_with("/meta.lsx") || !name.to_lowercase().starts_with("mods/") {
            continue;
        }
        let r = &e[256..];
        let (offset, flags, on_disk, size) = if version >= 18 {
            let lo = u32::from_le_bytes(r[0..4].try_into().ok()?) as u64;
            let hi = u16::from_le_bytes(r[4..6].try_into().ok()?) as u64;
            (lo | (hi << 32), r[7], u32::from_le_bytes(r[8..12].try_into().ok()?) as usize, u32::from_le_bytes(r[12..16].try_into().ok()?) as usize)
        } else {
            (u64::from_le_bytes(r[0..8].try_into().ok()?), r[16], u64::from_le_bytes(r[8..16].try_into().ok()?) as usize, u64::from_le_bytes(r[24..32].try_into().ok()?) as usize)
        };
        if on_disk > 16 * 1024 * 1024 || size > 16 * 1024 * 1024 {
            return None;
        }
        f.seek(SeekFrom::Start(offset)).ok()?;
        let mut data = vec![0u8; on_disk];
        f.read_exact(&mut data).ok()?;
        let plain = match flags & 0x0f {
            0 => data,
            1 => {
                let mut out = vec![];
                flate2::read::ZlibDecoder::new(&data[..]).take(size as u64).read_to_end(&mut out).ok()?;
                out
            }
            2 => lz4_flex::block::decompress(&data, size).ok()?,
            _ => return None,
        };
        return Some(String::from_utf8_lossy(&plain).into_owned());
    }
    None
}

/// What's placed: the .pak files Zom's Nexus put in the Mods folder, and the UUIDs it listed.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Placed {
    pub files: Vec<String>,
    pub uuids: Vec<String>,
}

fn placed_path(data: &Path) -> PathBuf {
    data.join("deployed").join("baldursgate3-paks.json")
}
pub fn placed(data: &Path) -> Placed {
    fs::read(placed_path(data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('>', "&gt;")
}

fn short_desc(m: &Module, ind: &str) -> String {
    format!(
        "{ind}<node id=\"ModuleShortDesc\">\n{ind}  <attribute id=\"Folder\" type=\"LSString\" value=\"{}\"/>\n{ind}  <attribute id=\"MD5\" type=\"LSString\" value=\"{}\"/>\n{ind}  <attribute id=\"Name\" type=\"LSString\" value=\"{}\"/>\n{ind}  <attribute id=\"PublishHandle\" type=\"uint64\" value=\"0\"/>\n{ind}  <attribute id=\"UUID\" type=\"guid\" value=\"{}\"/>\n{ind}  <attribute id=\"Version64\" type=\"int64\" value=\"{}\"/>\n{ind}</node>\n",
        esc(&m.folder),
        esc(&m.md5),
        esc(&m.name),
        esc(&m.uuid),
        esc(&m.version64)
    )
}

/// The modules listed in a modsettings.lsx now, in order.
pub fn listed(xml: &str) -> Vec<Module> {
    let Ok(doc) = roxmltree::Document::parse(xml) else { return vec![] };
    let Some(mods) = doc.descendants().find(|n| n.has_tag_name("node") && n.attribute("id") == Some("Mods")) else { return vec![] };
    mods.descendants()
        .filter(|n| n.has_tag_name("node") && n.attribute("id") == Some("ModuleShortDesc"))
        .filter_map(|n| {
            let a = |id: &str| n.children().find(|c| c.has_tag_name("attribute") && c.attribute("id") == Some(id)).and_then(|c| c.attribute("value")).map(String::from);
            Some(Module { uuid: a("UUID")?, folder: a("Folder").unwrap_or_default(), name: a("Name").unwrap_or_default(), version64: a("Version64").unwrap_or_default(), md5: a("MD5").unwrap_or_default() })
        })
        .collect()
}

/// A modsettings.lsx with `ours` (in order) after what's there, and `drop` (our old ones) out.
/// The base game module keeps first place; other tools' mods keep theirs.
pub fn settings_xml(old: &str, ours: &[Module], drop: &[String]) -> String {
    let had_order = old.contains("id=\"ModOrder\"");
    let mut list: Vec<Module> = listed(old).into_iter().filter(|m| !drop.iter().any(|d| d.eq_ignore_ascii_case(&m.uuid)) && !ours.iter().any(|o| o.uuid.eq_ignore_ascii_case(&m.uuid))).collect();
    if !list.iter().any(|m| m.folder.starts_with("Gustav")) {
        list.insert(0, Module { uuid: "cb555efe-2d9e-131f-8195-a89329d218ea".into(), folder: "GustavX".into(), name: "GustavX".into(), version64: "36028797018963968".into(), md5: String::new() });
    }
    list.extend(ours.iter().cloned());
    let version = old.lines().find(|l| l.trim_start().starts_with("<version ")).map(|l| l.trim().to_string()).unwrap_or_else(|| "<version major=\"4\" minor=\"8\" revision=\"0\" build=\"500\"/>".into());
    let mut x = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<save>\n");
    x.push_str(&format!("  {version}\n  <region id=\"ModuleSettings\">\n    <node id=\"root\">\n      <children>\n"));
    if had_order {
        x.push_str("        <node id=\"ModOrder\">\n          <children>\n");
        for m in &list {
            x.push_str(&format!("            <node id=\"Module\">\n              <attribute id=\"UUID\" type=\"FixedString\" value=\"{}\"/>\n            </node>\n", esc(&m.uuid)));
        }
        x.push_str("          </children>\n        </node>\n");
    }
    x.push_str("        <node id=\"Mods\">\n          <children>\n");
    for m in &list {
        x.push_str(&short_desc(m, "            "));
    }
    x.push_str("          </children>\n        </node>\n      </children>\n    </node>\n  </region>\n</save>\n");
    x
}

/// A mod's .pak files and their details, from its unpacked folder.
pub fn paks_in(dir: &Path) -> Vec<(PathBuf, Option<Module>)> {
    let mut paks = vec![];
    let mut info: Vec<Module> = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("pak")) {
                paks.push(p);
            } else if e.file_name().eq_ignore_ascii_case("info.json") {
                info.extend(from_info_json(&fs::read_to_string(&p).unwrap_or_default()));
            }
        }
    }
    paks.sort();
    paks.into_iter()
        .map(|p| {
            let stem = p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            let m = meta_in_pak(&p).and_then(|x| from_meta(&x)).or_else(|| info.iter().find(|m| m.folder.to_lowercase() == stem || m.name.to_lowercase() == stem).cloned()).or_else(|| (info.len() == 1).then(|| info[0].clone()));
            (p, m)
        })
        .collect()
}

/// Take Zom's Nexus's .pak files out of the Mods folder and its mods off the list.
pub fn clear(data: &Path, prefix: &Path) -> Result<(), String> {
    let was = placed(data);
    let dir = mods_dir(prefix);
    for f in &was.files {
        if !f.contains(['/', '\\']) {
            let _ = fs::remove_file(dir.join(f));
        }
    }
    let file = settings_file(prefix);
    if !was.uuids.is_empty() {
        if let Ok(old) = fs::read_to_string(&file) {
            crate::settings::write_atomic(&file, settings_xml(&old, &[], &was.uuids).as_bytes())?;
        }
    }
    let _ = fs::remove_file(placed_path(data));
    Ok(())
}

/// Put a profile's .pak files in (in order) and list them. Gives (mods listed, ones without details).
pub fn apply(data: &Path, prefix: &Path, mod_dirs: &[PathBuf], copy: bool) -> Result<(usize, Vec<String>), String> {
    clear(data, prefix)?;
    let dir = mods_dir(prefix);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut now = Placed::default();
    let mut ours: Vec<Module> = vec![];
    let mut unknown = vec![];
    for d in mod_dirs {
        for (pak, m) in paks_in(d) {
            let name = pak.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let to = dir.join(&name);
            if to.exists() {
                if now.files.contains(&name) {
                    let _ = fs::remove_file(&to); // A later mod's copy wins.
                } else {
                    continue; // Someone else's file: never replaced.
                }
            }
            if copy || fs::hard_link(&pak, &to).is_err() {
                fs::copy(&pak, &to).map_err(|e| format!("Couldn't put {name} in Baldur's Gate 3's Mods folder: {e}"))?;
            }
            if !now.files.contains(&name) {
                now.files.push(name.clone());
            }
            match m {
                Some(m) => {
                    ours.retain(|o| !o.uuid.eq_ignore_ascii_case(&m.uuid));
                    ours.push(m);
                }
                None => unknown.push(name),
            }
        }
    }
    now.uuids = ours.iter().map(|m| m.uuid.clone()).collect();
    let file = settings_file(prefix);
    let old = fs::read_to_string(&file).unwrap_or_default();
    // Keep the person's own list once, the first time.
    let keep = file.with_extension("lsx.zoms-nexus-backup");
    if !old.is_empty() && !keep.exists() {
        let _ = fs::write(&keep, &old);
    }
    if let Some(p) = file.parent() {
        fs::create_dir_all(p).map_err(|e| e.to_string())?;
    }
    crate::settings::write_atomic(&file, settings_xml(&old, &ours, &[]).as_bytes())?;
    let f = placed_path(data);
    fs::create_dir_all(f.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(f, serde_json::to_vec_pretty(&now).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok((ours.len(), unknown))
}

#[cfg(test)]
mod tests {
    use super::*;

    const META: &str = r#"<?xml version="1.0" encoding="UTF-8"?><save><region id="Config"><node id="root"><children><node id="ModuleInfo">
        <attribute id="Folder" type="LSString" value="CoolMod"/><attribute id="Name" type="LSString" value="Cool Mod"/>
        <attribute id="UUID" type="FixedString" value="aaaa-1111"/><attribute id="Version64" type="int64" value="36028797018963970"/>
        </node></children></node></region></save>"#;

    /// A minimal LSPK v18 archive holding one file.
    fn pak(path: &Path, inner: &str, body: &[u8], lz4: bool) {
        let data = if lz4 { lz4_flex::block::compress(body) } else { body.to_vec() };
        let mut entry = vec![0u8; 272];
        entry[..inner.len()].copy_from_slice(inner.as_bytes());
        let offset = 40u32;
        entry[256..260].copy_from_slice(&offset.to_le_bytes());
        entry[263] = if lz4 { 2 } else { 0 };
        entry[264..268].copy_from_slice(&(data.len() as u32).to_le_bytes());
        entry[268..272].copy_from_slice(&(body.len() as u32).to_le_bytes());
        let list = lz4_flex::block::compress(&entry);
        let list_at = 40 + data.len() as u64;
        let mut f = b"LSPK".to_vec();
        f.extend_from_slice(&18u32.to_le_bytes());
        f.extend_from_slice(&list_at.to_le_bytes());
        f.extend_from_slice(&((list.len() + 8) as u32).to_le_bytes());
        f.resize(40, 0);
        f.extend_from_slice(&data);
        f.extend_from_slice(&1u32.to_le_bytes());
        f.extend_from_slice(&(list.len() as u32).to_le_bytes());
        f.extend_from_slice(&list);
        fs::write(path, f).unwrap();
    }

    #[test]
    fn reads_paks_and_writes_the_mod_list() {
        let t = tempfile::tempdir().unwrap();
        let p = t.path().join("CoolMod.pak");
        pak(&p, "Mods/CoolMod/meta.lsx", META.as_bytes(), true);
        let m = from_meta(&meta_in_pak(&p).unwrap()).unwrap();
        assert_eq!((m.uuid.as_str(), m.folder.as_str(), m.version64.as_str()), ("aaaa-1111", "CoolMod", "36028797018963970"));
        pak(&t.path().join("plain.pak"), "Mods/Plain/meta.lsx", META.as_bytes(), false);
        assert!(meta_in_pak(&t.path().join("plain.pak")).is_some());
        assert!(meta_in_pak(&t.path().join("nope.pak")).is_none());

        let info = from_info_json(r#"{"Mods":[{"Name":"Other","Folder":"Other","UUID":"bbbb-2222","Version64":"1"}],"MD5":"abc"}"#);
        assert_eq!(info[0].md5, "abc");

        // A list with the base game and someone else's mod; ours go after.
        let old = settings_xml("", &[Module { uuid: "zzzz".into(), folder: "TheirMod".into(), ..Default::default() }], &[]);
        let x = settings_xml(&old, &[m.clone(), info[0].clone()], &[]);
        let names: Vec<String> = listed(&x).into_iter().map(|m| m.folder).collect();
        assert_eq!(names, vec!["GustavX", "TheirMod", "CoolMod", "Other"]);
        // Taking ours out leaves the rest.
        let y = settings_xml(&x, &[], &["AAAA-1111".into(), "bbbb-2222".into()]);
        assert_eq!(listed(&y).into_iter().map(|m| m.folder).collect::<Vec<_>>(), vec!["GustavX", "TheirMod"]);
        assert!(!x.contains("ModOrder"));
        assert!(settings_xml("<save><node id=\"ModOrder\"/></save>", &[m], &[]).contains("ModOrder"));
    }

    #[test]
    fn puts_paks_in_and_takes_them_out() {
        let t = tempfile::tempdir().unwrap();
        let (data, prefix, lib) = (t.path().join("data"), t.path().join("pfx"), t.path().join("lib"));
        fs::create_dir_all(lib.join("m1")).unwrap();
        pak(&lib.join("m1/CoolMod.pak"), "Mods/CoolMod/meta.lsx", META.as_bytes(), true);
        fs::create_dir_all(mods_dir(&prefix)).unwrap();
        fs::write(mods_dir(&prefix).join("Theirs.pak"), "x").unwrap();
        let (n, unknown) = apply(&data, &prefix, &[lib.join("m1")], false).unwrap();
        assert_eq!((n, unknown.len()), (1, 0));
        assert!(mods_dir(&prefix).join("CoolMod.pak").is_file());
        assert!(fs::read_to_string(settings_file(&prefix)).unwrap().contains("aaaa-1111"));
        clear(&data, &prefix).unwrap();
        assert!(!mods_dir(&prefix).join("CoolMod.pak").exists());
        assert!(mods_dir(&prefix).join("Theirs.pak").exists(), "someone else's file stays");
        assert!(!fs::read_to_string(settings_file(&prefix)).unwrap().contains("aaaa-1111"));
    }
}
