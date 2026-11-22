//! Zom's Nexus: a Nexus Mods download manager for Linux.
//!
//! - `nexus`: the Nexus Mods API with your own API key.
//! - `nxm`: the nxm:// links the website's **Mod manager download** button opens.
//! - `queue`: downloads, one at a time (sequential) or several at once (concurrent).
//! - `library`: where the files are kept, and unpacking them.
//! - `games`: the Nexus Mods games Steam has installed, plus ones you add; `art` their pictures.
//! - `collections`: Nexus Mods collections, queued in one go.
//! - `deploy` and `profiles`: sets of mods per game, put into the game folder when you play,
//!   and shared as mod packs. `check` finds file conflicts and plugin problems first.
//! - `updates`: newer versions of the files in the library.
//!
//! Settings live in ~/.config/com.zombeatz.zoms-nexus, the download list in
//! ~/.local/share/com.zombeatz.zoms-nexus, and the mods themselves in the library folder you pick
//! (~/Games/Nexus Mods at first).

mod art;
mod check;
mod collections;
mod deploy;
mod games;
mod library;
mod nexus;
mod nxm;
mod profiles;
mod queue;
mod requirements;
mod settings;
mod system;
mod updates;

use nexus::{FileInfo, ModInfo, Nexus, RateLimit, User};
use queue::{Job, Queue};
use serde::Serialize;
use settings::{AddedGame, Settings};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Listener, Manager, State, WindowEvent,
};
use tauri_plugin_opener::OpenerExt;

struct App {
    config_dir: PathBuf,
    data_dir: PathBuf,
    cache_dir: PathBuf,
    settings: Arc<Mutex<Settings>>,
    nexus: Nexus,
    queue: Queue,
    user: Mutex<Option<User>>,
    updates: Mutex<Option<updates::Report>>,
    checks: Mutex<HashMap<String, check::Report>>,
    /// Profiles are changed one at a time (and never while one is being put into a game).
    profiles: tokio::sync::Mutex<()>,
    /// The "still downloading in the tray" note has been shown this run.
    told_tray: std::sync::atomic::AtomicBool,
    /// Whether there's a tray icon (not every desktop has the library for one).
    has_tray: std::sync::atomic::AtomicBool,
}

impl App {
    fn set_user(&self, user: Option<User>) {
        self.queue.set_premium(user.as_ref().map(|u| u.is_premium).unwrap_or(false));
        *self.user.lock().unwrap() = user;
    }
}

type Res<T> = Result<T, String>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overview {
    settings: Settings,
    has_key: bool,
    user: Option<User>,
    limits: RateLimit,
    version: &'static str,
    /// Free space on the library's drive, in bytes.
    free_space: Option<u64>,
    /// The size of the library's drive, in bytes.
    total_space: Option<u64>,
}

fn overview(app: &App) -> Overview {
    // Each lock is taken and let go in its own statement. Inside one struct expression the
    // guards would all stay held to the end, and locking settings twice would hang the window.
    let settings = app.settings.lock().unwrap().clone();
    let user = app.user.lock().unwrap().clone();
    let limits = app.nexus.limits.lock().unwrap().clone();
    let disk = settings::disk(&settings.library());
    Overview { settings, has_key: app.nexus.has_key(), user, limits, version: nexus::APP_VERSION, free_space: disk.map(|d| d.0), total_space: disk.map(|d| d.1) }
}

#[tauri::command]
fn get_overview(app: State<'_, App>) -> Overview {
    overview(&app)
}

#[tauri::command]
fn save_settings(app: State<'_, App>, settings: Settings) -> Res<Overview> {
    let mut s = settings.clamp();
    // Keep the games list as it is; it has its own commands.
    let before = app.settings.lock().unwrap().clone();
    s.games = before.games.clone();
    s.version = settings::VERSION;
    if s.start_with_computer != before.start_with_computer {
        set_autostart(s.start_with_computer)?;
    }
    // Move the key between the file and the keyring.
    if s.key_in_keyring != before.key_in_keyring {
        if let Some(k) = app.nexus.key() {
            settings::store_key(&app.config_dir, Some(&k), s.key_in_keyring)?;
        }
    }
    fs::create_dir_all(s.library()).map_err(|e| format!("Couldn't use {} as the library folder: {e}", s.library_dir))?;
    settings::save(&app.config_dir, &s)?;
    *app.settings.lock().unwrap() = s;
    // A change of mode or slots takes effect right away.
    app.queue.poke();
    Ok(overview(&app))
}

#[tauri::command]
async fn set_api_key(app: State<'_, App>, key: String) -> Res<Overview> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("Paste your API key from nexusmods.com > Settings > API keys.".into());
    }
    let test = Nexus::new(Some(key.clone()));
    let user = test.validate().await.map_err(|e| e.message)?;
    let keyring = app.settings.lock().unwrap().key_in_keyring;
    settings::store_key(&app.config_dir, Some(&key), keyring)?;
    app.nexus.set_key(Some(key));
    *app.nexus.limits.lock().unwrap() = test.limits.lock().unwrap().clone();
    app.set_user(Some(user));
    Ok(overview(&app))
}

#[tauri::command]
fn forget_api_key(app: State<'_, App>) -> Res<Overview> {
    settings::store_key(&app.config_dir, None, false)?;
    app.nexus.set_key(None);
    app.set_user(None);
    Ok(overview(&app))
}

