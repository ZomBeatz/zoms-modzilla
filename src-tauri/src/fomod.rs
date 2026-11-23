//! Mod installers (FOMOD): a `fomod/ModuleConfig.xml` in a mod that asks which parts to install.
//!
//! - `read` turns the installer into steps, groups and options for the page, which asks the
//!   questions (showing a step only when its conditions hold, as options set flags).
//! - `install` builds a folder from the picked options, next to the unpacked one, with hard
//!   links (no extra space), laid out the way the game's mod folder wants it. Profiles then use
//!   that folder instead of the raw files; the picks are kept so a newer version of the file can
//!   be installed the same way.
//! - Conditions on other files ("is SkyUI installed?") are answered from the game's mod folder
//!   as it is now; game and mod manager version checks always pass.
//!
//! Every path from the installer is matched against files that really are in the mod, so an
//! installer can't reach outside it, and names are matched without case like on Windows.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

/// Marks a built folder, so placing it never skips a wrapping folder the way a raw mod does.
pub const MARK: &str = ".zoms-nexus-installed";
/// Installers bigger than this aren't read (they're a few kilobytes as a rule).
const MAX_XML: u64 = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub step: String,
    pub group: String,
    pub plugins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    /// The built folder's name in the mod's folder.
    pub folder: String,
    pub choices: Vec<Choice>,
}

/// A condition, with file and version checks already answered, so only flags are left.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", untagged)]
pub enum Cond {
    All { all: Vec<Cond> },
    Any { any: Vec<Cond> },
    Flag { flag: String, value: String },
    Fixed { fixed: bool },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pattern {
    pub when: Cond,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plugin {
    pub name: String,
    pub description: String,
    pub image: Option<String>,
    /// Flags set when it's picked.
    pub flags: Vec<(String, String)>,
    /// Required, Recommended, Optional, CouldBeUsable or NotUsable…
    pub kind: String,
    /// …unless one of these matches first.
    pub patterns: Vec<Pattern>,
    #[serde(skip)]
    files: Vec<FileRef>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub name: String,
    /// SelectExactlyOne, SelectAtMostOne, SelectAtLeastOne, SelectAll or SelectAny.
    pub kind: String,
    pub plugins: Vec<Plugin>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub name: String,
    pub visible: Option<Cond>,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone)]
struct FileRef {
    source: String,
    destination: String,
    folder: bool,
    priority: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installer {
    pub name: String,
    pub image: Option<String>,
    pub steps: Vec<Step>,
    /// The picks made last time (for this file or another file of the mod).
    pub previous: Option<Vec<Choice>>,
    /// The picks this file is installed with now, if it is.
    pub current: Option<Vec<Choice>>,
    /// `previous` are the picks of the collection's curator.
    pub from_collection: bool,
    #[serde(skip)]
    required: Vec<FileRef>,
    #[serde(skip)]
    conditional: Vec<(Cond, Vec<FileRef>)>,
    #[serde(skip)]
    root: PathBuf,
}

/// The folder holding `fomod/ModuleConfig.xml` in an unpacked mod (the mod's top folder or one
/// to three folders down), if it has an installer.
pub fn root(unpacked: &Path) -> Option<PathBuf> {
    let mut dir = unpacked.to_path_buf();
    for _ in 0..4 {
        let entries: Vec<_> = fs::read_dir(&dir).ok()?.flatten().collect();
        if let Some(f) = entries.iter().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("fomod") && e.path().is_dir()) {
            let has = fs::read_dir(f.path()).ok()?.flatten().any(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("moduleconfig.xml"));
            return has.then_some(dir);
        }
        match entries.as_slice() {
            [only] if only.path().is_dir() => dir = only.path(),
            _ => return None,
        }
    }
    None
}

fn config_path(root: &Path) -> Option<PathBuf> {
    let fomod = fs::read_dir(root).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("fomod"))?.path();
    fs::read_dir(&fomod).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("moduleconfig.xml")).map(|e| e.path())
}

/// Installer files come in UTF-8 or UTF-16 (with a byte order mark).
fn decode(b: &[u8]) -> String {
    match b {
        [0xFF, 0xFE, rest @ ..] => String::from_utf16_lossy(&rest.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes([c[0], c[1]])).collect::<Vec<_>>()),
        [0xFE, 0xFF, rest @ ..] => String::from_utf16_lossy(&rest.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>()),
        [0xEF, 0xBB, 0xBF, rest @ ..] => String::from_utf8_lossy(rest).into_owned(),
        _ => String::from_utf8_lossy(b).into_owned(),
    }
}