/// Check the key and the account again (Premium can change).
#[tauri::command]
async fn refresh_account(app: State<'_, App>) -> Res<Overview> {
    let user = app.nexus.validate().await.map_err(|e| e.message)?;
    app.set_user(Some(user));
    Ok(overview(&app))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Lookup {
    #[serde(rename = "mod")]
    info: ModInfo,
    files: Vec<FileInfo>,
}

/// A mod page address, for picking files from it.
#[tauri::command]
async fn lookup_mod(app: State<'_, App>, address: String) -> Res<Lookup> {
    let (game, id) = nxm::parse_mod_page(&address)?;
    let info = app.nexus.mod_info(&game, id).await.map_err(|e| e.message)?;
    let files = app.nexus.files(&game, id).await.map_err(|e| e.message)?;
    Ok(Lookup { info, files })
}

#[tauri::command]
fn queue_list(app: State<'_, App>) -> Vec<Job> {
    app.queue.list()
}

#[tauri::command]
fn queue_add_link(app: State<'_, App>, link: String) -> Res<Job> {
    let l = nxm::parse_nxm(&link)?;
    Ok(app.queue.add(&l.game, l.mod_id, l.file_id, l.key, l.expires, None))
}

#[tauri::command]
async fn queue_add_files(app: State<'_, App>, address: String, file_ids: Vec<u64>) -> Res<Vec<Job>> {
    let (game, id) = nxm::parse_mod_page(&address)?;
    let info = app.nexus.mod_info(&game, id).await.map_err(|e| e.message)?;
    let files = app.nexus.files(&game, id).await.map_err(|e| e.message)?;
    Ok(file_ids
        .iter()
        .filter_map(|fid| files.iter().find(|f| f.file_id == *fid))
        .map(|f| app.queue.add(&game, id, f.file_id, None, None, Some((&info, f))))
        .collect())
}

#[tauri::command]
fn queue_pause(app: State<'_, App>, id: String) {
    app.queue.pause(&id)
}

#[tauri::command]
fn queue_resume(app: State<'_, App>, id: String) {
    app.queue.resume(&id)
}

#[tauri::command]
fn queue_remove(app: State<'_, App>, id: String) {
    app.queue.remove(&id)
}

#[tauri::command]
fn queue_pause_all(app: State<'_, App>) {
    app.queue.pause_all()
}

#[tauri::command]
fn queue_resume_all(app: State<'_, App>) {
    app.queue.resume_all()
}

#[tauri::command]
fn queue_now(app: State<'_, App>, id: String) {
    app.queue.now(&id)
}

#[tauri::command]
fn queue_sort(app: State<'_, App>, smallest_first: bool) {
    app.queue.sort_by_size(smallest_first)
}

#[tauri::command]
fn queue_retry_failed(app: State<'_, App>) {
    app.queue.retry_failed()
}

#[tauri::command]
fn queue_clear_finished(app: State<'_, App>) {
    app.queue.clear_finished()
}

#[tauri::command]
fn queue_reorder(app: State<'_, App>, id: String, to: usize) {
    app.queue.reorder(&id, to)
}

fn lib_dir(app: &App) -> PathBuf {
    app.settings.lock().unwrap().library()
}

#[tauri::command]
async fn library_list(app: State<'_, App>) -> Res<Vec<library::StoredMod>> {
    let lib = lib_dir(&app);
    tokio::task::spawn_blocking(move || library::list(&lib)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn library_unpack(app: State<'_, App>, game: String, mod_id: u64, file_id: u64) -> Res<()> {
    let lib = lib_dir(&app);
    let keep = app.settings.lock().unwrap().keep_archives;
    tokio::task::spawn_blocking(move || {
        let dir = library::find(&lib, &game, mod_id)?;
        let meta = library::read_meta(&dir).ok_or("This mod's nexus.json is missing.")?;
        let f = meta.files.iter().find(|f| f.file_id == file_id).ok_or("That file isn't in the library any more.")?;
        let archive = f.archive.as_ref().ok_or("The archive was deleted after it was unpacked. Download it again to unpack it again.")?;
        let name = library::unpack(&dir.join(library::safe_name(archive)))?;
        library::set_unpacked(&dir, file_id, Some(name), !keep)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn library_remove_file(app: State<'_, App>, game: String, mod_id: u64, file_id: u64) -> Res<()> {
    // Deleting big folders takes a moment; never on the window's own thread.
    let lib = lib_dir(&app);
    tokio::task::spawn_blocking(move || library::remove_file(&lib, &game, mod_id, file_id)).await.map_err(|e| e.to_string())?
}

/// The space each library file takes on disk (archive and unpacked folder).
#[tauri::command]
async fn library_usage(app: State<'_, App>) -> Res<Vec<library::FileUsage>> {
    let lib = lib_dir(&app);
    tokio::task::spawn_blocking(move || library::usage(&lib)).await.map_err(|e| e.to_string())
}

/// Delete a file's archive, keeping its unpacked folder. Gives the bytes freed.
#[tauri::command]
async fn library_drop_archive(app: State<'_, App>, game: String, mod_id: u64, file_id: u64) -> Res<u64> {
    let lib = lib_dir(&app);
    tokio::task::spawn_blocking(move || library::drop_archive(&lib, &game, mod_id, file_id)).await.map_err(|e| e.to_string())?
}

#[tauri::command]
async fn library_remove_mod(app: State<'_, App>, game: String, mod_id: u64) -> Res<()> {
    let lib = lib_dir(&app);
    tokio::task::spawn_blocking(move || library::remove_mod(&lib, &game, mod_id)).await.map_err(|e| e.to_string())?
}

/// Open a folder in the file manager: the library, a game's part of it, one mod, or a game's
/// install folder. Only these, never any path the page names.
/// Add a mod from a zip, 7z or rar file on this computer.
#[tauri::command]
async fn library_import(app: State<'_, App>, game: String, path: String) -> Res<library::StoredMod> {
    let lib = lib_dir(&app);
    let unpack = app.settings.lock().unwrap().extract;
    let keep = app.settings.lock().unwrap().keep_archives;
    let m = tokio::task::spawn_blocking(move || -> Res<library::StoredMod> {
        let mut m = library::import(&lib, &game, Path::new(&path), queue::now())?;
        if unpack {
            let dir = PathBuf::from(&m.folder);
            let archive = dir.join(m.files[0].archive.clone().unwrap_or_default());
            // A file that won't unpack is still kept; Play tries again and says why.
            if let Ok(name) = library::unpack(&archive) {
                library::set_unpacked(&dir, m.files[0].file_id, Some(name.clone()), !keep)?;
                m.files[0].unpacked = Some(name);
            }
        }
        Ok(m)
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(m)
}

#[tauri::command]
fn open_folder(handle: AppHandle, app: State<'_, App>, game: Option<String>, mod_id: Option<u64>, install: Option<bool>) -> Res<()> {
    let lib = lib_dir(&app);
    let path = match (game, mod_id, install.unwrap_or(false)) {
        (Some(g), _, true) => list_games(&app).into_iter().find(|x| x.domain == g).and_then(|x| x.path).map(PathBuf::from).ok_or("That game's folder isn't known.")?,
        (Some(g), Some(m), false) => library::find(&lib, &g, m)?,
        (Some(g), None, false) => {
            let d = lib.join(library::safe_name(&g));
            fs::create_dir_all(&d).map_err(|e| e.to_string())?;
            d
        }
        (None, _, _) => {
            fs::create_dir_all(&lib).map_err(|e| e.to_string())?;
            lib
        }
    };
    handle.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
}

fn list_games(app: &App) -> Vec<games::Game> {
    let mut list = games::steam_games();
    for a in &app.settings.lock().unwrap().games {
        if let Some(g) = list.iter_mut().find(|g| g.domain == a.domain) {
            if a.path.is_some() {
                g.path = a.path.clone();
            }
            if a.mod_folder.is_some() {
                g.mod_folder = a.mod_folder.clone();
            }
            g.launch = a.launch.clone();
            continue;
        }
        list.push(games::Game {
            domain: a.domain.clone(),
            name: a.name.clone(),
            path: a.path.clone(),
            source: "added".into(),
            steam_app_id: None,
            proton_prefix: None,
            mod_folder: a.mod_folder.clone().or_else(|| deploy::default_mod_folder(&a.domain).map(String::from)),
            launch: a.launch.clone(),
            active_profile: None,
            free_space: None,
            total_space: None,
            last_updated: None,
        });
    }
    for g in &mut list {
        g.active_profile = deploy::deployed(&app.data_dir, &g.domain).profile_name;
        let disk = g.path.as_deref().and_then(|p| settings::disk(Path::new(p)));
        g.free_space = disk.map(|d| d.0);
        g.total_space = disk.map(|d| d.1);
    }
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list
}

#[tauri::command]
async fn games_list(app: State<'_, App>) -> Res<Vec<games::Game>> {
    Ok(list_games(&app))
}

/// The Steam library folders searched for games.
#[tauri::command]
async fn steam_libraries() -> Res<Vec<games::SteamLibrary>> {
    tokio::task::spawn_blocking(games::steam_libraries).await.map_err(|e| e.to_string())
}

#[tauri::command]
fn games_add(app: State<'_, App>, domain: String, name: Option<String>, path: Option<String>, mod_folder: Option<String>, launch: Option<String>) -> Res<()> {
    // Accept the game's name on Nexus Mods, or any address of one of its pages.
    let d = domain.trim();
    let domain = if d.contains("nexusmods.com") {
        let u = url::Url::parse(d).map_err(|e| e.to_string())?;
        let parts: Vec<String> = u.path().trim_matches('/').split('/').map(String::from).collect();
        let parts = if parts.first().map(String::as_str) == Some("games") { &parts[1..] } else { &parts[..] };
        parts.first().cloned().unwrap_or_default()
    } else {
        d.to_string()
    }
    .to_ascii_lowercase();
    if domain.is_empty() || !domain.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Type the game's name as it's written in its Nexus Mods address, like skyrimspecialedition.".into());
    }
    let name = name.filter(|n| !n.trim().is_empty()).or_else(|| games::known_name(&domain).map(String::from)).unwrap_or_else(|| domain.clone());
    let path = path.filter(|p| !p.trim().is_empty());
    if let Some(p) = &path {
        if !PathBuf::from(p).is_dir() {
            return Err(format!("There's no folder at {p}."));
        }
    }
    let mod_folder = match mod_folder.map(|m| m.trim().trim_matches('/').to_string()) {
        Some(m) if !m.is_empty() && !Path::new(&m).components().all(|c| matches!(c, std::path::Component::Normal(_))) => {
            return Err("The mod folder has to be a folder inside the game's folder, like Data or Mods.".into())
        }
        m => m,
    };
    let launch = launch.filter(|l| !l.trim().is_empty());
    let mut s = app.settings.lock().unwrap();
    s.games.retain(|g| g.domain != domain);
    s.games.push(AddedGame { domain, name, path, mod_folder, launch });
    settings::save(&app.config_dir, &s)
}

#[tauri::command]
fn games_remove(app: State<'_, App>, domain: String) -> Res<()> {
    let mut s = app.settings.lock().unwrap();
    s.games.retain(|g| g.domain != domain);
    settings::save(&app.config_dir, &s)
}

// ---------- art, updates, browsing ----------

/// A game's cover, hero banner or logo as a data: address, or nothing when there isn't one.
#[tauri::command]
async fn game_art(app: State<'_, App>, domain: String, kind: String) -> Res<Option<String>> {
    let kind = art::Kind::parse(&kind).ok_or("There's no picture of that kind.")?;
    let steam = list_games(&app).iter().find(|g| g.domain == domain).and_then(|g| g.steam_app_id).or_else(|| games::known_app(&domain));
    Ok(art::find(&app.cache_dir, &app.nexus, &domain, steam, kind).await.map(|b| art::data_url(&b)))
}

#[tauri::command]
async fn library_check_updates(app: State<'_, App>) -> Res<updates::Report> {
    let lib = lib_dir(&app);
    let mods = tokio::task::spawn_blocking(move || library::list(&lib)).await.map_err(|e| e.to_string())?;
    // Mods from your own files aren't on Nexus Mods.
    let mods: Vec<_> = mods.into_iter().filter(|m| !m.local).collect();
    let report = updates::check(&app.nexus, mods).await;
    *app.updates.lock().unwrap() = Some(report.clone());
    Ok(report)
}

/// Updates for just the files a profile uses (one request per mod in it).
#[tauri::command]
async fn profile_updates(app: State<'_, App>, id: String) -> Res<updates::Report> {
    let p = find_profile(&app, &id)?;
    let lib = lib_dir(&app);
    let mods = tokio::task::spawn_blocking(move || library::list(&lib)).await.map_err(|e| e.to_string())?;
    let mods: Vec<library::StoredMod> = mods
        .into_iter()
        .filter(|m| !m.local && m.game == p.game)
        .filter_map(|mut m| {
            let used: Vec<u64> = p.mods.iter().filter(|x| x.mod_id == m.mod_id).map(|x| x.file_id).collect();
            m.files.retain(|f| used.contains(&f.file_id));
            (!m.files.is_empty()).then_some(m)
        })
        .collect();
    Ok(updates::check(&app.nexus, mods).await)
}

// ---------- notes ----------

fn notes_file(app: &App) -> PathBuf {
    app.config_dir.join("notes.json")
}

/// Your notes on mods, by "game:mod number".
#[tauri::command]
fn mod_notes(app: State<'_, App>) -> HashMap<String, String> {
    fs::read_to_string(notes_file(&app)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command]
fn set_mod_note(app: State<'_, App>, game: String, mod_id: u64, note: String) -> Res<()> {
    let mut all = mod_notes(app.clone());
    let key = format!("{game}:{mod_id}");
    let note = note.trim().chars().take(2000).collect::<String>();
    if note.is_empty() {
        all.remove(&key);
    } else {
        all.insert(key, note);
    }
    settings::write_atomic(&notes_file(&app), &serde_json::to_vec_pretty(&all).map_err(|e| e.to_string())?)
}

// ---------- help ----------

/// A report for getting help: versions, this computer, settings (never the API key), games,
/// profiles and the download list. Saved where the person picks.
#[tauri::command]
async fn diagnostics(app: State<'_, App>, path: String) -> Res<()> {
    let s = app.settings.lock().unwrap().clone();
    let sys = system::check(app.has_tray.load(std::sync::atomic::Ordering::Relaxed), nxm_status().ours, games::steam_library_dirs().len(), &lib_dir(&app));
    let jobs: Vec<String> = app.queue.list().iter().map(|j| format!("{:?} {} {}/{} {} {}", j.status, j.game, j.mod_id, j.file_id, j.mod_name, j.error.clone().unwrap_or_default())).collect();
    let report = serde_json::json!({
        "app": format!("Zom's Nexus {}", nexus::APP_VERSION),
        "system": sys,
        "webkit_dmabuf_off": std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").ok(),
        "account": app.user.lock().unwrap().as_ref().map(|u| if u.is_premium { "Premium" } else { "free" }),
        "settings": s,
        "games": list_games(&app),
        "profiles": profiles::load(&app.config_dir),
        "checks": app.checks.lock().unwrap().clone(),
        "downloads": jobs,
    });
    let mut path = PathBuf::from(path);
    if path.extension().is_none() {
        path.set_extension("json");
    }
    fs::write(&path, serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?).map_err(|e| format!("Couldn't save the report: {e}"))
}

#[tauri::command]
fn library_updates(app: State<'_, App>) -> Option<updates::Report> {
    app.updates.lock().unwrap().clone()
}

/// Queue one file by its numbers (an update, a file from Browse).
#[tauri::command]
fn queue_add(app: State<'_, App>, game: String, mod_id: u64, file_id: u64) -> Res<Job> {
    if game.is_empty() || !game.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("That isn't a game name Nexus Mods uses.".into());
    }
    Ok(app.queue.add(&game, mod_id, file_id, None, None, None))
}

#[tauri::command]
async fn browse(app: State<'_, App>, game: String, list: String) -> Res<Vec<ModInfo>> {
    app.nexus.browse(&game, &list).await.map_err(|e| e.message)
}

/// A mod's details and files by its numbers (from Browse).
#[tauri::command]
async fn mod_files(app: State<'_, App>, game: String, mod_id: u64) -> Res<Lookup> {
    let info = app.nexus.mod_info(&game, mod_id).await.map_err(|e| e.message)?;
    let files = app.nexus.files(&game, mod_id).await.map_err(|e| e.message)?;
    Ok(Lookup { info, files })
}

/// What mods need, as listed on Nexus Mods: every mod of one game in one request (kept for a
/// day, so asking again is free). Also names files still waiting for their mod's name.
#[tauri::command]
async fn mod_requirements(app: State<'_, App>, game: String, mod_ids: Vec<u64>) -> Res<Vec<requirements::Facts>> {
    if !app.nexus.has_key() {
        return Ok(vec![]);
    }
    let wanted: Vec<(String, u64)> = mod_ids.iter().filter(|m| !library::is_local(**m)).map(|m| (game.clone(), *m)).collect();
    let f = requirements::facts(&app.nexus, &app.data_dir, &wanted).await.map_err(|e| e.message)?;
    let found: Vec<requirements::Facts> = wanted.iter().filter_map(|(g, m)| requirements::lookup(&f, g, *m).cloned()).collect();
    for m in &found {
        app.queue.name_mod(&game, m.mod_id, &m.name);
    }
    Ok(found)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QueuedNeeds {
    queued: Vec<String>,
    /// Mods without a file to download (or that Nexus Mods couldn't find), with why.
    skipped: Vec<String>,
}

/// Queue the main file of each of these mods (what another mod needs). One request per mod,
/// for its file list; asked for by a click, never by itself.
#[tauri::command]
async fn queue_requirements(app: State<'_, App>, game: String, mod_ids: Vec<u64>) -> Res<QueuedNeeds> {
    let mut out = QueuedNeeds { queued: vec![], skipped: vec![] };
    let facts = requirements::facts(&app.nexus, &app.data_dir, &mod_ids.iter().map(|m| (game.clone(), *m)).collect::<Vec<_>>()).await.unwrap_or_default();
    for id in mod_ids.into_iter().take(50) {
        let name = requirements::lookup(&facts, &game, id).map(|f| f.name.clone()).unwrap_or_else(|| format!("Mod {id}"));
        match app.nexus.files(&game, id).await {
            Ok(files) => match files.iter().find(|f| !matches!(f.category.as_deref(), Some("ARCHIVED" | "OLD_VERSION" | "DELETED"))) {
                Some(f) => {
                    app.queue.add(&game, id, f.file_id, None, None, None);
                    app.queue.name_mod(&game, id, &name);
                    app.queue.describe_file(&game, id, f, true);
                    out.queued.push(name);
                }
                None => out.skipped.push(format!("{name} (no current file)")),
            },
            Err(e) => out.skipped.push(format!("{name} ({})", e.message)),
        }
    }
    Ok(out)
}

// ---------- collections ----------

#[tauri::command]
async fn lookup_collection(app: State<'_, App>, address: String) -> Res<collections::Collection> {
    let a = collections::parse(&address)?;
    collections::fetch(&app.nexus, &a).await.map_err(|e| e.message)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectionAdded {
    queued: usize,
    /// Files already in the library, so not downloaded again.
    skipped: usize,
    profile: deploy::Profile,
}

/// Queue every file of a collection (optional ones too, when asked) and make a profile of it.
async fn add_collection(app: &App, address: &str, optional: bool) -> Res<CollectionAdded> {
    let a = collections::parse(address)?;
    let c = collections::fetch(&app.nexus, &a).await.map_err(|e| e.message)?;
    let files: Vec<_> = c.files.iter().filter(|f| optional || !f.optional).collect();
    // Files already in the library aren't downloaded again; the profile uses them as they are.
    let lib = lib_dir(app);
    let have = tokio::task::spawn_blocking(move || library::list(&lib)).await.map_err(|e| e.to_string())?;
    let (mut queued, mut skipped) = (0, 0);
    for f in &files {
        if have.iter().any(|m| m.game == f.game && m.mod_id == f.mod_id && m.files.iter().any(|x| x.file_id == f.file_id)) {
            skipped += 1;
            continue;
        }
        app.queue.add(&f.game, f.mod_id, f.file_id, None, None, None);
        app.queue.name_mod(&f.game, f.mod_id, &f.mod_name);
        let shown = FileInfo { file_id: f.file_id, name: f.name.clone(), file_name: String::new(), version: f.version.clone(), category: None, size: f.size, uploaded: None, is_primary: false, description: None };
        app.queue.describe_file(&f.game, f.mod_id, &shown, false);
        queued += 1;
    }
    let name = match c.revision {
        Some(r) => format!("{} (revision {r})", c.name),
        None => c.name.clone(),
    };
    let profile = deploy::Profile {
        id: format!("p{}", queue::now()),
        name,
        game: c.game.clone(),
        mods: files.iter().filter(|f| f.game == c.game).map(|f| deploy::ProfileMod { mod_id: f.mod_id, file_id: f.file_id, enabled: true }).collect(),
        collection: Some(c.slug.clone()),
        manual_requirements: false,
            new_downloads: false,
    };
    let _turn = app.profiles.lock().await;
    let mut all = profiles::load(&app.config_dir);
    all.push(profile.clone());
    profiles::save(&app.config_dir, &all)?;
    Ok(CollectionAdded { queued, skipped, profile })
}

#[tauri::command]
async fn queue_add_collection(app: State<'_, App>, address: String, optional: bool) -> Res<CollectionAdded> {
    add_collection(&app, &address, optional).await
}

// ---------- profiles ----------

#[tauri::command]
async fn profiles_list(app: State<'_, App>) -> Res<Vec<deploy::Profile>> {
    Ok(profiles::load(&app.config_dir))
}

#[tauri::command]
async fn profile_save(app: State<'_, App>, mut profile: deploy::Profile) -> Res<deploy::Profile> {
    if profile.name.trim().is_empty() {
        return Err("Give the profile a name.".into());
    }
    if profile.game.is_empty() || !profile.game.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err("Pick the profile's game.".into());
    }
    let _turn = app.profiles.lock().await;
    let mut all = profiles::load(&app.config_dir);
    if profile.id.is_empty() {
        profile.id = format!("p{}{}", queue::now(), all.len());
        all.push(profile.clone());
    } else if let Some(p) = all.iter_mut().find(|p| p.id == profile.id) {
        *p = profile.clone();
    } else {
        all.push(profile.clone());
    }
    profiles::save(&app.config_dir, &all)?;
    app.checks.lock().unwrap().remove(&profile.id);
    Ok(profile)
}

#[tauri::command]
async fn profile_delete(app: State<'_, App>, id: String) -> Res<()> {
    let _turn = app.profiles.lock().await;
    let mut all = profiles::load(&app.config_dir);
    all.retain(|p| p.id != id);
    profiles::save(&app.config_dir, &all)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileResolved {
    profile: deploy::Profile,
    #[serde(flatten)]
    changes: requirements::Resolved,
}

/// Put what the profile's mods need into it, switched on and sorted above them (see
/// `requirements::resolve`). Asks Nexus Mods only about mods not asked about today, and again
/// for requirements it adds (their own requirements), at most a few rounds.
#[tauri::command]
async fn profile_resolve(app: State<'_, App>, id: String) -> Res<ProfileResolved> {
    let _turn = app.profiles.lock().await;
    let mut p = find_profile(&app, &id)?;
    if p.manual_requirements || !app.nexus.has_key() {
        return Ok(ProfileResolved { profile: p, changes: Default::default() });
    }
    let lib = lib_dir(&app);
    let library = tokio::task::spawn_blocking(move || library::list(&lib)).await.map_err(|e| e.to_string())?;
    let mut all = requirements::Resolved::default();
    for _ in 0..4 {
        let wanted: Vec<(String, u64)> = p.mods.iter().filter(|m| m.enabled && !library::is_local(m.mod_id)).map(|m| (p.game.clone(), m.mod_id)).collect();
        let facts = requirements::facts(&app.nexus, &app.data_dir, &wanted).await.map_err(|e| e.message)?;
        let r = requirements::resolve(&mut p, &library, &facts);
        let more = !r.added.is_empty() || !r.switched_on.is_empty();
        all.added.extend(r.added);
        all.switched_on.extend(r.switched_on);
        all.moved += r.moved;
        all.missing = r.missing;
        if !more {
            break;
        }
    }
    if !all.added.is_empty() || !all.switched_on.is_empty() || all.moved > 0 {
        let mut profiles = profiles::load(&app.config_dir);
        if let Some(x) = profiles.iter_mut().find(|x| x.id == p.id) {
            *x = p.clone();
        }
        profiles::save(&app.config_dir, &profiles)?;
        app.checks.lock().unwrap().remove(&p.id);
    }
    Ok(ProfileResolved { profile: p, changes: all })
}

fn find_profile(app: &App, id: &str) -> Res<deploy::Profile> {
    profiles::load(&app.config_dir).into_iter().find(|p| p.id == id).ok_or_else(|| "That profile isn't there any more.".to_string())
}

/// The game's install folder, mod folder and Proton prefix, or why it can't take mods yet.
fn game_target(app: &App, domain: &str) -> Res<(PathBuf, String, Option<PathBuf>, games::Game)> {
    let g = list_games(app).into_iter().find(|g| g.domain == domain).ok_or("Add this game on the Games page first, with its install folder.")?;
    let root = g.path.clone().map(PathBuf::from).ok_or("Set this game's install folder on the Games page first.")?;
    let folder = g.mod_folder.clone().ok_or("Zom's Nexus doesn't know where this game's mods go yet. Set its mod folder on the Games page.")?;
    let prefix = g.proton_prefix.clone().map(PathBuf::from);
    Ok((root, folder, prefix, g))
}

#[tauri::command]
async fn profile_check(app: State<'_, App>, id: String) -> Res<check::Report> {
    let p = find_profile(&app, &id)?;
    let lib = lib_dir(&app);
    let (root, folder) = match game_target(&app, &p.game) {
        Ok((r, f, _, _)) => (Some(r), f),
        Err(_) => (None, deploy::default_mod_folder(&p.game).unwrap_or("").to_string()),
    };
    let ours: HashSet<String> = deploy::deployed(&app.data_dir, &p.game).files.iter().map(|f| f.to_lowercase()).collect();
    let p2 = p.clone();
    let (mut report, mods) = tokio::task::spawn_blocking(move || {
        let mods = library::list(&lib);
        (check::check(&p2, &mods, &folder, root.as_deref(), &ours), mods)
    })
    .await
    .map_err(|e| e.to_string())?;
    // What the mods need, and whether they're current, from Nexus Mods (one request for every
    // mod not asked about today; none at all when everything was asked about already).
    if app.nexus.has_key() {
        let wanted: Vec<(String, u64)> = p.mods.iter().filter(|m| m.enabled && !library::is_local(m.mod_id)).map(|m| (p.game.clone(), m.mod_id)).collect();
        if let Ok(facts) = requirements::facts(&app.nexus, &app.data_dir, &wanted).await {
            let updated = list_games(&app).into_iter().find(|g| g.domain == p.game).and_then(|g| g.last_updated);
            check::add_requirements(&mut report, &p, &mods, &facts, updated);
        }
    }
    app.checks.lock().unwrap().insert(p.id.clone(), report.clone());
    Ok(report)
}

#[tauri::command]
fn profile_checks(app: State<'_, App>) -> HashMap<String, check::Report> {
    app.checks.lock().unwrap().clone()
}

async fn apply_profile(app: &App, id: &str) -> Res<deploy::Applied> {
    let _turn = app.profiles.lock().await;
    let p = find_profile(app, id)?;
    let (root, folder, prefix, _) = game_target(app, &p.game)?;
    let lib = lib_dir(app);
    let data = app.data_dir.clone();
    let game = p.game.clone();
    tokio::task::spawn_blocking(move || {
        let mods = library::list(&lib);
        deploy::apply(&data, &deploy::Target { game: &game, root: &root, mod_folder: &folder, prefix: prefix.as_deref() }, &p, &mods)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn profile_apply(app: State<'_, App>, id: String) -> Res<deploy::Applied> {
    apply_profile(&app, &id).await
}

/// Take a game's mods out, putting its own files back.
#[tauri::command]
async fn profile_clear(app: State<'_, App>, game: String) -> Res<()> {
    let _turn = app.profiles.lock().await;
    let (root, _, _, _) = game_target(&app, &game)?;
    let data = app.data_dir.clone();
    tokio::task::spawn_blocking(move || deploy::clear(&data, &game, &root)).await.map_err(|e| e.to_string())?
}

fn launch(handle: &AppHandle, g: &games::Game) -> Res<()> {
    if let Some(cmd) = &g.launch {
        let dir = g.path.clone().unwrap_or_else(|| ".".into());
        std::process::Command::new("sh").arg("-c").arg(cmd).current_dir(dir).spawn().map_err(|e| format!("Couldn't start the game: {e}"))?;
        return Ok(());
    }
    let app_id = g.steam_app_id.ok_or("Add a start command for this game on the Games page, or start it yourself.")?;
    handle.opener().open_url(format!("steam://rungameid/{app_id}"), None::<&str>).map_err(|e| format!("Couldn't ask Steam to start the game: {e}"))
}

#[tauri::command]
async fn game_launch(handle: AppHandle, app: State<'_, App>, domain: String) -> Res<()> {
    let g = list_games(&app).into_iter().find(|g| g.domain == domain).ok_or("That game isn't on the Games page.")?;
    launch(&handle, &g)
}

/// Put the profile's mods in, then start the game.
#[tauri::command]
async fn profile_play(handle: AppHandle, app: State<'_, App>, id: String) -> Res<deploy::Applied> {
    let applied = apply_profile(&app, &id).await?;
    let p = find_profile(&app, &id)?;
    let (_, _, _, g) = game_target(&app, &p.game)?;
    launch(&handle, &g)?;
    Ok(applied)
}

#[tauri::command]
async fn profile_export(app: State<'_, App>, id: String, path: String) -> Res<()> {
    let p = find_profile(&app, &id)?;
    let mods = library::list(&lib_dir(&app));
    // A pack lists Nexus Mods numbers; mods from your own files can't be fetched by others.
    let mut p = p;
    p.mods.retain(|m| !library::is_local(m.mod_id));
    let pack = profiles::to_pack(&p, |m, f| {
        let sm = mods.iter().find(|x| x.game == p.game && x.mod_id == m);
        (sm.map(|x| x.name.clone()).unwrap_or_default(), sm.and_then(|x| x.files.iter().find(|y| y.file_id == f)).map(|y| y.file_name.clone()).unwrap_or_default())
    });
    let mut path = PathBuf::from(path);
    if path.extension().is_none() {
        path.set_extension("json");
    }
    fs::write(&path, serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?).map_err(|e| format!("Couldn't save the mod pack: {e}"))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackImported {
    profile: deploy::Profile,
    queued: usize,
}

/// Open a mod pack: make its profile and queue whatever isn't in the library yet.
#[tauri::command]
async fn profile_import(app: State<'_, App>, path: String) -> Res<PackImported> {
    let meta = fs::metadata(&path).map_err(|e| format!("Couldn't open that file: {e}"))?;
    if meta.len() > 5_000_000 {
        return Err("That file is too big to be a mod pack.".into());
    }
    let pack = profiles::read_pack(&fs::read_to_string(&path).map_err(|e| format!("Couldn't read that file: {e}"))?)?;
    let have = library::list(&lib_dir(&app));
    let mut queued = 0;
    for m in pack.mods.iter().filter(|m| m.enabled) {
        let stored = have.iter().any(|s| s.game == pack.game && s.mod_id == m.mod_id && s.files.iter().any(|f| f.file_id == m.file_id));
        if !stored {
            app.queue.add(&pack.game, m.mod_id, m.file_id, None, None, None);
            queued += 1;
        }
    }
    let _turn = app.profiles.lock().await;
    let mut all = profiles::load(&app.config_dir);
    let profile = profiles::from_pack(&pack, format!("p{}{}", queue::now(), all.len()));
    all.push(profile.clone());
    profiles::save(&app.config_dir, &all)?;
    Ok(PackImported { profile, queued })
}

// ---------- nxm:// links ----------

const NXM_DESKTOP: &str = "com.zombeatz.zoms-nexus-nxm.desktop";

fn applications_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("applications"))
}

/// The program to start for a link: the AppImage itself when running from one.
fn program() -> Res<PathBuf> {
    if let Some(a) = std::env::var_os("APPIMAGE") {
        return Ok(PathBuf::from(a));
    }
    std::env::current_exe().map_err(|e| e.to_string())
}

/// Quote a path for an Exec= line (the desktop entry spec's rules).
fn exec_quote(p: &str) -> String {
    let mut s = String::from("\"");
    for c in p.chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                s.push('\\');
                s.push(c)
            }
            '%' => s.push_str("%%"),
            _ => s.push(c),
        }
    }
    s.push('"');
    s
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LinkStatus {
    /// Zom's Nexus opens nxm:// links.
    ours: bool,
    /// The program that opens them now, if another one does.
    other: Option<String>,
}

#[tauri::command]
fn nxm_status() -> LinkStatus {
    let out = std::process::Command::new("xdg-mime").args(["query", "default", "x-scheme-handler/nxm"]).output();
    let current = out.ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let ours = current == NXM_DESKTOP || current == "zoms-nexus.desktop" || current == "Zom's Nexus.desktop";
    LinkStatus { ours, other: (!ours && !current.is_empty()).then_some(current) }
}

/// What this computer has for Zom's Nexus, with how to add what's missing.
#[tauri::command]
async fn system_check(app: State<'_, App>) -> Res<system::Report> {
    let tray = app.has_tray.load(std::sync::atomic::Ordering::Relaxed);
    let links = nxm_status().ours;
    let steam = tokio::task::spawn_blocking(games::steam_library_dirs).await.map_err(|e| e.to_string())?.len();
    Ok(system::check(tray, links, steam, &lib_dir(&app)))
}

#[tauri::command]
fn nxm_register() -> Res<LinkStatus> {
    let dir = applications_dir().ok_or("Couldn't find your applications folder.")?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let exe = program()?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Zom's Nexus\nComment=Opens Nexus Mods download links in Zom's Nexus\nExec={} %u\nIcon=zoms-nexus\nTerminal=false\nNoDisplay=true\nMimeType=x-scheme-handler/nxm;\n",
        exec_quote(&exe.to_string_lossy())
    );
    fs::write(dir.join(NXM_DESKTOP), entry).map_err(|e| e.to_string())?;
    let _ = std::process::Command::new("update-desktop-database").arg(&dir).output();
    let ok = std::process::Command::new("xdg-mime").args(["default", NXM_DESKTOP, "x-scheme-handler/nxm"]).status().map(|s| s.success()).unwrap_or(false);
    if !ok {
        return Err("Couldn't set Zom's Nexus as the program for Nexus Mods links (xdg-mime is missing or failed).".into());
    }
    Ok(nxm_status())
}

/// A link from the website, from the command line or from a second copy of the app.
fn take_link(handle: &AppHandle, args: &[String]) {
    let Some(link) = args.iter().find(|a| a.starts_with("nxm://")) else { return };
    let app = handle.state::<App>();
    if link.contains("/collections/") {
        let (h, l) = (handle.clone(), link.clone());
        tauri::async_runtime::spawn(async move {
            match add_collection(&h.state::<App>(), &l, false).await {
                Ok(c) => {
                    let _ = h.emit("collection:added", c.profile.name.clone());
                    let _ = h.emit("profiles:changed", ());
                }
                Err(e) => {
                    let _ = h.emit("nxm:error", e);
                }
            }
        });
    } else {
    match nxm::parse_nxm(link) {
        Ok(l) => {
            let job = app.queue.add(&l.game, l.mod_id, l.file_id, l.key, l.expires, None);
            let _ = handle.emit("nxm:added", job);
            // Its name (and what it needs) now, in one request, instead of when it starts.
            let h = handle.clone();
            tauri::async_runtime::spawn(async move {
                let app = h.state::<App>();
                if let Ok(f) = requirements::facts(&app.nexus, &app.data_dir, &[(l.game.clone(), l.mod_id)]).await {
                    if let Some(m) = requirements::lookup(&f, &l.game, l.mod_id) {
                        app.queue.name_mod(&l.game, l.mod_id, &m.name);
                    }
                }
                // Its size and name: the request the download would make anyway, made now and kept.
                if let Ok(file) = app.nexus.file(&l.game, l.mod_id, l.file_id).await {
                    app.queue.describe_file(&l.game, l.mod_id, &file, true);
                }
            });
        }
        Err(e) => {
            let _ = handle.emit("nxm:error", e);
        }
    }
    }
    // While the Nexus Mods window is open, it keeps the focus (the next page opens there).
    let browsing = handle.get_webview_window(NEXUS_WINDOW).and_then(|w| w.is_visible().ok()).unwrap_or(false);
    if !browsing {
        show_main(handle);
    }
}

fn show_main(handle: &AppHandle) {
    if let Some(w) = handle.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// A desktop notification (through notify-send, when it's installed).
fn notify(title: &str, body: &str) {
    let _ = std::process::Command::new("notify-send").args(["-a", "Zom's Nexus", "-i", "zoms-nexus", title, body]).spawn();
}

/// Started with the computer: open in the background, only to carry on with downloads.
const BACKGROUND: &str = "--background";

fn config_home() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
}

/// Add or take away the entry that starts Zom's Nexus when you sign in.
fn set_autostart(on: bool) -> Res<()> {
    let dir = config_home().ok_or("Couldn't find your config folder.")?.join("autostart");
    let file = dir.join("zoms-nexus.desktop");
    if !on {
        let _ = fs::remove_file(&file);
        return Ok(());
    }
    let exe = program()?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let entry = format!(
        "[Desktop Entry]\nType=Application\nName=Zom's Nexus\nComment=Carries on with unfinished mod downloads\nExec={} {BACKGROUND}\nIcon=zoms-nexus\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
        exec_quote(&exe.to_string_lossy())
    );
    fs::write(&file, entry).map_err(|e| format!("Couldn't set Zom's Nexus to start with the computer: {e}"))
}

// ---------- the Nexus Mods window ----------

/// The app's own window onto nexusmods.com, for free accounts' Slow download clicks: one
/// window that moves from file to file, instead of a browser tab for each.
const NEXUS_WINDOW: &str = "nexus";

/// Only Nexus Mods pages are opened in it from the app.
fn nexus_page(url: &str) -> Res<url::Url> {
    let u = url::Url::parse(url).map_err(|_| "That isn't a Nexus Mods address.".to_string())?;
    let host = u.host_str().unwrap_or("");
    if u.scheme() != "https" || !(host == "nexusmods.com" || host.ends_with(".nexusmods.com")) {
        return Err("Only Nexus Mods pages open in this window.".into());
    }
    Ok(u)
}

#[tauri::command]
async fn nexus_window(handle: AppHandle, url: String) -> Res<()> {
    let u = nexus_page(&url)?;
    if let Some(w) = handle.get_webview_window(NEXUS_WINDOW) {
        w.navigate(u).map_err(|e| e.to_string())?;
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    let (h1, h2) = (handle.clone(), handle.clone());
    tauri::WebviewWindowBuilder::new(&handle, NEXUS_WINDOW, tauri::WebviewUrl::External(u))
        .title("Nexus Mods · Zom's Nexus")
        .inner_size(1100.0, 820.0)
        .min_inner_size(600.0, 400.0)
        // The Slow download button opens an nxm:// link: take it here, straight into the queue.
        .on_navigation(move |url| {
            if url.scheme() == "nxm" {
                take_link(&h1, &[url.to_string()]);
                return false;
            }
            true
        })
        // Manual download would save the file inside this window, where nothing uses it.
        .on_download(move |_, event| {
            if let tauri::webview::DownloadEvent::Requested { .. } = event {
                let _ = h2.emit("nexus:manual", ());
                return false;
            }
            true
        })
        .build()
        .map_err(|e| format!("Couldn't open the Nexus Mods window: {e}"))?;
    Ok(())
}

#[tauri::command]
fn nexus_window_close(handle: AppHandle) {
    if let Some(w) = handle.get_webview_window(NEXUS_WINDOW) {
        let _ = w.close();
    }
}

/// Put the app's icon in your icon theme (~/.local/share/icons), so the window, the task bar and
/// the Nexus Mods link entry show it. A dev build also gets a hidden desktop entry, because
/// Wayland desktops find a window's icon through one; installed packages bring their own.
#[cfg(target_os = "linux")]
fn install_icon() {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return };
    let icons = home.join(".local/share/icons/hicolor");
    for (size, bytes) in [
        ("32x32", &include_bytes!("../icons/32x32.png")[..]),
        ("128x128", &include_bytes!("../icons/128x128.png")[..]),
        ("256x256", &include_bytes!("../icons/256x256.png")[..]),
        ("512x512", &include_bytes!("../icons/512x512.png")[..]),
    ] {
        let file = icons.join(size).join("apps/zoms-nexus.png");
        if fs::read(&file).ok().as_deref() != Some(bytes) {
            let _ = fs::create_dir_all(file.parent().unwrap());
            let _ = fs::write(&file, bytes);
        }
    }
    // The window's app id is "zoms-nexus", so the desktop finds its icon through
    // zoms-nexus.desktop, the same name the .deb and .rpm install in /usr/share/applications.
    // A dev build writes a personal one (marked as ours); any other build deletes that copy, so
    // it never hides the installed entry.
    let Some(dir) = applications_dir() else { return };
    let file = dir.join("zoms-nexus.desktop");
    let ours = fs::read_to_string(&file).map(|t| t.contains(DEV_MARK)).unwrap_or(false);
    // The .deb and .rpm install "Zom's Nexus.desktop" (it names the window's app id itself).
    let installed = ["/usr/share/applications/zoms-nexus.desktop", "/usr/share/applications/Zom's Nexus.desktop"].iter().any(|p| std::path::Path::new(p).exists());
    if cfg!(debug_assertions) && !installed {
        if let Ok(exe) = program() {
            let entry = format!(
                "[Desktop Entry]\nType=Application\nName=Zom's Nexus (dev)\nExec={}\nIcon=zoms-nexus\nTerminal=false\nNoDisplay=true\n{DEV_MARK}\n",
                exec_quote(&exe.to_string_lossy())
            );
            if fs::read_to_string(&file).ok().as_deref() != Some(entry.as_str()) && (ours || !file.exists()) {
                let _ = fs::create_dir_all(&dir);
                let _ = fs::write(&file, entry);
            }
        }
    } else if ours {
        let _ = fs::remove_file(&file);
    }
}

#[cfg(target_os = "linux")]
const DEV_MARK: &str = "X-Zoms-Nexus-Dev=true";

pub fn run() {
    // Before the window opens, so the desktop finds the icon for it.
    #[cfg(target_os = "linux")]
    install_icon();
    let background = std::env::args().any(|a| a == BACKGROUND);
    tauri::Builder::default()
        // nxm:// links the Nexus Mods window loads in a frame (navigations are caught above).
        .register_asynchronous_uri_scheme_protocol("nxm", |ctx, request, responder| {
            take_link(ctx.app_handle(), &[request.uri().to_string()]);
            responder.respond(tauri::http::Response::builder().status(204).body(Vec::new()).unwrap());
        })
        .on_window_event(|window, event| match (window.label(), event) {
            // Closing the window while files download keeps them going, in the tray.
            ("main", WindowEvent::CloseRequested { api, .. }) => {
                let app = window.state::<App>();
                if app.queue.busy() {
                    api.prevent_close();
                    // With a tray icon to come back through, hide; without one, minimize, so
                    // the window can't be lost.
                    let tray = app.has_tray.load(std::sync::atomic::Ordering::Relaxed);
                    let _ = if tray { window.hide() } else { window.minimize() };
                    if !app.told_tray.swap(true, std::sync::atomic::Ordering::Relaxed) {
                        notify(
                            "Still downloading",
                            if tray { "Zom's Nexus carries on in the background. Open it from its icon in the system tray." } else { "Zom's Nexus carries on, minimized. Close it again once the downloads finish." },
                        );
                    }
                } else {
                    window.app_handle().exit(0);
                }
            }
            (NEXUS_WINDOW, WindowEvent::Destroyed) => {
                let _ = window.app_handle().emit("nexus:closed", ());
            }
            _ => {}
        })
        // A second start (a link from the browser) hands its link to the copy already open.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| take_link(app, &args)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let cache_dir = app.path().app_cache_dir()?;
            fs::create_dir_all(&config_dir)?;
            fs::create_dir_all(&data_dir)?;
            let s = settings::load(&config_dir);
            let _ = fs::create_dir_all(s.library());
            let nexus = Nexus::new(settings::load_key(&config_dir));
            let settings = Arc::new(Mutex::new(s));
            let queue = Queue::new(Some(app.handle().clone()), data_dir.clone(), settings.clone(), nexus.clone());
            queue.start();
            app.manage(App {
                config_dir,
                data_dir,
                cache_dir,
                settings,
                nexus: nexus.clone(),
                queue,
                user: Mutex::new(None),
                updates: Mutex::new(None),
                checks: Mutex::new(HashMap::new()),
                profiles: tokio::sync::Mutex::new(()),
                told_tray: Default::default(),
                has_tray: Default::default(),
            });

            // The requests left, to the page whenever an answer from Nexus Mods changes them.
            let h = app.handle().clone();
            nexus.on_limits(Arc::new(move |l: &RateLimit| {
                let _ = h.emit("limits:changed", l.clone());
            }));

            // The tray icon: open, pause or resume, quit.
            let open = MenuItem::with_id(app, "open", "Open Zom's Nexus", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", "Pause all downloads", true, None::<&str>)?;
            let resume = MenuItem::with_id(app, "resume", "Resume all downloads", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &pause, &resume, &PredefinedMenuItem::separator(app)?, &quit])?;
            let mut tray = TrayIconBuilder::with_id("main").tooltip("Zom's Nexus").menu(&menu).on_menu_event(|app, e| match e.id().as_ref() {
                "open" => show_main(app),
                "pause" => app.state::<App>().queue.pause_all(),
                "resume" => app.state::<App>().queue.resume_all(),
                "quit" => app.exit(0),
                _ => {}
            });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            // The tray needs the appindicator library; without it, building one would crash.
            if system::tray_library().is_some() && tray.build(app).is_ok() {
                app.state::<App>().has_tray.store(true, std::sync::atomic::Ordering::Relaxed);
            }

            // Everything finished while the window was closed: say so, and close.
            let h = app.handle().clone();
            app.listen_any("queue:idle", move |_| {
                let hidden = h.get_webview_window("main").and_then(|w| w.is_visible().ok()).map(|v| !v).unwrap_or(false);
                if hidden {
                    notify("Downloads finished", "Every file is in your library.");
                    h.exit(0);
                }
            });

            if background {
                // Started with the computer: stay out of the way, and close if there's nothing to do.
                let h = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                    if h.state::<App>().queue.busy() {
                        // No tray to open it from: show it, minimized.
                        if !h.state::<App>().has_tray.load(std::sync::atomic::Ordering::Relaxed) {
                            if let Some(w) = h.get_webview_window("main") {
                                let _ = w.show();
                                let _ = w.minimize();
                            }
                        }
                        notify("Carrying on with your downloads", "Zom's Nexus is finishing them in the background.");
                    } else {
                        h.exit(0);
                    }
                });
            } else {
                show_main(app.handle());
            }

            // Waiting files added before their details were looked up: look them up once now
            // (each is the request its download would make anyway, and it's kept for then).
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let app = h.state::<App>();
                for (game, mod_id, file_id) in app.queue.undescribed().into_iter().take(50) {
                    match app.nexus.file(&game, mod_id, file_id).await {
                        Ok(f) => app.queue.describe_file(&game, mod_id, &f, true),
                        Err(e) if e.status == 429 || e.status == 0 => break,
                        Err(_) => {}
                    }
                }
            });

            // Learn whether the account is Premium, in the background.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if nexus.has_key() {
                    if let Ok(u) = nexus.validate().await {
                        handle.state::<App>().set_user(Some(u));
                        let _ = handle.emit("account:changed", ());
                    }
                }
            });

            let args: Vec<String> = std::env::args().collect();
            take_link(app.handle(), &args);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_overview,
            save_settings,
            set_api_key,
            forget_api_key,
            refresh_account,
            lookup_mod,
            queue_list,
            queue_add_link,
            queue_add_files,
            queue_pause,
            queue_resume,
            queue_remove,
            queue_pause_all,
            queue_resume_all,
            queue_now,
            queue_sort,
            queue_retry_failed,
            mod_requirements,
            queue_requirements,
            profile_resolve,
            system_check,
            profile_updates,
            mod_notes,
            set_mod_note,
            diagnostics,
            queue_clear_finished,
            queue_reorder,
            library_list,
            library_unpack,
            library_remove_file,
            library_remove_mod,
            library_import,
            library_usage,
            library_drop_archive,
            open_folder,
            games_list,
            games_add,
            steam_libraries,
            nexus_window,
            nexus_window_close,
            games_remove,
            nxm_status,
            nxm_register,
            game_art,
            library_check_updates,
            library_updates,
            queue_add,
            browse,
            mod_files,
            lookup_collection,
            queue_add_collection,
            profiles_list,
            profile_save,
            profile_delete,
            profile_check,
            profile_checks,
            profile_apply,
            profile_clear,
            game_launch,
            profile_play,
            profile_export,
            profile_import,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Zom's Nexus");
}