type Node<'a, 'i> = roxmltree::Node<'a, 'i>;

fn child<'a, 'i>(n: Node<'a, 'i>, name: &str) -> Option<Node<'a, 'i>> {
    n.children().find(|c| c.is_element() && c.tag_name().name().eq_ignore_ascii_case(name))
}
fn children<'a, 'i>(n: Node<'a, 'i>, name: &'static str) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(move |c| c.is_element() && c.tag_name().name().eq_ignore_ascii_case(name))
}
fn attr(n: Node, name: &str) -> String {
    n.attributes().find(|a| a.name().eq_ignore_ascii_case(name)).map(|a| a.value().to_string()).unwrap_or_default()
}
fn text(n: Option<Node>) -> String {
    n.map(|n| n.descendants().filter(|d| d.is_text()).filter_map(|d| d.text()).collect::<String>().trim().to_string()).unwrap_or_default()
}

/// Sort by name unless the installer asks for its own order ("Explicit").
fn ordered<T>(mut v: Vec<T>, order: &str, name: impl Fn(&T) -> String) -> Vec<T> {
    match order.to_ascii_lowercase().as_str() {
        "explicit" => v,
        "descending" => {
            v.sort_by_key(|t| std::cmp::Reverse(name(t).to_lowercase()));
            v
        }
        _ => {
            v.sort_by_key(|t| name(t).to_lowercase());
            v
        }
    }
}

/// Answers "is this file active?" for file conditions.
pub type FileState<'a> = &'a dyn Fn(&str) -> &'static str;

fn cond(n: Node, files: FileState) -> Cond {
    let tag = n.tag_name().name().to_ascii_lowercase();
    match tag.as_str() {
        "flagdependency" => Cond::Flag { flag: attr(n, "flag"), value: attr(n, "value") },
        "filedependency" => Cond::Fixed { fixed: files(&attr(n, "file")).eq_ignore_ascii_case(&attr(n, "state")) },
        "gamedependency" | "fommdependency" | "fomodependency" => Cond::Fixed { fixed: true },
        _ => {
            // "dependencies" (and "visible", "moduleDependencies"): a list joined by its operator.
            let items: Vec<Cond> = n.children().filter(|c| c.is_element()).map(|c| cond(c, files)).collect();
            if attr(n, "operator").eq_ignore_ascii_case("or") {
                Cond::Any { any: items }
            } else {
                Cond::All { all: items }
            }
        }
    }
}

fn file_refs(n: Option<Node>) -> Vec<FileRef> {
    let Some(n) = n else { return vec![] };
    n.children()
        .filter(|c| c.is_element())
        .filter_map(|c| {
            let folder = match c.tag_name().name().to_ascii_lowercase().as_str() {
                "file" => false,
                "folder" => true,
                _ => return None,
            };
            let source = attr(c, "source");
            // No destination at all means "the same place as in the mod"; an empty one means
            // the top of the mod folder.
            let destination = if c.attributes().any(|a| a.name().eq_ignore_ascii_case("destination")) { attr(c, "destination") } else { source.clone() };
            Some(FileRef { source, destination, folder, priority: attr(c, "priority").parse().unwrap_or(0) })
        })
        .collect()
}

/// Read a mod's installer. `files` answers file conditions.
pub fn read(root: &Path, files: FileState) -> Result<Installer, String> {
    let path = config_path(root).ok_or("This mod's installer (fomod/ModuleConfig.xml) is missing.")?;
    if fs::metadata(&path).map(|m| m.len()).unwrap_or(0) > MAX_XML {
        return Err("This mod's installer is too big to read.".into());
    }
    let xml = decode(&fs::read(&path).map_err(|e| e.to_string())?);
    // Some installers declare UTF-16 but are read here as text already.
    let xml = xml.trim_start_matches('\u{feff}');
    let opts = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let doc = roxmltree::Document::parse_with_options(xml, opts).map_err(|e| format!("Couldn't read this mod's installer: {e}"))?;
    let top = doc.root_element();
    let name = text(child(top, "moduleName"));
    let image = child(top, "moduleImage").map(|i| attr(i, "path")).filter(|p| !p.is_empty());

    let mut steps = vec![];
    if let Some(s) = child(top, "installSteps") {
        let list: Vec<Step> = children(s, "installStep")
            .map(|st| {
                let visible = child(st, "visible").map(|v| cond(v, files));
                let groups = child(st, "optionalFileGroups")
                    .map(|og| {
                        let gs: Vec<Group> = children(og, "group")
                            .map(|g| {
                                let plugins = child(g, "plugins")
                                    .map(|ps| {
                                        let list: Vec<Plugin> = children(ps, "plugin").map(|p| plugin(p, files)).collect();
                                        ordered(list, &attr(ps, "order"), |p| p.name.clone())
                                    })
                                    .unwrap_or_default();
                                Group { name: attr(g, "name"), kind: attr(g, "type"), plugins }
                            })
                            .collect();
                        ordered(gs, &attr(og, "order"), |g| g.name.clone())
                    })
                    .unwrap_or_default();
                Step { name: attr(st, "name"), visible, groups }
            })
            .collect();
        steps = ordered(list, &attr(s, "order"), |s| s.name.clone());
    }
    let required = file_refs(child(top, "requiredInstallFiles"));
    let conditional = child(top, "conditionalFileInstalls")
        .and_then(|c| child(c, "patterns"))
        .map(|ps| children(ps, "pattern").map(|p| (child(p, "dependencies").map(|d| cond(d, files)).unwrap_or(Cond::Fixed { fixed: true }), file_refs(child(p, "files")))).collect())
        .unwrap_or_default();
    Ok(Installer { name, image, steps, previous: None, current: None, from_collection: false, required, conditional, root: root.to_path_buf() })
}

fn plugin(p: Node, files: FileState) -> Plugin {
    let flags = child(p, "conditionFlags").map(|f| children(f, "flag").map(|x| (attr(x, "name"), text(Some(x)))).collect()).unwrap_or_default();
    let (mut kind, mut patterns) = ("Optional".to_string(), vec![]);
    if let Some(td) = child(p, "typeDescriptor") {
        if let Some(t) = child(td, "type") {
            kind = attr(t, "name");
        } else if let Some(dt) = child(td, "dependencyType") {
            kind = child(dt, "defaultType").map(|d| attr(d, "name")).unwrap_or(kind);
            patterns = child(dt, "patterns")
                .map(|ps| children(ps, "pattern").map(|pt| Pattern { when: child(pt, "dependencies").map(|d| cond(d, files)).unwrap_or(Cond::Fixed { fixed: false }), kind: child(pt, "type").map(|t| attr(t, "name")).unwrap_or_default() }).collect())
                .unwrap_or_default();
        }
    }
    Plugin {
        name: attr(p, "name"),
        description: text(child(p, "description")),
        image: child(p, "image").map(|i| attr(i, "path")).filter(|s| !s.is_empty()),
        flags,
        kind,
        patterns,
        files: file_refs(child(p, "files")),
    }
}

pub fn holds(c: &Cond, flags: &HashMap<String, String>) -> bool {
    match c {
        Cond::All { all } => all.iter().all(|c| holds(c, flags)),
        Cond::Any { any } => any.is_empty() || any.iter().any(|c| holds(c, flags)),
        Cond::Flag { flag, value } => flags.get(flag).map(String::as_str).unwrap_or("") == value,
        Cond::Fixed { fixed } => *fixed,
    }
}

/// Which plugins `choices` picks, step by step, keeping only visible steps and setting flags
/// as it goes (a later step can see an earlier one's flags).
fn picked<'a>(inst: &'a Installer, choices: &[Choice]) -> (Vec<&'a Plugin>, HashMap<String, String>) {
    let mut flags = HashMap::new();
    let mut out = vec![];
    for s in &inst.steps {
        if let Some(v) = &s.visible {
            if !holds(v, &flags) {
                continue;
            }
        }
        for g in &s.groups {
            let chosen = choices.iter().find(|c| c.step == s.name && c.group == g.name);
            for p in &g.plugins {
                let kind = p.patterns.iter().find(|pt| holds(&pt.when, &flags)).map(|pt| pt.kind.as_str()).unwrap_or(&p.kind);
                let forced = g.kind.eq_ignore_ascii_case("SelectAll") || kind.eq_ignore_ascii_case("Required");
                let usable = !kind.eq_ignore_ascii_case("NotUsable");
                if usable && (forced || chosen.is_some_and(|c| c.plugins.contains(&p.name))) {
                    out.push(p);
                }
            }
            for p in out.iter().filter(|p| g.plugins.iter().any(|q| std::ptr::eq(**p, q))) {
                for (k, v) in &p.flags {
                    flags.insert(k.clone(), v.clone());
                }
            }
        }
    }
    (out, flags)
}

/// Every file under `dir` by its lowercase path ("textures/sky.dds"), and every folder.
fn index(dir: &Path) -> (HashMap<String, PathBuf>, HashMap<String, PathBuf>) {
    let (mut files, mut dirs) = (HashMap::new(), HashMap::new());
    let mut stack = vec![(dir.to_path_buf(), String::new())];
    while let Some((p, rel)) = stack.pop() {
        let Ok(rd) = fs::read_dir(&p) else { continue };
        for e in rd.flatten() {
            let Ok(t) = e.file_type() else { continue };
            let r = if rel.is_empty() { e.file_name().to_string_lossy().to_lowercase() } else { format!("{rel}/{}", e.file_name().to_string_lossy().to_lowercase()) };
            if t.is_dir() {
                dirs.insert(r.clone(), e.path());
                stack.push((e.path(), r));
            } else if t.is_file() {
                files.insert(r, e.path());
            }
        }
    }
    (files, dirs)
}

/// "Data\\Textures\\..\\x" → "data/textures/x" without ever going above the top; None when it tries.
fn clean(p: &str) -> Option<String> {
    let mut out: Vec<&str> = vec![];
    for part in p.split(['\\', '/']) {
        match part {
            "" | "." => {}
            ".." => return None,
            x => out.push(x),
        }
    }
    Some(out.join("/"))
}

/// Build the folder for `choices` in `mod_dir`, from the installer at `inst.root`. Gives what to
/// keep in the library.
pub fn install(inst: &Installer, choices: &[Choice], mod_dir: &Path, base_name: &str) -> Result<Installed, String> {
    let (plugins, flags) = picked(inst, choices);
    let mut refs: Vec<&FileRef> = inst.required.iter().collect();
    refs.extend(plugins.iter().flat_map(|p| p.files.iter()));
    for (c, fs_) in &inst.conditional {
        if holds(c, &flags) {
            refs.extend(fs_.iter());
        }
    }
    // A higher priority wins a clash; with equal ones the later file does.
    let mut refs: Vec<(usize, &FileRef)> = refs.into_iter().enumerate().collect();
    refs.sort_by_key(|(i, r)| (r.priority, *i));

    let (files, dirs) = index(&inst.root);
    // dest (lowercase) -> (dest as written, source)
    let mut plan: HashMap<String, (String, PathBuf)> = HashMap::new();
    let mut put = |dest: String, from: PathBuf| {
        if dest.is_empty() || dest.to_lowercase().starts_with("fomod/") {
            return;
        }
        plan.insert(dest.to_lowercase(), (dest, from));
    };
    for (_, r) in refs {
        let (Some(src), Some(dst)) = (clean(&r.source), clean(&r.destination)) else { continue };
        let key = src.to_lowercase();
        if r.folder || (!files.contains_key(&key) && dirs.contains_key(&key)) {
            let prefix = if key.is_empty() { String::new() } else { format!("{key}/") };
            let mut list: Vec<(&String, &PathBuf)> = files.iter().filter(|(k, _)| k.starts_with(&prefix)).collect();
            list.sort();
            for (k, from) in list {
                // Keep the file's own spelling under the folder.
                let tail_len = k.len() - prefix.len();
                let real = from.to_string_lossy();
                let tail = real[real.len() - tail_len..].to_string();
                put(if dst.is_empty() { tail } else { format!("{dst}/{tail}") }, from.clone());
            }
        } else if let Some(from) = files.get(&key) {
            // A file whose destination is a folder keeps its name ("textures/" style).
            let dest = if r.destination.ends_with(['/', '\\']) || dst.is_empty() { format!("{}{}", if dst.is_empty() { String::new() } else { format!("{dst}/") }, from.file_name().unwrap().to_string_lossy()) } else { dst };
            put(dest, from.clone());
        }
    }

    let folder = crate::library::safe_name(&format!("{base_name} (installed)"));
    let dest = mod_dir.join(&folder);
    let tmp = mod_dir.join(format!(".{folder}.building"));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let build = || -> Result<(), String> {
        for (dest_rel, from) in plan.values() {
            let to = tmp.join(dest_rel);
            if let Some(p) = to.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            if fs::hard_link(from, &to).is_err() {
                fs::copy(from, &to).map_err(|e| format!("Couldn't copy {dest_rel}: {e}"))?;
            }
        }
        fs::write(tmp.join(MARK), b"Built by Zom's Nexus from the mod's installer choices.\n").map_err(|e| e.to_string())
    };
    if let Err(e) = build() {
        let _ = fs::remove_dir_all(&tmp);
        return Err(e);
    }
    let _ = fs::remove_dir_all(&dest);
    fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(Installed { folder, choices: choices.to_vec() })
}

/// Choices from elsewhere (a collection's curator) matched to this installer's own names, ignoring
/// spaces at either end and case. Steps, groups or options it doesn't have are dropped.
pub fn fit(inst: &Installer, choices: &[Choice]) -> Vec<Choice> {
    let same = |a: &str, b: &str| a.trim().eq_ignore_ascii_case(b.trim());
    let mut out = vec![];
    for c in choices {
        let Some(s) = inst.steps.iter().find(|s| same(&s.name, &c.step)) else { continue };
        let Some(g) = s.groups.iter().find(|g| same(&g.name, &c.group)) else { continue };
        let plugins: Vec<String> = c.plugins.iter().filter_map(|n| g.plugins.iter().find(|p| same(&p.name, n)).map(|p| p.name.clone())).collect();
        out.push(Choice { step: s.name.clone(), group: g.name.clone(), plugins });
    }
    out
}

/// Choices collections' curators made, by "game:mod:file", kept in `collection-choices.json`.
pub type Suggested = std::collections::BTreeMap<String, Vec<Choice>>;

pub fn suggested(data: &Path) -> Suggested {
    fs::read_to_string(data.join("collection-choices.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn remember(data: &Path, game: &str, found: Vec<(u64, u64, Vec<Choice>)>) -> Result<usize, String> {
    let mut all = suggested(data);
    let n = found.len();
    for (m, f, c) in found {
        all.insert(format!("{game}:{m}:{f}"), c);
    }
    fs::write(data.join("collection-choices.json"), serde_json::to_vec_pretty(&all).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(n)
}

/// An image from the installer (the plugin's picture), checked to be inside the mod. At most 4 MB.
pub fn image(root: &Path, rel: &str) -> Option<(Vec<u8>, &'static str)> {
    let key = clean(rel)?.to_lowercase();
    let (files, _) = index(root);
    let path = files.get(&key)?;
    if fs::metadata(path).ok()?.len() > 4 * 1024 * 1024 {
        return None;
    }
    let mime = match key.rsplit('.').next()? {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        _ => return None,
    };
    Some((fs::read(path).ok()?, mime))
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<config xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <moduleName>Snow Test</moduleName>
  <requiredInstallFiles><folder source="Core" destination="" /></requiredInstallFiles>
  <installSteps order="Explicit">
    <installStep name="Look">
      <optionalFileGroups order="Explicit">
        <group name="Snow" type="SelectExactlyOne">
          <plugins order="Explicit">
            <plugin name="Soft"><description>Soft snow</description><files><file source="Options\Soft\snow.dds" destination="textures\snow.dds" /></files>
              <conditionFlags><flag name="snow">soft</flag></conditionFlags><typeDescriptor><type name="Recommended"/></typeDescriptor></plugin>
            <plugin name="Hard"><description>Hard snow</description><files><file source="options/hard/SNOW.dds" destination="textures/snow.dds" /></files>
              <conditionFlags><flag name="snow">hard</flag></conditionFlags><typeDescriptor><type name="Optional"/></typeDescriptor></plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
    <installStep name="Extras for soft">
      <visible><flagDependency flag="snow" value="soft" /></visible>
      <optionalFileGroups><group name="Extra" type="SelectAny"><plugins><plugin name="Sparkle"><description/><files><file source="extra/sparkle.esp" destination="sparkle.esp"/></files><typeDescriptor><type name="Optional"/></typeDescriptor></plugin></plugins></group></optionalFileGroups>
    </installStep>
  </installSteps>
  <conditionalFileInstalls><patterns><pattern><dependencies operator="And"><flagDependency flag="snow" value="hard"/></dependencies><files><file source="extra/hard.ini" destination="hard.ini"/></files></pattern></patterns></conditionalFileInstalls>
</config>"#;

    fn setup() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let r = d.path().join("unpacked/Wrapper");
        for (p, c) in [("fomod/ModuleConfig.xml", XML), ("Core/meshes/a.nif", "a"), ("Options/Soft/snow.dds", "soft"), ("Options/Hard/snow.dds", "hard"), ("Extra/sparkle.esp", "sp"), ("Extra/hard.ini", "hi")] {
            let f = r.join(p);
            fs::create_dir_all(f.parent().unwrap()).unwrap();
            fs::write(f, c).unwrap();
        }
        d
    }

    #[test]
    fn reads_and_installs_with_flags_and_hidden_steps() {
        let d = setup();
        let root = root(&d.path().join("unpacked")).expect("finds the installer under a wrapping folder");
        let none: FileState = &|_| "Missing";
        let inst = read(&root, none).unwrap();
        assert_eq!(inst.name, "Snow Test");
        assert_eq!(inst.steps.len(), 2);
        assert_eq!(inst.steps[0].groups[0].plugins[0].kind, "Recommended");

        // Hard snow: the soft-only step is skipped even though Sparkle is named, and the
        // conditional file comes along.
        let hard = vec![Choice { step: "Look".into(), group: "Snow".into(), plugins: vec!["Hard".into()] }, Choice { step: "Extras for soft".into(), group: "Extra".into(), plugins: vec!["Sparkle".into()] }];
        let i = install(&inst, &hard, d.path(), "unpacked").unwrap();
        let out = d.path().join(&i.folder);
        assert_eq!(fs::read_to_string(out.join("textures/snow.dds")).unwrap(), "hard");
        assert!(out.join("meshes/a.nif").exists(), "required folder goes to the top");
        assert!(out.join("hard.ini").exists());
        assert!(!out.join("sparkle.esp").exists());
        assert!(out.join(MARK).exists());

        let soft = vec![Choice { step: "Look".into(), group: "Snow".into(), plugins: vec!["Soft".into()] }, Choice { step: "Extras for soft".into(), group: "Extra".into(), plugins: vec!["Sparkle".into()] }];
        let i = install(&inst, &soft, d.path(), "unpacked").unwrap();
        let out = d.path().join(&i.folder);
        assert_eq!(fs::read_to_string(out.join("textures/snow.dds")).unwrap(), "soft");
        assert!(out.join("sparkle.esp").exists());
        assert!(!out.join("hard.ini").exists(), "built again from scratch");
    }

    #[test]
    fn reads_utf16_and_refuses_paths_out() {
        let mut b = vec![0xFF, 0xFE];
        for u in XML.replace("utf-8", "utf-16").encode_utf16() {
            b.extend_from_slice(&u.to_le_bytes());
        }
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join("fomod")).unwrap();
        fs::write(d.path().join("fomod/moduleconfig.xml"), b).unwrap();
        let none: FileState = &|_| "Missing";
        assert_eq!(read(d.path(), none).unwrap().name, "Snow Test");
        assert_eq!(clean("..\\..\\etc\\passwd"), None);
        assert_eq!(clean("Data\\Textures\\x.dds").as_deref(), Some("Data/Textures/x.dds"));
    }

    #[test]
    fn fits_curator_choices_to_the_installer() {
        let d = setup();
        let root = root(&d.path().join("unpacked")).unwrap();
        let none: FileState = &|_| "Missing";
        let inst = read(&root, none).unwrap();
        let f = fit(&inst, &[Choice { step: " look".into(), group: "SNOW ".into(), plugins: vec![" Hard".into(), "Gone".into()] }, Choice { step: "Nope".into(), group: "x".into(), plugins: vec![] }]);
        assert_eq!(f, vec![Choice { step: "Look".into(), group: "Snow".into(), plugins: vec!["Hard".into()] }]);
    }

    #[test]
    fn conditions_join_and_or() {
        let mut f = HashMap::new();
        f.insert("a".to_string(), "1".to_string());
        let c = Cond::Any { any: vec![Cond::Flag { flag: "a".into(), value: "2".into() }, Cond::All { all: vec![Cond::Flag { flag: "a".into(), value: "1".into() }, Cond::Fixed { fixed: true }] }] };
        assert!(holds(&c, &f));
        assert!(holds(&Cond::Flag { flag: "unset".into(), value: "".into() }, &f));
    }
}
