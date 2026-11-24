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

mod activity;
mod ai;
mod art;
mod backup;
mod bg3;
mod check;
mod collections;
mod deploy;
mod disk;
mod gamelogs;
mod gone;
mod fomod;
mod gamepad;
mod games;
mod history;
mod host;
mod launchers;
mod launchopts;
mod layers;
mod library;
mod loaders;
mod loot;
mod log;
mod modsettings;
mod mo2;
mod thunderstore;
mod tml;
mod runs;
mod nexus;
mod nxm;
mod order;
mod profiles;
mod queue;
mod release;
mod requirements;
mod saves;
mod snapshots;
mod settings;
mod steamshortcut;
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
    /// Why new downloads are waiting now (download hours, a game running), if they are.
    held: Option<String>,
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
    let held = app.queue.held();
    Overview { settings, has_key: app.nexus.has_key(), user, limits, version: nexus::APP_VERSION, held, free_space: disk.map(|d| d.0), total_space: disk.map(|d| d.1) }
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
    s.game_libraries = before.game_libraries.clone();
    s.version = settings::VERSION;
    activity::set_off(&s.activity_off);
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

// ---------- mod installers (FOMOD) ----------

/// A file's installer, with file conditions answered from the game's mod folder as it is now.
fn installer_for(app: &App, game: &str, mod_id: u64, file_id: u64) -> Res<(PathBuf, library::StoredMod, fomod::Installer)> {
    let lib = lib_dir(app);
    let dir = library::find(&lib, game, mod_id)?;
    let meta = library::read_meta(&dir).ok_or("This mod's nexus.json is missing.")?;
    let f = meta.files.iter().find(|f| f.file_id == file_id).ok_or("That file isn't in the library any more.")?.clone();
    // The raw unpacked files, never the built folder.
    let raw = match &f.unpacked {
        Some(u) => dir.join(library::safe_name(u)),
        None => deploy::unpacked_dir(&meta, &library::StoredFile { installed: None, ..f.clone() })?,
    };
    let root = fomod::root(&raw).ok_or("This file has no installer.")?;
    let mods_dir = game_target(app, game).ok().map(|(r, folder, _, _)| r.join(folder));
    let state = move |file: &str| -> &'static str {
        match &mods_dir {
            Some(d) if find_nocase(d, file).is_some() => "Active",
            _ => "Missing",
        }
    };
    let mut inst = fomod::read(&root, &state)?;
    inst.current = f.installed.as_ref().map(|i| i.choices.clone());
    let curator = fomod::suggested(&app.data_dir).get(&format!("{game}:{mod_id}:{file_id}")).map(|c| fomod::fit(&inst, c));
    inst.from_collection = inst.current.is_none() && curator.is_some();
    inst.previous = inst
        .current
        .clone()
        .or(curator)
        .or_else(|| meta.files.iter().filter(|o| o.file_id != file_id).filter_map(|o| o.installed.as_ref()).map(|i| i.choices.clone()).next());
    Ok((dir, meta, inst))
}

/// `rel` under `dir`, matching each part without case (Windows games' files).
fn find_nocase(dir: &Path, rel: &str) -> Option<PathBuf> {
    let mut p = dir.to_path_buf();
    for part in rel.split(['/', '\\']).filter(|x| !x.is_empty() && *x != ".") {
        if part == ".." {
            return None;
        }
        let exact = p.join(part);
        p = if exact.exists() { exact } else { fs::read_dir(&p).ok()?.flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part))?.path() };
    }
    Some(p)
}

#[tauri::command]
async fn fomod_read(app: State<'_, App>, game: String, mod_id: u64, file_id: u64) -> Res<fomod::Installer> {
    // Reading an installer is quick (one small file and a folder listing).
    installer_for(&app, &game, mod_id, file_id).map(|(_, _, i)| i)
}

/// Build the file's folder from the picked options, and use it from now on.
#[tauri::command]
async fn fomod_install(app: State<'_, App>, game: String, mod_id: u64, file_id: u64, choices: Vec<fomod::Choice>) -> Res<library::StoredMod> {
    let (dir, meta, inst) = installer_for(&app, &game, mod_id, file_id)?;
    let f = meta.files.iter().find(|f| f.file_id == file_id).ok_or("That file isn't in the library any more.")?;
    let base = f.unpacked.clone().unwrap_or_else(|| f.name.clone());
    let d2 = dir.clone();
    let installed = tokio::task::spawn_blocking(move || fomod::install(&inst, &choices, &d2, &base)).await.map_err(|e| e.to_string())??;
    library::set_installed(&dir, file_id, Some(installed))?;
    log::info(&format!("Installed {} ({game} {mod_id}/{file_id}) with its installer's choices", meta.name));
    library::read_meta(&dir).ok_or_else(|| "This mod's nexus.json is missing.".into())
}

/// Forget the installer choices: the file is used as it is again.
#[tauri::command]
fn fomod_reset(app: State<'_, App>, game: String, mod_id: u64, file_id: u64) -> Res<()> {
    let dir = library::find(&lib_dir(&app), &game, mod_id)?;
    library::set_installed(&dir, file_id, None)
}

/// A picture from the installer, as a data address the page can show.
#[tauri::command]
fn fomod_image(app: State<'_, App>, game: String, mod_id: u64, file_id: u64, path: String) -> Res<Option<String>> {
    use base64::Engine;
    let dir = library::find(&lib_dir(&app), &game, mod_id)?;
    let meta = library::read_meta(&dir).ok_or("This mod's nexus.json is missing.")?;
    let Some(u) = meta.files.iter().find(|f| f.file_id == file_id).and_then(|f| f.unpacked.clone()) else { return Ok(None) };
    let Some(root) = fomod::root(&dir.join(library::safe_name(&u))) else { return Ok(None) };
    Ok(fomod::image(&root, &path).map(|(b, mime)| format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(b))))
}

/// Keep a game's mods in another folder (`to`), or back in the library (None): they're moved
/// there, then used from there. Gives the bytes moved.
#[tauri::command]
async fn library_move_game(app: State<'_, App>, game: String, to: Option<String>) -> Res<u64> {
    let lib = lib_dir(&app);
    let from = library::root_for(&lib, &game);
    let target = match &to {
        Some(t) => {
            let p = PathBuf::from(t.trim());
            if !p.is_absolute() {
                return Err("Pick a folder.".into());
            }
            p
        }
        None => lib.clone(),
    };
    if target == from {
        return Ok(0);
    }
    if target.starts_with(from.join(&game)) {
        return Err("Pick a folder outside the one the mods are in now.".into());
    }
    let (f, t, g) = (from.clone(), target.clone(), game.clone());
    let moved = tokio::task::spawn_blocking(move || library::move_game(&f, &t, &g)).await.map_err(|e| e.to_string())??;
    let mut s = app.settings.lock().unwrap().clone();
    match to {
        Some(_) if target != lib => {
            s.game_libraries.insert(game.clone(), target.to_string_lossy().into_owned());
        }
        _ => {
            s.game_libraries.remove(&game);
        }
    }
    settings::save(&app.config_dir, &s)?;
    library::set_roots(&s.game_libraries);
    *app.settings.lock().unwrap() = s;
    log::info(&format!("Moved {game}'s mods to {} ({moved} bytes)", target.display()));
    Ok(moved)
}

/// Save what Zom's Nexus knows (profiles, notes, tags, restore points, settings) to one file.
#[tauri::command]
fn app_backup(app: State<'_, App>, path: String) -> Res<usize> {
    let b = backup::make(&app.config_dir, &app.data_dir, queue::now());
    let mut path = PathBuf::from(path);
    if path.extension().is_none() {
        path.set_extension("json");
    }
    fs::write(&path, serde_json::to_vec_pretty(&b).map_err(|e| e.to_string())?).map_err(|e| format!("Couldn't save the backup: {e}"))?;
    Ok(b.files.len())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Restored {
    made: u64,
    files: Vec<String>,
}

/// Put back a backup. Profiles and the rest are replaced by the backup's; this computer's
/// library folders stay. The settings take effect at once.
#[tauri::command]
async fn app_restore(app: State<'_, App>, path: String) -> Res<Restored> {
    let b = backup::read(Path::new(&path))?;
    let _turn = app.profiles.lock().await;
    let keep = app.settings.lock().unwrap().clone();
    let files = backup::restore(&b, &app.config_dir, &app.data_dir, &keep)?;
    let s = settings::load(&app.config_dir);
    activity::set_off(&s.activity_off);
    library::set_roots(&s.game_libraries);
    *app.settings.lock().unwrap() = s;
    app.queue.poke();
    log::info(&format!("Restored a backup from {} ({} files)", log::stamp(log::local(b.made)), files.len()));
    Ok(Restored { made: b.made, files })
}

/// Stop waiting for a game to start, and with `end`, close it (its processes in its folder).
#[tauri::command]
fn game_stop(app: State<'_, App>, game: String, end: bool) -> Res<usize> {
    let path = list_games(&app).into_iter().find(|g| g.domain == game).and_then(|g| g.path).ok_or("That game's folder isn't known.")?;
    Ok(activity::stop(&game, Path::new(&path), end))
}

/// What changed in a mod, from its changelog on Nexus Mods (one request, when asked).
#[tauri::command]
async fn mod_changelog(app: State<'_, App>, game: String, mod_id: u64) -> Res<Vec<nexus::Change>> {
    if game.is_empty() || !game.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') || library::is_local(mod_id) {
        return Ok(vec![]);
    }
    app.nexus.changelog(&game, mod_id).await.map_err(|e| e.message)
}

/// The space each library file takes on disk (archive and unpacked folder).
#[tauri::command]
async fn library_usage(app: State<'_, App>) -> Res<Vec<library::FileUsage>> {
    let lib = lib_dir(&app);
    tokio::task::spawn_blocking(move || library::usage(&lib)).await.map_err(|e| e.to_string())
}

// ---------- activity ----------

#[tauri::command]
fn activity_list() -> Vec<activity::Event> {
    activity::list()
}

/// Mark everything read (the page is open), or clear the list.
#[tauri::command]
fn activity_mark(clear: bool) {
    activity::change(|all| {
        if clear {
            all.clear();
        } else {
            all.iter_mut().for_each(|e| e.read = true);
        }
    });
}

#[tauri::command]
fn activity_ignored(app: State<'_, App>) -> Vec<String> {
    activity::ignored(&app.data_dir)
}

/// Ignore a problem on the Activity page (or stop ignoring it).
#[tauri::command]
fn activity_ignore(app: State<'_, App>, key: String, on: bool) -> Res<Vec<String>> {
    activity::set_ignored(&app.data_dir, &key, on)
}

#[tauri::command]
fn profile_play_times(app: State<'_, App>) -> HashMap<String, activity::PlayTime> {
    activity::play_times(&app.data_dir)
}

/// What fills the library's drive (and the game's, when it's another one): mods, archives,
/// Steam games and the rest. Reads only this computer.
#[tauri::command]
async fn disk_usage(app: State<'_, App>, game: Option<String>) -> Res<Vec<disk::DriveUse>> {
    let lib = lib_dir(&app);
    let target = game.and_then(|d| list_games(&app).into_iter().find(|g| g.domain == d)).and_then(|g| Some((g.name.clone(), PathBuf::from(g.path?))));
    tokio::task::spawn_blocking(move || disk::breakdown(&lib, target.as_ref().map(|(n, p)| (n.as_str(), p.as_path())), &games::steam_library_dirs()))
        .await
        .map_err(|e| e.to_string())
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
            let d = library::game_dir(&lib, &g)?;
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

/// Heroic and Lutris games, read again at most every 30 seconds (games are listed often).
fn launchers_cached() -> Vec<launchers::Found> {
    static KEPT: std::sync::Mutex<Option<(std::time::Instant, Vec<launchers::Found>)>> = std::sync::Mutex::new(None);
    let mut k = KEPT.lock().unwrap();
    if let Some((at, v)) = k.as_ref() {
        if at.elapsed() < std::time::Duration::from_secs(30) {
            return v.clone();
        }
    }
    let v = launchers::all();
    *k = Some((std::time::Instant::now(), v.clone()));
    v
}

/// This computer: a Steam Deck, SteamOS, Game Mode, Flatpak.
#[tauri::command]
fn device() -> launchers::Device {
    launchers::device()
}

/// Proton builds added by hand (GE-Proton and friends).
#[tauri::command]
fn custom_protons() -> Vec<String> {
    launchers::custom_protons()
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
            g.save_folder = a.save_folder.clone();
            g.launch = a.launch.clone();
            continue;
        }
        list.push(games::Game {
            domain: a.domain.clone(),
            name: a.name.clone(),
            path: a.path.clone(),
            source: "added".into(),
            steam_app_id: a.steam_app_id,
            proton_prefix: a.steam_app_id.zip(a.path.as_deref()).and_then(|(id, p)| games::proton_prefix_for(Path::new(p), id)).map(|p| p.to_string_lossy().into_owned()),
            mod_folder: a.mod_folder.clone().or_else(|| deploy::default_mod_folder(&a.domain).map(String::from)),
            launch: a.launch.clone(),
            active_profile: None,
            free_space: None,
            total_space: None,
            last_updated: None,
            mod_tool: None,
            mods_copied: false,
            save_folder: a.save_folder.clone(),
            steam_cloud: false,
            proton: None,
        });
    }
    // Heroic (GOG, Epic, Amazon) and Lutris games Zom's Nexus knows, when Steam doesn't have them.
    for f in launchers_cached() {
        let Some((domain, name)) = launchers::known_for(&f.title) else { continue };
        if list.iter().any(|g| g.domain == domain) {
            continue;
        }
        list.push(games::Game {
            domain: domain.to_string(),
            name: name.to_string(),
            path: Some(f.path.to_string_lossy().into_owned()),
            source: f.source.clone(),
            steam_app_id: None,
            proton_prefix: f.prefix.as_ref().map(|p| p.to_string_lossy().into_owned()),
            mod_folder: deploy::default_mod_folder(domain).map(String::from),
            launch: Some(f.launch.clone()),
            active_profile: None,
            free_space: None,
            total_space: None,
            last_updated: None,
            mod_tool: None,
            mods_copied: false,
            save_folder: None,
            steam_cloud: false,
            proton: f.wine.clone(),
        });
    }
    let lib = app.settings.lock().unwrap().library();
    for g in &mut list {
        if g.proton.is_none() {
            g.proton = g.proton_prefix.as_deref().and_then(|p| launchers::steam_proton(Path::new(p)));
        }
        g.active_profile = deploy::deployed(&app.data_dir, &g.domain).profile_name;
        let disk = g.path.as_deref().and_then(|p| settings::disk(Path::new(p)));
        g.mod_tool = deploy::mod_tool(&g.domain).map(String::from);
        g.save_folder = saves::folder(&g.domain, g.save_folder.as_deref(), g.proton_prefix.as_deref().map(Path::new), g.path.as_deref().map(Path::new)).map(|p| p.to_string_lossy().into_owned());
        g.steam_cloud = g.save_folder.is_some() && g.steam_app_id.map(games::steam_cloud).unwrap_or(false);
        // Against the game's own library folder, when it has one (on its drive, say).
        let root = library::root_for(&lib, &g.domain);
        let root = if root.is_dir() { root } else { lib.clone() };
        g.mods_copied = g.path.as_deref().map(|p| Path::new(p).is_dir() && root.is_dir() && !deploy::same_drive(&root, Path::new(p))).unwrap_or(false);
        g.free_space = disk.map(|d| d.0);
        g.total_space = disk.map(|d| d.1);
    }
    list.sort_by_key(|a| a.name.to_lowercase());
    list
}

#[tauri::command]
async fn games_list(app: State<'_, App>) -> Res<Vec<games::Game>> {
    Ok(list_games(&app))
}

/// The Steam library folders searched for games.
/// A game that was uninstalled while its mods are still in the library.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GoneGame {
    domain: String,
    name: String,
    path: String,
    mods: usize,
    bytes: u64,
    profiles: usize,
}

/// Games uninstalled since Zom's Nexus last saw them, with mods still in the library (and not
/// marked to keep). Notes where the installed ones live as it goes. Reads only this computer.
#[tauri::command]
async fn games_gone(app: State<'_, App>) -> Res<Vec<GoneGame>> {
    let lib = lib_dir(&app);
    let installed: Vec<(String, String, String)> = list_games(&app).into_iter().filter_map(|g| g.path.clone().filter(|p| Path::new(p).is_dir()).map(|p| (g.domain, g.name, p))).collect();
    let mut seen = gone::load(&app.data_dir);
    if gone::note(&mut seen, &installed) {
        gone::save(&app.data_dir, &seen)?;
    }
    let profiles = profiles::load(&app.config_dir);
    let gone_list: Vec<(String, gone::Known)> = seen.into_iter().filter(|(d, k)| !k.kept && !installed.iter().any(|(x, _, _)| x == d) && gone::uninstalled(Path::new(&k.path))).collect();
    if gone_list.is_empty() {
        return Ok(vec![]);
    }
    tokio::task::spawn_blocking(move || {
        let usage = library::usage(&lib);
        gone_list
            .into_iter()
            .filter_map(|(domain, k)| {
                let mine: Vec<_> = usage.iter().filter(|u| u.game == domain).collect();
                let mods = mine.iter().map(|u| u.mod_id).collect::<HashSet<_>>().len();
                (mods > 0).then(|| GoneGame { profiles: profiles.iter().filter(|p| p.game == domain).count(), bytes: mine.iter().map(|u| u.archive + u.unpacked).sum(), mods, name: k.name, path: k.path, domain })
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

/// Answer for an uninstalled game: delete its mods (and, if asked, its profiles), or keep them
/// and don't ask again until it's installed again. Gives the bytes freed.
#[tauri::command]
async fn games_gone_answer(app: State<'_, App>, domain: String, delete: bool, profiles_too: bool) -> Res<u64> {
    let mut seen = gone::load(&app.data_dir);
    let k = seen.get(&domain).cloned().ok_or("Zom's Nexus doesn't know that game.")?;
    if !gone::uninstalled(Path::new(&k.path)) {
        return Err(format!("{} is still installed (or its drive isn't connected), so its mods were left alone.", k.name));
    }
    let mut freed = 0;
    if delete {
        let lib = lib_dir(&app);
        let d2 = domain.clone();
        freed = tokio::task::spawn_blocking(move || library::remove_game(&lib, &d2)).await.map_err(|e| e.to_string())??;
        // The game folder is gone, so what was put in it went with it.
        let _ = fs::remove_file(app.data_dir.join("deployed").join(format!("{domain}.json")));
        if profiles_too {
            let _turn = app.profiles.lock().await;
            let mut all = profiles::load(&app.config_dir);
            all.retain(|p| p.game != domain);
            profiles::save(&app.config_dir, &all)?;
        }
        log::info(&format!("{} was uninstalled: deleted its mods ({freed} bytes){}", k.name, if profiles_too { " and profiles" } else { "" }));
        seen.remove(&domain);
    } else if let Some(x) = seen.get_mut(&domain) {
        x.kept = true;
    }
    gone::save(&app.data_dir, &seen)?;
    Ok(freed)
}

#[tauri::command]
async fn steam_libraries() -> Res<Vec<games::SteamLibrary>> {
    tokio::task::spawn_blocking(games::steam_libraries).await.map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Catalog {
    /// Every game on Nexus Mods (asked for once a week).
    games: Vec<nexus::NexusGame>,
    /// Everything Steam has installed here.
    installed: Vec<games::SteamApp>,
}

/// For adding a game: every game on Nexus Mods, and what Steam has installed.
#[tauri::command]
async fn game_catalog(app: State<'_, App>) -> Res<Catalog> {
    let file = app.cache_dir.join("nexus-games.json");
    let fresh = fs::metadata(&file).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|d| d.as_secs() < 7 * 86400).unwrap_or(false);
    let mut list: Vec<nexus::NexusGame> = if fresh { fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default() } else { vec![] };
    if list.is_empty() && app.nexus.has_key() {
        match app.nexus.games().await {
            Ok(g) => {
                let _ = fs::create_dir_all(&app.cache_dir);
                let _ = fs::write(&file, serde_json::to_vec(&g).unwrap_or_default());
                list = g;
            }
            Err(e) => {
                // An old copy is better than none.
                list = fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
                if list.is_empty() {
                    return Err(e.message);
                }
            }
        }
    }
    let installed = tokio::task::spawn_blocking(games::steam_installed).await.map_err(|e| e.to_string())?;
    Ok(Catalog { games: list, installed })
}

#[tauri::command]
fn games_add(app: State<'_, App>, domain: String, name: Option<String>, path: Option<String>, mod_folder: Option<String>, launch: Option<String>, steam_app_id: Option<u32>, save_folder: Option<String>) -> Res<()> {
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
    // Editing a game keeps its Steam number.
    let steam_app_id = steam_app_id.or_else(|| s.games.iter().find(|g| g.domain == domain).and_then(|g| g.steam_app_id));
    s.games.retain(|g| g.domain != domain);
    let save_folder = save_folder.filter(|f| !f.trim().is_empty());
    s.games.push(AddedGame { domain, name, path, mod_folder, launch, steam_app_id, save_folder });
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
    let newer: Vec<&str> = report.updates.iter().filter(|u| u.newer.is_some()).map(|u| u.mod_name.as_str()).collect();
    if !newer.is_empty() {
        let shown = newer.iter().take(4).copied().collect::<Vec<_>>().join(", ");
        activity::add("updates", format!("{} {} an update", newer.len(), if newer.len() == 1 { "mod has" } else { "mods have" }), Some(if newer.len() > 4 { format!("{shown} and {} more.", newer.len() - 4) } else { format!("{shown}.") }), None, Some("library"), None);
    }
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

/// Your tags on mods ("textures", "must have"), by "game:mod number".
#[tauri::command]
fn mod_tags(app: State<'_, App>) -> HashMap<String, Vec<String>> {
    fs::read_to_string(app.config_dir.join("mod-tags.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[tauri::command]
fn set_mod_tags(app: State<'_, App>, game: String, mod_id: u64, tags: Vec<String>) -> Res<HashMap<String, Vec<String>>> {
    let mut all = mod_tags(app.clone());
    let key = format!("{game}:{mod_id}");
    let mut clean: Vec<String> = vec![];
    for t in tags.iter().map(|t| t.trim().chars().take(40).collect::<String>()).filter(|t| !t.is_empty()) {
        if !clean.iter().any(|c| c.eq_ignore_ascii_case(&t)) && clean.len() < 20 {
            clean.push(t);
        }
    }
    if clean.is_empty() {
        all.remove(&key);
    } else {
        all.insert(key, clean);
    }
    settings::write_atomic(&app.config_dir.join("mod-tags.json"), &serde_json::to_vec_pretty(&all).map_err(|e| e.to_string())?)?;
    Ok(all)
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
    // Personal details out: the home folder becomes ~, and the account and computer user names
    // are replaced, wherever they appear (paths, profile names, errors).
    let mut text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    if let Some(home) = std::env::var_os("HOME").map(|h| h.to_string_lossy().into_owned()).filter(|h| h.len() > 1) {
        text = text.replace(&home, "~");
    }
    let mut names: Vec<String> = vec![];
    if let Some(u) = std::env::var_os("USER").map(|u| u.to_string_lossy().into_owned()) {
        names.push(u);
    }
    if let Some(u) = app.user.lock().unwrap().as_ref() {
        names.push(u.name.clone());
    }
    for n in names.into_iter().filter(|n| n.len() >= 3) {
        text = text.replace(&n, "<you>");
    }
    fs::write(&path, text).map_err(|e| format!("Couldn't save the report: {e}"))
}

/// The end of the log, for Settings > Logs.
#[tauri::command]
fn log_tail(lines: Option<usize>) -> String {
    log::tail(lines.unwrap_or(300).min(5000))
}

/// Something the page wants in the log (its own errors).
#[tauri::command]
fn log_page(level: String, message: String) {
    let level = if level == "error" { "page error" } else { "page" };
    log::write(level, &message.chars().take(2000).collect::<String>());
}

#[tauri::command]
fn log_open(handle: AppHandle) -> Res<()> {
    let p = log::path().ok_or("There's no log yet.")?;
    let dir = p.parent().map(Path::to_path_buf).ok_or("There's no log folder.")?;
    handle.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(|e| e.to_string())
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
/// Read the collection's own file for its curator's installer choices, so mods with installers
/// go in the way the collection meant. Never stops what it's part of: a problem is only logged.
async fn learn_choices(app: &App, c: &collections::Collection) {
    let Some(link) = c.download_link.clone() else { return };
    let got = tokio::time::timeout(std::time::Duration::from_secs(60), app.nexus.collection_archive(&link, 64 * 1024 * 1024)).await;
    let bytes = match got {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return log::warn(&format!("Couldn't get {}'s collection file for its installer choices: {}", c.slug, e.message)),
        Err(_) => return log::warn(&format!("Getting {}'s collection file took too long; its installer choices are left to you", c.slug)),
    };
    let dir = app.cache_dir.join("collection-files").join(format!("{}-{}", library::safe_name(&c.slug), c.revision.unwrap_or(0)));
    let (game, slug) = (c.game.clone(), c.slug.clone());
    let data = app.data_dir.clone();
    let res = tokio::task::spawn_blocking(move || -> Res<usize> {
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let archive = dir.join("collection.archive");
        fs::write(&archive, &bytes).map_err(|e| e.to_string())?;
        let found = (|| -> Res<usize> {
            let unpacked = dir.join(library::unpack(&archive)?);
            let json = std::iter::once(unpacked.join("collection.json"))
                .chain(fs::read_dir(&unpacked).into_iter().flatten().flatten().map(|e| e.path().join("collection.json")))
                .find(|p| p.is_file())
                .ok_or("The collection's file has no collection.json.")?;
            if fs::metadata(&json).map(|m| m.len()).unwrap_or(0) > 32 * 1024 * 1024 {
                return Err("The collection.json is too big.".into());
            }
            let v: serde_json::Value = serde_json::from_slice(&fs::read(&json).map_err(|e| e.to_string())?).map_err(|e| format!("Couldn't read collection.json: {e}"))?;
            fomod::remember(&data, &game, collections::choices_in(&v))
        })();
        let _ = fs::remove_dir_all(&dir);
        found
    })
    .await
    .map_err(|e| e.to_string())
    .and_then(|r| r);
    match res {
        Ok(n) => log::info(&format!("{slug}: noted the curator's installer choices for {n} mods")),
        Err(e) => log::warn(&format!("{slug}: couldn't read the collection's installer choices: {e}")),
    }
}

async fn add_collection(app: &App, address: &str, optional: bool) -> Res<CollectionAdded> {
    let a = collections::parse(address)?;
    let c = collections::fetch(&app.nexus, &a).await.map_err(|e| e.message)?;
    learn_choices(app, &c).await;
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
            own_saves: false,
            base: Some(base_of(&c, optional)),
            addons: vec![],
            rules: vec![],
            note: None,
            ts_pack: None,
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

/// The collection's files as a profile's base.
fn base_of(c: &collections::Collection, optional: bool) -> layers::Base {
    layers::Base {
        slug: c.slug.clone(),
        revision: c.revision,
        name: c.name.clone(),
        optional,
        files: c.files.iter().filter(|f| f.game == c.game && (optional || !f.optional)).map(|f| layers::FilePick { mod_id: f.mod_id, file_id: f.file_id }).collect(),
    }
}

/// Queue a collection's files the library doesn't have. Gives how many were queued.
fn queue_collection_files<'a>(app: &App, have: &[library::StoredMod], files: impl Iterator<Item = &'a collections::CollectionFile>) -> usize {
    let mut queued = 0;
    for f in files {
        if have.iter().any(|m| m.game == f.game && m.mod_id == f.mod_id && m.files.iter().any(|x| x.file_id == f.file_id)) {
            continue;
        }
        app.queue.add(&f.game, f.mod_id, f.file_id, None, None, None);
        app.queue.name_mod(&f.game, f.mod_id, &f.mod_name);
        let shown = FileInfo { file_id: f.file_id, name: f.name.clone(), file_name: String::new(), version: f.version.clone(), category: None, size: f.size, uploaded: None, is_primary: false, description: None };
        app.queue.describe_file(&f.game, f.mod_id, &shown, false);
        queued += 1;
    }
    queued
}

// ---------- profiles from collections: the base and add-ons on top (see `layers`) ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LayerView {
    id: String,
    name: String,
    enabled: bool,
    added: Vec<u64>,
    off: Vec<u64>,
    files: Vec<u64>,
}

fn layer_view(id: &str, name: &str, enabled: bool, c: &layers::Changes) -> LayerView {
    LayerView { id: id.into(), name: name.into(), enabled, added: c.added.iter().map(|m| m.mod_id).collect(), off: c.off.clone(), files: c.files.iter().map(|f| f.mod_id).collect() }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BaseView {
    slug: String,
    revision: Option<u64>,
    name: String,
    count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Layers {
    /// None for a profile not made from a collection, or made before bases were kept.
    base: Option<BaseView>,
    /// The collection a profile without a base came from, so it can be linked.
    collection: Option<String>,
    /// The revision its name gives, for linking.
    revision: Option<u64>,
    addons: Vec<LayerView>,
    yours: Option<LayerView>,
    /// Names of the mods named above.
    names: HashMap<u64, String>,
}

fn names_for(app: &App, game: &str, ids: impl Iterator<Item = u64>) -> HashMap<u64, String> {
    let lib = library::list(&lib_dir(app));
    ids.map(|id| (id, lib.iter().find(|m| m.game == game && m.mod_id == id).map(|m| m.name.clone()).unwrap_or_else(|| format!("Mod {id}")))).collect()
}

fn layers_of(app: &App, p: &deploy::Profile) -> Layers {
    let Some(base) = &p.base else {
        return Layers { base: None, collection: p.collection.clone(), revision: layers::revision_in(&p.name), addons: vec![], yours: None, names: HashMap::new() };
    };
    let yours = layers::yours(base, &p.mods, &p.addons);
    let addons: Vec<LayerView> = p.addons.iter().map(|a| layer_view(&a.id, &a.name, a.enabled, &a.changes)).collect();
    let yours = layer_view("yours", "Your changes", true, &yours);
    let ids: HashSet<u64> = addons.iter().chain([&yours]).flat_map(|l| l.added.iter().chain(&l.off).chain(&l.files).copied()).collect();
    Layers {
        base: Some(BaseView { slug: base.slug.clone(), revision: base.revision, name: base.name.clone(), count: base.files.len() }),
        collection: p.collection.clone(),
        revision: base.revision,
        addons,
        yours: Some(yours),
        names: names_for(app, &p.game, ids.into_iter()),
    }
}

/// What the profile's collection, add-ons and your own changes are.
#[tauri::command]
async fn profile_layers(app: State<'_, App>, id: String) -> Res<Layers> {
    let p = find_profile(&app, &id)?;
    Ok(layers_of(&app, &p))
}

/// Change one profile under the profiles lock and save it.
async fn edit_profile<T>(app: &App, id: &str, f: impl FnOnce(&mut deploy::Profile) -> Res<T>) -> Res<(deploy::Profile, T)> {
    let _turn = app.profiles.lock().await;
    let mut all = profiles::load(&app.config_dir);
    let p = all.iter_mut().find(|p| p.id == id).ok_or("That profile isn't there any more.")?;
    let out = f(p)?;
    let p = p.clone();
    profiles::save(&app.config_dir, &all)?;
    Ok((p, out))
}

/// For a profile made before bases were kept: look up its collection at the revision it was
/// made from (one request), so your changes can be told apart from the collection's.
#[tauri::command]
async fn profile_collection_link(app: State<'_, App>, id: String) -> Res<Layers> {
    let p = find_profile(&app, &id)?;
    let p = profile_collection_link_inner(&app, &p).await?;
    Ok(layers_of(&app, &p))
}

async fn profile_collection_link_inner(app: &App, p: &deploy::Profile) -> Res<deploy::Profile> {
    let id = p.id.clone();
    let slug = p.collection.clone().ok_or("This profile wasn't made from a collection.")?;
    let addr = collections::Address { game: p.game.clone(), slug, revision: layers::revision_in(&p.name) };
    let c = collections::fetch(&app.nexus, &addr).await.map_err(|e| e.message)?;
    learn_choices(app, &c).await;
    // Optional files count when the profile has any of them.
    let optional = c.files.iter().any(|f| f.optional && p.mods.iter().any(|m| m.mod_id == f.mod_id));
    let base = base_of(&c, optional);
    Ok(edit_profile(app, &id, |p| {
        p.base = Some(base);
        Ok(())
    })
    .await?
    .0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectionUpdate {
    from: Option<u64>,
    to: Option<u64>,
    /// What moving to `to` does (or did).
    changes: layers::Rebased,
    names: HashMap<u64, String>,
    queued: usize,
    /// The profile after moving, when it moved.
    profile: Option<deploy::Profile>,
}

/// Look for a newer revision of the profile's collection (one request) and say what moving to
/// it would do; with `apply`, move to `revision`, keeping your changes and add-ons, and queue
/// the files the library doesn't have.
#[tauri::command]
async fn profile_collection_update(app: State<'_, App>, id: String, revision: Option<u64>, apply: bool) -> Res<CollectionUpdate> {
    let p = find_profile(&app, &id)?;
    let old = p.base.clone().ok_or("Find your changes first, so they're kept when the collection moves on.")?;
    let addr = collections::Address { game: p.game.clone(), slug: old.slug.clone(), revision: if apply { revision } else { None } };
    let c = collections::fetch(&app.nexus, &addr).await.map_err(|e| e.message)?;
    if apply {
        learn_choices(&app, &c).await;
    }
    let new = base_of(&c, old.optional);
    let mut names: HashMap<u64, String> = c.files.iter().map(|f| (f.mod_id, f.mod_name.clone())).collect();
    if !apply {
        let mut mods = p.mods.clone();
        let on: Vec<&layers::Changes> = p.addons.iter().filter(|a| a.enabled).map(|a| &a.changes).collect();
        let changes = layers::rebase(&mut mods, &old, &new, &on);
        for (k, v) in names_for(&app, &p.game, changes.removed.iter().copied()) {
            names.entry(k).or_insert(v);
        }
        return Ok(CollectionUpdate { from: old.revision, to: c.revision, changes, names, queued: 0, profile: None });
    }
    let (p, changes) = edit_profile(&app, &id, |p| {
        let on: Vec<layers::Changes> = p.addons.iter().filter(|a| a.enabled).map(|a| a.changes.clone()).collect();
        let base = p.base.clone().ok_or("Find your changes first.")?;
        let r = layers::rebase(&mut p.mods, &base, &new, &on.iter().collect::<Vec<_>>());
        if let Some(rev) = new.revision {
            p.name = layers::renamed(&p.name, rev);
        }
        p.base = Some(new.clone());
        Ok(r)
    })
    .await?;
    let lib = lib_dir(&app);
    let have = tokio::task::spawn_blocking(move || library::list(&lib)).await.map_err(|e| e.to_string())?;
    let wanted: HashSet<(u64, u64)> = p.mods.iter().filter(|m| m.enabled).map(|m| (m.mod_id, m.file_id)).collect();
    let queued = queue_collection_files(&app, &have, c.files.iter().filter(|f| f.game == c.game && wanted.contains(&(f.mod_id, f.file_id))));
    for (k, v) in names_for(&app, &p.game, changes.removed.iter().copied()) {
        names.entry(k).or_insert(v);
    }
    Ok(CollectionUpdate { from: old.revision, to: c.revision, changes, names, queued, profile: Some(p) })
}

/// Make your changes into a named add-on.
#[tauri::command]
async fn profile_addon_save(app: State<'_, App>, id: String, name: String) -> Res<deploy::Profile> {
    let name: String = name.trim().chars().take(80).collect();
    if name.is_empty() {
        return Err("Give the add-on a name.".into());
    }
    Ok(edit_profile(&app, &id, |p| {
        let base = p.base.as_ref().ok_or("This profile has no collection to put an add-on on.")?;
        let changes = layers::yours(base, &p.mods, &p.addons);
        if changes.is_empty() {
            return Err("There are no changes of yours to make an add-on from.".into());
        }
        p.addons.push(layers::Addon { id: format!("a{}", queue::now()), name, enabled: true, changes });
        Ok(())
    })
    .await?
    .0)
}

/// Switch an add-on on or off, rename it, or take it out (its changes come off first).
#[tauri::command]
async fn profile_addon_set(app: State<'_, App>, id: String, addon: String, enabled: Option<bool>, name: Option<String>, remove: bool) -> Res<deploy::Profile> {
    Ok(edit_profile(&app, &id, |p| {
        let base = p.base.clone().ok_or("This profile has no collection.")?;
        let i = p.addons.iter().position(|a| a.id == addon).ok_or("That add-on isn't there any more.")?;
        if let Some(n) = name.map(|n| n.trim().chars().take(80).collect::<String>()).filter(|n| !n.is_empty()) {
            p.addons[i].name = n;
        }
        let want = if remove { false } else { enabled.unwrap_or(p.addons[i].enabled) };
        if want != p.addons[i].enabled {
            let changes = p.addons[i].changes.clone();
            if want {
                layers::apply(&mut p.mods, &changes);
            } else {
                let others: Vec<layers::Changes> = p.addons.iter().enumerate().filter(|(j, a)| *j != i && a.enabled).map(|(_, a)| a.changes.clone()).collect();
                layers::revert(&mut p.mods, &changes, &base, &others.iter().collect::<Vec<_>>());
            }
            p.addons[i].enabled = want;
        }
        if remove {
            p.addons.remove(i);
        }
        Ok(())
    })
    .await?
    .0)
}

/// Save an add-on as an add-on pack: the collection it goes on, and the Nexus Mods numbers of
/// what it changes. Never the mods themselves.
#[tauri::command]
async fn profile_addon_export(app: State<'_, App>, id: String, addon: String, path: String) -> Res<()> {
    let p = find_profile(&app, &id)?;
    let base = p.base.as_ref().ok_or("This profile has no collection.")?;
    let a = p.addons.iter().find(|a| a.id == addon).ok_or("That add-on isn't there any more.")?;
    let mods = library::list(&lib_dir(&app));
    let pack = profiles::addon_pack(&p.game, base, a, |m, f| {
        let sm = mods.iter().find(|x| x.game == p.game && x.mod_id == m);
        (sm.map(|x| x.name.clone()).unwrap_or_default(), sm.and_then(|x| x.files.iter().find(|y| y.file_id == f)).map(|y| y.file_name.clone()).unwrap_or_default())
    });
    let mut path = PathBuf::from(path);
    if path.extension().is_none() {
        path.set_extension("json");
    }
    fs::write(&path, serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?).map_err(|e| format!("Couldn't save the add-on pack: {e}"))
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
        let rules = profile.rules.clone();
        order::keep(&mut profile.mods, &rules);
        profile.base = p.base.clone();
        profile.addons = p.addons.clone();
        *p = profile.clone();
    } else {
        all.push(profile.clone());
    }
    profiles::save(&app.config_dir, &all)?;
    // The last check stays until the next one replaces it (the page checks again after a save),
    // so leaving the page and coming back never shows an empty card.
    Ok(profile)
}

/// Put the profiles in the order given (dragged on the Profiles page). Ids not given keep
/// their place after the others.
#[tauri::command]
async fn profiles_order(app: State<'_, App>, ids: Vec<String>) -> Res<Vec<deploy::Profile>> {
    let _turn = app.profiles.lock().await;
    let mut all = profiles::load(&app.config_dir);
    all.sort_by_key(|p| ids.iter().position(|i| *i == p.id).unwrap_or(usize::MAX));
    profiles::save(&app.config_dir, &all)?;
    Ok(all)
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
    let data2 = app.data_dir.clone();
    let (mut report, mods) = tokio::task::spawn_blocking(move || {
        let mods = library::list(&lib);
        let mut r = check::check(&p2, &mods, &folder, root.as_deref(), &ours);
        // Mod loaders, told apart by their files, so a game never seen before gets the same help.
        if let Some(root) = root.as_deref() {
            r.loaders_missing = loaders::missing(root, &r.owners);
            r.loaders = loaders::present(root, &r.owners);
        }
        check::use_order(&mut r, &loot::saved(&data2, &p2.id));
        (r, mods)
    })
    .await
    .map_err(|e| e.to_string())?;
    // What the mods need, and whether they're current, from Nexus Mods (one request for every
    // mod not asked about today; none at all when everything was asked about already).
    if app.nexus.has_key() {
        let wanted: Vec<(String, u64)> = p.mods.iter().filter(|m| m.enabled && !library::is_local(m.mod_id)).map(|m| (p.game.clone(), m.mod_id)).collect();
        if let Ok(mut facts) = requirements::facts(&app.nexus, &app.data_dir, &wanted).await {
            // Also what Nexus Mods says about the requirements that aren't downloaded, to tell
            // ones that were taken down (hidden, removed) from ones to get. Kept a day too.
            let have: HashSet<u64> = mods.iter().filter(|m| m.game == p.game).map(|m| m.mod_id).collect();
            let reqs: Vec<(String, u64)> = facts.values().flat_map(|f| f.requires.iter()).filter(|q| q.mod_id != 0 && !q.other_game && !have.contains(&q.mod_id)).map(|q| (p.game.clone(), q.mod_id)).collect::<HashSet<_>>().into_iter().collect();
            if !reqs.is_empty() {
                if let Ok(more) = requirements::facts(&app.nexus, &app.data_dir, &reqs).await {
                    facts.extend(more);
                }
            }
            let updated = list_games(&app).into_iter().find(|g| g.domain == p.game).and_then(|g| g.last_updated);
            check::add_requirements(&mut report, &p, &mods, &facts, updated);
        }
    }
    // What the game's logs said the last time it started with this profile in it (only logs
    // written after the mods went in).
    let placed = deploy::deployed(&app.data_dir, &p.game);
    if placed.profile_id.as_deref() == Some(p.id.as_str()) {
        // The files the profile would put in, against the ones in the game now.
        let now: HashSet<String> = placed.files.iter().map(|f| f.to_lowercase()).collect();
        let want: HashSet<&String> = report.owners.keys().collect();
        report.out_of_date = want.len() != now.len() || want.iter().any(|f| !now.contains(f.as_str()));
        // Same files, but is each one the copy from the mod that wins it now? (A changed order
        // or a new file of a mod changes that without changing any file name.)
        if !report.out_of_date {
            if let Some(root) = list_games(&app).into_iter().find(|g| g.domain == p.game).and_then(|g| g.path) {
                let sources: Vec<(String, PathBuf)> = report.sources.values().cloned().collect();
                report.out_of_date = tokio::task::spawn_blocking(move || {
                    use std::os::unix::fs::MetadataExt;
                    sources.iter().any(|(rel, from)| match (fs::metadata(Path::new(&root).join(rel)), fs::metadata(from)) {
                        (Ok(g), Ok(m)) if g.dev() == m.dev() => g.ino() != m.ino(),
                        (Ok(g), Ok(m)) => g.len() != m.len(),
                        _ => false,
                    })
                })
                .await
                .unwrap_or(false);
            }
        }
        if let Some(root) = list_games(&app).into_iter().find(|g| g.domain == p.game).and_then(|g| g.path) {
            let manifest = app.data_dir.join("deployed").join(format!("{}.json", library::safe_name(&p.game)));
            let since = fs::metadata(&manifest).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
            let owners = std::mem::take(&mut report.owners);
            let sources = std::mem::take(&mut report.sources);
            let prefix = list_games(&app).into_iter().find(|g| g.domain == p.game).and_then(|g| g.proton_prefix).map(PathBuf::from);
            report.game_log = tokio::task::spawn_blocking(move || gamelogs::read(Path::new(&root), &owners, &sources, prefix.as_deref(), since)).await.ok().flatten().filter(|l| !l.issues.is_empty() || !l.assets.is_empty() || l.crash.is_some() || l.smapi.as_ref().is_some_and(|s| !s.broken.is_empty() || !s.updates.is_empty()));
        }
    }
    // Under Proton, loader DLLs (RED4ext's winmm.dll, …) only run when the launch options say so.
    if let Some(g) = list_games(&app).into_iter().find(|g| g.domain == p.game && g.domain == "stardewvalley") {
        if let (Some(app_id), Some(path)) = (g.steam_app_id, g.path.clone()) {
            report.launch_fix = tokio::task::spawn_blocking(move || launchopts::smapi_fix(&launchopts::launch_options(app_id).unwrap_or_default(), Path::new(&path))).await.ok().flatten();
        }
    }
    if let Some(g) = list_games(&app).into_iter().find(|g| g.domain == p.game) {
        if let (Some(app_id), Some(_), Some(path)) = (g.steam_app_id, g.proton_prefix.as_ref(), g.path.clone()) {
            let dll_fix = tokio::task::spawn_blocking(move || {
                let dlls = launchopts::proxy_dlls(Path::new(&path));
                if dlls.is_empty() {
                    return None;
                }
                launchopts::fix(&launchopts::launch_options(app_id).unwrap_or_default(), &dlls)
            })
            .await
            .ok()
            .flatten();
            if dll_fix.is_some() {
                report.launch_fix = dll_fix;
            }
        }
    }
    let blocking = |r: &check::Report| r.plugins.len() + usize::from(r.launch_fix.is_some()) + r.game_log.as_ref().filter(|l| !l.before_change).map(|l| l.issues.len()).unwrap_or(0) + r.needs.iter().filter(|n| matches!(n.kind.as_str(), "notDownloaded" | "notInProfile" | "switchedOff")).count();
    let before = app.checks.lock().unwrap().insert(p.id.clone(), report.clone());
    activity::tell("checks:changed");
    if let Some(b) = before {
        let (was, now) = (blocking(&b), blocking(&report));
        if now > was {
            activity::add("worse", format!("“{}” has {} to fix", p.name, if now == 1 { "a problem".to_string() } else { format!("{now} problems") }), Some(format!("Its check found {} more than before. Open it to see them, or use Fix for me.", now - was)), Some(&p.game), Some("profiles"), Some(&p.id));
        }
    }
    Ok(report)
}

/// A settings file a profile's mods have, and whether the profile has its own version of it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingFile {
    path: String,
    mod_id: Option<u64>,
    mod_name: Option<String>,
    /// The profile has its own (changed in the game or here).
    own: bool,
}

/// Each settings file of the profile's switched-on mods: path → (mod number, name, mod's own copy).
fn setting_sources(app: &App, p: &deploy::Profile) -> std::collections::BTreeMap<String, (u64, String, PathBuf)> {
    let folder = game_target(app, &p.game).map(|t| t.1).unwrap_or_else(|_| deploy::default_mod_folder(&p.game).unwrap_or("").to_string());
    let mods = library::list(&lib_dir(app));
    let mut out = std::collections::BTreeMap::new();
    for pm in p.mods.iter().filter(|m| m.enabled) {
        let Some(m) = mods.iter().find(|m| m.game == p.game && m.mod_id == pm.mod_id) else { continue };
        let Some(f) = m.files.iter().find(|f| f.file_id == pm.file_id) else { continue };
        let Ok(dir) = deploy::unpacked_dir(m, f) else { continue };
        for (rel, from) in deploy::entries(&dir, &folder) {
            if modsettings::is_setting(&rel) {
                out.insert(rel, (m.mod_id, m.name.clone(), from));
            }
        }
    }
    out
}

#[tauri::command]
async fn profile_settings(app: State<'_, App>, id: String) -> Res<Vec<SettingFile>> {
    let p = find_profile(&app, &id)?;
    {
        let sources = setting_sources(&app, &p);
        let own = modsettings::kept(&app.data_dir, &p.id);
        let mut out: Vec<SettingFile> = sources.iter().map(|(path, (id, name, _))| SettingFile { path: path.clone(), mod_id: Some(*id), mod_name: Some(name.clone()), own: own.contains_key(path) }).collect();
        // Ones mods made while the game ran.
        for path in own.keys().filter(|k| !sources.contains_key(*k)) {
            out.push(SettingFile { path: path.clone(), mod_id: None, mod_name: None, own: true });
        }
        out.sort_by_key(|a| a.path.to_lowercase());
        Ok(out)
    }
}

/// The file in the game, when this profile is in it now.
fn live_setting(app: &App, p: &deploy::Profile, path: &str) -> Option<PathBuf> {
    let d = deploy::deployed(&app.data_dir, &p.game);
    if d.profile_id.as_deref() != Some(p.id.as_str()) || !d.files.iter().any(|f| f == path) {
        return None;
    }
    let root = PathBuf::from(list_games(app).into_iter().find(|g| g.domain == p.game)?.path?);
    let rel = Path::new(path);
    rel.components().all(|c| matches!(c, std::path::Component::Normal(_))).then(|| root.join(rel))
}

#[tauri::command]
async fn profile_setting_read(app: State<'_, App>, id: String, path: String) -> Res<String> {
    let p = find_profile(&app, &id)?;
    let own = modsettings::kept(&app.data_dir, &p.id);
    let file = match live_setting(&app, &p, &path) {
        Some(f) if f.is_file() => f,
        _ => match own.get(&path) {
            Some(f) => f.clone(),
            None => setting_sources(&app, &p).remove(&path).map(|s| s.2).ok_or("That isn't one of this profile's settings files.")?,
        },
    };
    if fs::metadata(&file).map(|m| m.len() > 2 * 1024 * 1024).unwrap_or(false) {
        return Err("That file is too big to edit here.".into());
    }
    fs::read(&file).map(|b| String::from_utf8_lossy(&b).into_owned()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn profile_setting_write(app: State<'_, App>, id: String, path: String, text: String) -> Res<()> {
    let p = find_profile(&app, &id)?;
    let known = modsettings::kept(&app.data_dir, &p.id).contains_key(&path) || setting_sources(&app, &p).contains_key(&path);
    if !known || !modsettings::is_setting(&path) {
        return Err("That isn't one of this profile's settings files.".into());
    }
    let dest = modsettings::store(&app.data_dir, &p.id).join(Path::new(&path));
    if !Path::new(&path).components().all(|c| matches!(c, std::path::Component::Normal(_))) {
        return Err("That isn't one of this profile's settings files.".into());
    }
    fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
    settings::write_atomic(&dest, text.as_bytes())?;
    // In the game now: change it there too, so the next start uses it.
    if let Some(live) = live_setting(&app, &p, &path) {
        fs::write(&live, text.as_bytes()).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Back to the mod's own settings file.
#[tauri::command]
async fn profile_setting_reset(app: State<'_, App>, id: String, path: String) -> Res<()> {
    let p = find_profile(&app, &id)?;
    modsettings::forget(&app.data_dir, &p.id, &path)?;
    if let (Some(live), Some((_, _, from))) = (live_setting(&app, &p, &path), setting_sources(&app, &p).remove(&path)) {
        fs::copy(&from, &live).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// The last time this profile was played, when the game closed right after starting: what
/// changed since it last ran fine.
#[tauri::command]
fn profile_last_run(app: State<'_, App>, id: String) -> Res<Option<runs::Trouble>> {
    let p = find_profile(&app, &id)?;
    let names: HashMap<u64, String> = library::list(&lib_dir(&app)).into_iter().filter(|m| m.game == p.game).map(|m| (m.mod_id, m.name)).collect();
    let now: Vec<(u64, u64)> = p.mods.iter().filter(|m| m.enabled).map(|m| (m.mod_id, m.file_id)).collect();
    Ok(runs::trouble(&runs::list(&app.data_dir, &p.id), &now, &names))
}

#[tauri::command]
fn profile_checks(app: State<'_, App>) -> HashMap<String, check::Report> {
    app.checks.lock().unwrap().clone()
}

async fn apply_profile(app: &App, id: &str) -> Res<deploy::Applied> {
    let _turn = app.profiles.lock().await;
    let mut p = find_profile(app, id)?;
    let rules = p.rules.clone();
    order::keep(&mut p.mods, &rules);
    let (root, folder, prefix, g) = game_target(app, &p.game)?;
    // Saves first: this profile's own, or the shared ones. A problem stops here, before any
    // mods move, so saves and mods never get out of step.
    if let Some(live) = g.save_folder.as_deref().map(PathBuf::from) {
        if p.own_saves || saves::owner(&live) != saves::SHARED {
            let want = if p.own_saves { p.id.clone() } else { saves::SHARED.to_string() };
            tokio::task::spawn_blocking(move || saves::switch(&live, &want)).await.map_err(|e| e.to_string())??;
        }
    } else if p.own_saves {
        return Err("This profile keeps its own saves, but Zom's Nexus doesn't know where this game's saves are. Set its save folder on the Games page.".into());
    }
    let lib = lib_dir(app);
    let data = app.data_dir.clone();
    let game = p.game.clone();
    let p2 = p.clone();
    let copy = app.settings.lock().unwrap().copy_games.contains(&p.game);
    let data2 = app.data_dir.clone();
    let applied = tokio::task::spawn_blocking(move || {
        auto_install(&lib, &data2, &p2, &root, &folder);
        let mods = library::list(&lib);
        let applied = deploy::apply(&data, &deploy::Target { game: &game, root: &root, mod_folder: &folder, prefix: prefix.as_deref(), copy }, &p2, &mods)?;
        // Baldur's Gate 3: .pak mods go in its Mods folder in the prefix, and on its mod list.
        if game == bg3::GAME {
            if let Some(pfx) = prefix.as_deref() {
                let dirs: Vec<PathBuf> = p2
                    .mods
                    .iter()
                    .filter(|pm| pm.enabled)
                    .filter_map(|pm| {
                        let m = mods.iter().find(|m| m.game == game && m.mod_id == pm.mod_id)?;
                        let f = m.files.iter().find(|f| f.file_id == pm.file_id)?;
                        deploy::unpacked_dir(m, f).ok()
                    })
                    .collect();
                let (_, unknown) = bg3::apply(&data, pfx, &dirs, copy || !deploy::same_drive(&lib, pfx))?;
                if !unknown.is_empty() {
                    log::warn(&format!("Baldur's Gate 3: no mod details in {} (put in, but not on the mod list)", unknown.join(", ")));
                }
            }
        }
        Ok::<_, String>(applied)
    })
    .await
    .map_err(|e| e.to_string())??;
    if let Err(e) = history::note(&app.data_dir, &p, queue::now()) {
        log::warn(&format!("Couldn't note a restore point: {e}"));
    }
    Ok(applied)
}

/// Mods in the profile with an installer whose options weren't chosen, but whose collection's
/// curator chose them: install them that way now. A problem leaves the mod as it was (the check
/// still says it has an installer).
fn auto_install(lib: &Path, data: &Path, p: &deploy::Profile, game_root: &Path, mod_folder: &str) {
    let suggested = fomod::suggested(data);
    let mods_dir = game_root.join(mod_folder);
    let state = |file: &str| -> &'static str { if find_nocase(&mods_dir, file).is_some() { "Active" } else { "Missing" } };
    // Stardew Valley's Windows build (under Proton) needs SMAPI's Windows files.
    let windows = game_root.join("Stardew Valley.exe").is_file();
    for pm in p.mods.iter().filter(|m| m.enabled) {
        // SMAPI's installer: unpack its files for the game's own folder.
        if p.game == "stardewvalley" {
            if let Ok(dir) = library::find(lib, &p.game, pm.mod_id) {
                if let Some(meta) = library::read_meta(&dir) {
                    // Like SMAPI's own installer: SMAPI runs on the game's .NET, described by the
                    // game's deps.json, copied under SMAPI's name (again each time, for game updates).
                    if let Some(i) = meta.files.iter().find(|f| f.file_id == pm.file_id).and_then(|f| f.installed.as_ref()) {
                        let built = dir.join(library::safe_name(&i.folder));
                        if built.join(deploy::GAME_ROOT_MARK).is_file() {
                            let (from, to) = (game_root.join("Stardew Valley.deps.json"), built.join("StardewModdingAPI.deps.json"));
                            if from.is_file() && fs::read(&from).ok() != fs::read(&to).ok() {
                                if let Err(e) = fs::remove_file(&to).or(Ok::<(), std::io::Error>(())).and_then(|_| fs::copy(&from, &to).map(|_| ())) {
                                    log::warn(&format!("Couldn't copy the game's deps.json for SMAPI: {e}"));
                                }
                            }
                        }
                    }
                    if let Some(f) = meta.files.iter().find(|f| f.file_id == pm.file_id && f.installed.is_none()) {
                        if let Ok(raw) = deploy::unpacked_dir(&meta, f) {
                            let base = library::read_meta(&dir).and_then(|m| m.files.into_iter().find(|x| x.file_id == pm.file_id)).and_then(|x| x.unpacked).unwrap_or_else(|| f.name.clone());
                            match deploy::smapi_build(&raw, &dir, &base, windows) {
                                Some(Ok(i)) => match library::set_installed(&dir, pm.file_id, Some(i.clone())) {
                                    Ok(()) => {
                                        let to = dir.join(library::safe_name(&i.folder)).join("StardewModdingAPI.deps.json");
                                        let _ = fs::copy(game_root.join("Stardew Valley.deps.json"), to);
                                        log::info(&format!("Unpacked SMAPI ({}) for the game folder", if windows { "Windows build" } else { "Linux build" }))
                                    }
                                    Err(e) => log::warn(&format!("Couldn't note SMAPI's files: {e}")),
                                },
                                Some(Err(e)) => log::warn(&format!("Couldn't unpack SMAPI's installer: {e}")),
                                None => {}
                            }
                        }
                    }
                }
            }
        }
        let Some(choices) = suggested.get(&format!("{}:{}:{}", p.game, pm.mod_id, pm.file_id)) else { continue };
        let Ok(dir) = library::find(lib, &p.game, pm.mod_id) else { continue };
        let Some(meta) = library::read_meta(&dir) else { continue };
        let Some(f) = meta.files.iter().find(|f| f.file_id == pm.file_id) else { continue };
        if f.installed.is_some() {
            continue;
        }
        let raw = match &f.unpacked {
            Some(u) => dir.join(library::safe_name(u)),
            None => match deploy::unpacked_dir(&meta, f) {
                Ok(d) => d,
                Err(_) => continue,
            },
        };
        let Some(root) = fomod::root(&raw) else { continue };
        let base = library::read_meta(&dir).and_then(|m| m.files.into_iter().find(|x| x.file_id == pm.file_id)).and_then(|x| x.unpacked).unwrap_or_else(|| f.name.clone());
        let done = fomod::read(&root, &state).and_then(|inst| {
            let fitted = fomod::fit(&inst, choices);
            fomod::install(&inst, &fitted, &dir, &base)
        });
        match done.and_then(|i| library::set_installed(&dir, pm.file_id, Some(i))) {
            Ok(()) => log::info(&format!("Installed {} with its collection's installer choices", meta.name)),
            Err(e) => log::warn(&format!("Couldn't install {} with its collection's choices: {e}", meta.name)),
        }
    }
}

/// What went into a game each time it changed, newest first.
#[tauri::command]
fn profile_history(app: State<'_, App>, game: String) -> Vec<history::Point> {
    history::list(&app.data_dir, &game)
}

/// Note why something changed, on a restore point.
#[tauri::command]
fn profile_history_note(app: State<'_, App>, game: String, at: u64, note: String) -> Res<()> {
    history::set_note(&app.data_dir, &game, at, &note)
}

/// Make a profile the way it was at a restore point (making it again if it was deleted). The
/// page puts it in the game afterwards.
#[tauri::command]
async fn profile_restore(app: State<'_, App>, game: String, at: u64) -> Res<deploy::Profile> {
    let point = history::list(&app.data_dir, &game).into_iter().find(|x| x.at == at).ok_or("That restore point isn't there any more.")?;
    let _turn = app.profiles.lock().await;
    let mut all = profiles::load(&app.config_dir);
    let p = match all.iter_mut().find(|p| p.id == point.profile_id) {
        Some(p) => {
            p.mods = point.mods.clone();
            p.rules = point.rules.clone();
            p.clone()
        }
        None => {
            let p = deploy::Profile { id: point.profile_id.clone(), name: format!("{} (restored)", point.profile_name), game: game.clone(), mods: point.mods.clone(), collection: None, manual_requirements: true, new_downloads: false, own_saves: false, base: None, addons: vec![], rules: point.rules.clone(), note: None, ts_pack: None };
            all.push(p.clone());
            p
        }
    };
    profiles::save(&app.config_dir, &all)?;
    Ok(p)
}

#[tauri::command]
async fn profile_apply(app: State<'_, App>, id: String) -> Res<deploy::Applied> {
    apply_profile(&app, &id).await.inspect_err(|e| log::error(&format!("Putting profile {id} in the game: {e}")))
}

/// Take a game's mods out, putting its own files back.
#[tauri::command]
async fn profile_clear(app: State<'_, App>, game: String) -> Res<()> {
    let _turn = app.profiles.lock().await;
    clear_game(&app, game).await
}

/// Every game's mods out, its own files and usual saves back: before uninstalling Zom's Nexus,
/// say. Games running now are left alone. Gives the names of the games done and the ones left.
#[tauri::command]
async fn games_clear_all(app: State<'_, App>) -> Res<(Vec<String>, Vec<String>)> {
    let _turn = app.profiles.lock().await;
    let (mut done, mut left) = (vec![], vec![]);
    for g in list_games(&app) {
        let placed = deploy::deployed(&app.data_dir, &g.domain);
        if placed.files.is_empty() && placed.backups.is_empty() {
            continue;
        }
        let running = g.path.clone().map(PathBuf::from).map(|p| activity::running(&p)).unwrap_or(false);
        if running {
            left.push(format!("{} (running now)", g.name));
            continue;
        }
        match clear_game(&app, g.domain.clone()).await {
            Ok(()) => done.push(g.name.clone()),
            Err(e) => left.push(format!("{} ({e})", g.name)),
        }
    }
    Ok((done, left))
}

async fn clear_game(app: &App, game: String) -> Res<()> {
    let (root, _, _, g) = game_target(app, &game)?;
    // The shared saves come back with the game's own files.
    if let Some(live) = g.save_folder.as_deref().map(PathBuf::from) {
        if saves::owner(&live) != saves::SHARED {
            tokio::task::spawn_blocking(move || saves::switch(&live, saves::SHARED)).await.map_err(|e| e.to_string())??;
        }
    }
    let data = app.data_dir.clone();
    let prefix = g.proton_prefix.clone().map(PathBuf::from);
    tokio::task::spawn_blocking(move || {
        if game == bg3::GAME {
            if let Some(pfx) = prefix.as_deref() {
                bg3::clear(&data, pfx)?;
            }
        }
        deploy::clear(&data, &game, &root)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn launch(handle: &AppHandle, g: &games::Game) -> Res<()> {
    if let Some(cmd) = &g.launch {
        let dir = g.path.clone().unwrap_or_else(|| ".".into());
        host::command("sh").arg("-c").arg(cmd).current_dir(dir).spawn().map_err(|e| format!("Couldn't start the game: {e}"))?;
        return Ok(());
    }
    let app_id = g.steam_app_id.ok_or("Add a start command for this game on the Games page, or start it yourself.")?;
    handle.opener().open_url(format!("steam://rungameid/{app_id}"), None::<&str>).map_err(|e| format!("Couldn't ask Steam to start the game: {e}"))
}

#[tauri::command]
async fn game_launch(handle: AppHandle, app: State<'_, App>, domain: String) -> Res<()> {
    let g = list_games(&app).into_iter().find(|g| g.domain == domain).ok_or("That game isn't on the Games page.")?;
    if let Err(e) = launch(&handle, &g) {
        activity::set_state(&domain, "failed", None, Some(e.clone()));
        return Err(e);
    }
    // Started without a profile: its time isn't counted toward one, but the sidebar still shows it.
    match g.path.clone() {
        Some(path) => activity::watch(app.data_dir.clone(), String::new(), String::new(), domain, g.name.clone(), PathBuf::from(path)),
        None => activity::set_state(&domain, "starting", None, None),
    }
    Ok(())
}

/// Put the profile's mods in, then start the game.
#[tauri::command]
async fn profile_play(handle: AppHandle, app: State<'_, App>, id: String) -> Res<deploy::Applied> {
    let p = find_profile(&app, &id)?;
    activity::set_state(&p.game, "applying", Some(&p.name), None);
    let result = async {
        let applied = apply_profile(&app, &id).await?;
        let (_, _, _, g) = game_target(&app, &p.game)?;
        // A snapshot of the saves about to be played, when asked for, before the game can
        // write to them (never stops Play).
        let snap = app.settings.lock().unwrap().snapshot_before_play;
        if let (true, Some(live)) = (snap, g.save_folder.clone().map(PathBuf::from)) {
            let (data, game, owner, label) = (app.data_dir.clone(), p.game.clone(), saves::owner(&live), format!("Before playing “{}”", p.name));
            let _ = tokio::task::spawn_blocking(move || snapshots::take(&data, &game, &live, &owner, &label, true)).await;
        }
        launch(&handle, &g)?;
        Ok::<_, String>((applied, g))
    }
    .await;
    match result {
        Ok((applied, g)) => {
            runs::begin(&p.id, find_profile(&app, &id).unwrap_or(p.clone()).mods.iter().filter(|m| m.enabled).map(|m| (m.mod_id, m.file_id)).collect());
            match g.path.clone() {
                Some(path) => activity::watch(app.data_dir.clone(), p.id.clone(), p.name.clone(), p.game.clone(), g.name.clone(), PathBuf::from(path)),
                None => activity::set_state(&p.game, "starting", Some(&p.name), None),
            }
            Ok(applied)
        }
        Err(e) => {
            activity::set_state(&p.game, "failed", Some(&p.name), Some(e.clone()));
            Err(e)
        }
    }
}

/// Saves in the game's save folder that are copies of the shared saves (put there by Steam
/// Cloud), when a profile with its own saves is in.
#[tauri::command]
async fn saves_copies(app: State<'_, App>, game: String) -> Res<usize> {
    let Some(live) = list_games(&app).into_iter().find(|g| g.domain == game).and_then(|g| g.save_folder) else { return Ok(0) };
    tokio::task::spawn_blocking(move || saves::copies(Path::new(&live)).len()).await.map_err(|e| e.to_string())
}

/// Move those copies aside (never deleted). Not while the game is running.
#[tauri::command]
async fn saves_move_copies(app: State<'_, App>, game: String) -> Res<usize> {
    let g = list_games(&app).into_iter().find(|g| g.domain == game).ok_or("That game isn't on the Games page.")?;
    let live = g.save_folder.clone().ok_or("This game's save folder isn't set.")?;
    let path = g.path.clone();
    tokio::task::spawn_blocking(move || {
        if path.map(|p| activity::running(Path::new(&p))).unwrap_or(false) {
            return Err(format!("{} is running. Quit it first, so no save is moved while the game writes it.", g.name));
        }
        saves::move_copies(Path::new(&live))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Where each game is (starting, running, …) since the app started.
#[tauri::command]
fn game_states() -> Vec<activity::GameState> {
    activity::game_states()
}

#[tauri::command]
async fn profile_export(app: State<'_, App>, id: String, path: String) -> Res<()> {
    let pack = pack_of(&app, &id)?;
    let mut path = PathBuf::from(path);
    if path.extension().is_none() {
        path.set_extension("json");
    }
    fs::write(&path, serde_json::to_vec_pretty(&pack).map_err(|e| e.to_string())?).map_err(|e| format!("Couldn't save the mod pack: {e}"))
}

/// The start of every share code, so a pasted one is recognised (and a future format can differ).
const SHARE: &str = "ZN1-";

/// A profile as a share code: its mod pack (Nexus Mods numbers and names only), packed small
/// into text to paste in a message. Opening it works like opening the pack file.
#[tauri::command]
fn profile_share_code(app: State<'_, App>, id: String) -> Res<String> {
    use base64::Engine;
    use std::io::Write;
    let pack = pack_of(&app, &id)?;
    let mut z = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
    z.write_all(&serde_json::to_vec(&pack).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(format!("{SHARE}{}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(z.finish().map_err(|e| e.to_string())?)))
}

/// Open a pasted share code like a mod pack.
#[tauri::command]
async fn profile_import_code(app: State<'_, App>, code: String) -> Res<PackImported> {
    use base64::Engine;
    use std::io::Read;
    let code: String = code.split_whitespace().collect();
    let body = code.strip_prefix(SHARE).ok_or("That isn't a Zom's Nexus share code (they start with ZN1-).")?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(body).map_err(|_| "That share code is incomplete or changed. Copy it again.".to_string())?;
    let mut text = String::new();
    // At most 5 MB unpacked, like a pack file.
    flate2::read::DeflateDecoder::new(&bytes[..]).take(5_000_000).read_to_string(&mut text).map_err(|_| "That share code is incomplete or changed. Copy it again.".to_string())?;
    import_pack(&app, &text).await
}

fn pack_of(app: &App, id: &str) -> Res<profiles::Pack> {
    let p = find_profile(app, id)?;
    let mods = library::list(&lib_dir(app));
    // A pack lists Nexus Mods numbers; mods from your own files can't be fetched by others.
    let mut p = p;
    p.mods.retain(|m| !library::is_local(m.mod_id));
    let pack = profiles::to_pack(&p, |m, f| {
        let sm = mods.iter().find(|x| x.game == p.game && x.mod_id == m);
        (sm.map(|x| x.name.clone()).unwrap_or_default(), sm.and_then(|x| x.files.iter().find(|y| y.file_id == f)).map(|y| y.file_name.clone()).unwrap_or_default())
    });
    Ok(pack)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PackImported {
    profile: deploy::Profile,
    queued: usize,
    /// Something worth saying about where an add-on pack went.
    note: Option<String>,
}

async fn ts_game_of(app: &App, game: &str) -> Res<Option<thunderstore::TsGame>> {
    let g = list_games(app).into_iter().find(|g| g.domain == game).ok_or("That game isn't on the Games page.")?;
    thunderstore::game(app.nexus.http(), &app.cache_dir, g.steam_app_id, &g.domain, &g.name).await
}

/// The game on Thunderstore, if it's there.
#[tauri::command]
async fn ts_game(app: State<'_, App>, game: String) -> Res<Option<thunderstore::TsGame>> {
    ts_game_of(&app, &game).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TsResults {
    results: Vec<thunderstore::Found>,
    total: usize,
}

/// Search a game's Thunderstore mods (the list is fetched at most once an hour).
#[tauri::command]
async fn ts_search(app: State<'_, App>, game: String, q: String, sort: String, offset: usize) -> Res<TsResults> {
    let g = ts_game_of(&app, &game).await?.ok_or("This game isn't on Thunderstore.")?;
    let list = thunderstore::listing(app.nexus.http(), &app.cache_dir, &g.community).await?;
    let adult = app.settings.lock().unwrap().adult_content != "hide";
    let (results, total) = thunderstore::search(&list, &q, &sort, adult, offset, 30);
    Ok(TsResults { results, total })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TsAdded {
    /// Downloaded now (names).
    added: Vec<String>,
    /// In the library already.
    had: usize,
    /// Requirements Thunderstore doesn't have.
    missing: Vec<String>,
    profile: Option<deploy::Profile>,
}

/// Download a Thunderstore mod and what it needs into the library. With `profile`, they go into
/// that profile too; a mod pack without one becomes a new profile.
#[tauri::command]
async fn ts_add(handle: AppHandle, app: State<'_, App>, game: String, full_name: String, profile: Option<String>) -> Res<TsAdded> {
    let g = ts_game_of(&app, &game).await?.ok_or("This game isn't on Thunderstore.")?;
    if !g.supported {
        return Err(format!("{} uses {} on Thunderstore, which Zom's Nexus can't put in yet (BepInEx games work).", g.name, g.loader));
    }
    let list = thunderstore::listing(app.nexus.http(), &app.cache_dir, &g.community).await?;
    let (chain, missing) = thunderstore::resolve(&list, &full_name, None);
    let top = chain.last().map(|(p, _)| (*p).clone()).ok_or("That mod isn't on Thunderstore any more.")?;
    let lib = lib_dir(&app);
    let keep = app.settings.lock().unwrap().keep_archives;
    let (mut added, mut had) = (vec![], 0);
    let n = chain.len();
    for (i, (p, v)) in chain.iter().enumerate() {
        let _ = handle.emit("thunderstore:progress", serde_json::json!({ "name": p.name.replace('_', " "), "at": i + 1, "of": n }));
        if thunderstore::fetch_into(app.nexus.http(), &lib, &game, &g, p, v, keep, queue::now()).await? {
            added.push(p.name.replace('_', " "));
        } else {
            had += 1;
        }
    }
    let _ = handle.emit("library:changed", 0);
    let picks: Vec<deploy::ProfileMod> = chain.iter().map(|(p, v)| deploy::ProfileMod { mod_id: thunderstore::mod_id(&p.full_name), file_id: thunderstore::file_id(&format!("{}-{}", p.full_name, v.number)), enabled: true }).collect();
    let modpack = top.categories.iter().any(|c| c.eq_ignore_ascii_case("modpacks"));
    let mut made = None;
    if profile.is_some() || modpack {
        let _turn = app.profiles.lock().await;
        let mut all = profiles::load(&app.config_dir);
        match profile.as_deref().and_then(|id| all.iter_mut().find(|p| p.id == id)) {
            Some(p) => {
                for m in &picks {
                    match p.mods.iter_mut().find(|x| x.mod_id == m.mod_id) {
                        Some(x) => {
                            x.file_id = m.file_id;
                            x.enabled = true;
                        }
                        None => p.mods.push(m.clone()),
                    }
                }
                made = Some(p.clone());
            }
            None => {
                let mut mods = picks.clone();
                // A pack is its requirements: leave out the empty pack itself.
                mods.retain(|m| m.mod_id != thunderstore::mod_id(&top.full_name));
                let p = deploy::Profile { id: format!("p{}{}", queue::now(), all.len()), name: top.name.replace('_', " "), game: game.clone(), mods, collection: None, manual_requirements: false, new_downloads: false, own_saves: false, base: None, addons: vec![], rules: vec![], note: Some(format!("Thunderstore mod pack {} by {}.", top.name.replace('_', " "), top.owner)), ts_pack: chain.last().map(|(p, v)| (p.full_name.clone(), v.number.clone())) };
                all.push(p.clone());
                made = Some(p);
            }
        }
        profiles::save(&app.config_dir, &all)?;
    }
    Ok(TsAdded { added, had, missing, profile: made })
}

/// tModLoader's mods (from the Steam Workshop) and mod packs, when tModLoader is installed.
#[tauri::command]
async fn tml_state() -> Res<Option<tml::TmlState>> {
    tokio::task::spawn_blocking(tml::state).await.map_err(|e| e.to_string())
}

fn tml_dir() -> Res<PathBuf> {
    tml::mods_dir().ok_or_else(|| "tModLoader's Mods folder isn't here. Start tModLoader once from Steam, then try again.".to_string())
}

fn tml_running() -> bool {
    games::steam_library_dirs().iter().any(|l| activity::running(&l.join("steamapps/common/tModLoader")))
}

/// Switch on exactly these tModLoader mods.
#[tauri::command]
fn tml_set(mods: Vec<String>) -> Res<()> {
    if tml_running() {
        return Err("tModLoader is running: close it first (it rewrites the list when it closes).".into());
    }
    tml::set_enabled(&tml_dir()?, &mods)
}

/// Save a tModLoader mod pack (tModLoader's own Mod Packs menu lists it too).
#[tauri::command]
fn tml_save_pack(name: String, mods: Vec<String>) -> Res<()> {
    tml::save_pack(&tml_dir()?, &name, &mods)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TsPackUpdate {
    profile_id: String,
    profile_name: String,
    pack: String,
    have: String,
    newest: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TsUpdates {
    mods: Vec<thunderstore::TsUpdate>,
    packs: Vec<TsPackUpdate>,
}

/// Newer versions of a game's Thunderstore mods, and of the mod packs its profiles came from.
/// Free: it reads the list already fetched (at most once an hour).
#[tauri::command]
async fn ts_updates(app: State<'_, App>, game: String) -> Res<TsUpdates> {
    let Some(g) = ts_game_of(&app, &game).await? else { return Ok(TsUpdates { mods: vec![], packs: vec![] }) };
    let stored: Vec<(String, String)> = library::list(&lib_dir(&app))
        .into_iter()
        .filter(|m| m.game == game)
        .filter_map(|m| {
            let full = m.source.as_deref()?.strip_prefix("thunderstore:")?.to_string();
            // The newest version you have of it.
            let have = m.files.iter().filter_map(|f| f.version.clone()).max_by(|a, b| thunderstore::ver_cmp(a, b))?;
            Some((full, have))
        })
        .collect();
    let packs_of: Vec<deploy::Profile> = profiles::load(&app.config_dir).into_iter().filter(|p| p.game == game && p.ts_pack.is_some()).collect();
    if stored.is_empty() && packs_of.is_empty() {
        return Ok(TsUpdates { mods: vec![], packs: vec![] });
    }
    let list = thunderstore::listing(app.nexus.http(), &app.cache_dir, &g.community).await?;
    let mods = thunderstore::updates(&list, &stored);
    let packs = packs_of
        .into_iter()
        .filter_map(|p| {
            let (full, have) = p.ts_pack.clone()?;
            let u = thunderstore::updates(&list, &[(full.clone(), have.clone())]).pop()?;
            Some(TsPackUpdate { profile_id: p.id, profile_name: p.name, pack: u.name, have, newest: u.newest })
        })
        .collect();
    Ok(TsUpdates { mods, packs })
}

/// Download the newest versions of these Thunderstore mods (and what they need now), and move
/// every profile of the game that has them onto the new ones.
#[tauri::command]
async fn ts_update(handle: AppHandle, app: State<'_, App>, game: String, full_names: Vec<String>) -> Res<usize> {
    let g = ts_game_of(&app, &game).await?.ok_or("This game isn't on Thunderstore.")?;
    let list = thunderstore::listing(app.nexus.http(), &app.cache_dir, &g.community).await?;
    let lib = lib_dir(&app);
    let keep = app.settings.lock().unwrap().keep_archives;
    let mut newest: Vec<(u64, u64)> = vec![];
    let mut needs: Vec<(u64, u64, u64)> = vec![]; // (requirement mod, file, for mod)
    for (i, full) in full_names.iter().enumerate() {
        let (chain, _) = thunderstore::resolve(&list, full, None);
        for (p, v) in &chain {
            let _ = handle.emit("thunderstore:progress", serde_json::json!({ "name": p.name.replace('_', " "), "at": i + 1, "of": full_names.len() }));
            thunderstore::fetch_into(app.nexus.http(), &lib, &game, &g, p, v, keep, queue::now()).await?;
            let ids = (thunderstore::mod_id(&p.full_name), thunderstore::file_id(&format!("{}-{}", p.full_name, v.number)));
            if p.full_name.eq_ignore_ascii_case(full) {
                newest.push(ids);
            } else {
                needs.push((ids.0, ids.1, thunderstore::mod_id(full)));
            }
        }
    }
    let _ = handle.emit("library:changed", 0);
    let _turn = app.profiles.lock().await;
    let mut all = profiles::load(&app.config_dir);
    let mut moved = 0;
    for p in all.iter_mut().filter(|p| p.game == game) {
        let mut touched = false;
        for (m, f) in &newest {
            if let Some(pm) = p.mods.iter_mut().find(|x| x.mod_id == *m && x.file_id != *f) {
                pm.file_id = *f;
                touched = true;
            }
        }
        // What an updated mod needs now, if the profile doesn't have it.
        for (m, f, for_mod) in &needs {
            if p.mods.iter().any(|x| x.mod_id == *for_mod) && !p.mods.iter().any(|x| x.mod_id == *m) {
                let at = p.mods.iter().position(|x| x.mod_id == *for_mod).unwrap_or(p.mods.len());
                p.mods.insert(at, deploy::ProfileMod { mod_id: *m, file_id: *f, enabled: true });
                touched = true;
            }
        }
        moved += usize::from(touched);
    }
    profiles::save(&app.config_dir, &all)?;
    Ok(moved)
}

/// Move a profile made from a Thunderstore mod pack onto the pack's newest version: its mods'
/// new versions, the ones it added, and the ones it dropped switched off (your own additions stay).
#[tauri::command]
async fn ts_update_pack(handle: AppHandle, app: State<'_, App>, id: String) -> Res<deploy::Profile> {
    let p = find_profile(&app, &id)?;
    let (full, have) = p.ts_pack.clone().ok_or("This profile wasn't made from a Thunderstore mod pack.")?;
    let g = ts_game_of(&app, &p.game).await?.ok_or("This game isn't on Thunderstore.")?;
    let list = thunderstore::listing(app.nexus.http(), &app.cache_dir, &g.community).await?;
    let (newest, now, gone) = thunderstore::pack_change(&list, &full, &have).ok_or("That mod pack isn't on Thunderstore any more.")?;
    let (chain, _) = thunderstore::resolve(&list, &full, Some(&newest));
    let lib = lib_dir(&app);
    let keep = app.settings.lock().unwrap().keep_archives;
    let n = chain.len();
    for (i, (pk, v)) in chain.iter().enumerate() {
        let _ = handle.emit("thunderstore:progress", serde_json::json!({ "name": pk.name.replace('_', " "), "at": i + 1, "of": n }));
        thunderstore::fetch_into(app.nexus.http(), &lib, &p.game, &g, pk, v, keep, queue::now()).await?;
    }
    let _ = handle.emit("library:changed", 0);
    let _turn = app.profiles.lock().await;
    let mut all = profiles::load(&app.config_dir);
    let q = all.iter_mut().find(|x| x.id == id).ok_or("That profile is gone.")?;
    for (m, f) in &now {
        match q.mods.iter_mut().find(|x| x.mod_id == *m) {
            Some(x) => x.file_id = *f,
            None => q.mods.push(deploy::ProfileMod { mod_id: *m, file_id: *f, enabled: true }),
        }
    }
    for m in &gone {
        if let Some(x) = q.mods.iter_mut().find(|x| x.mod_id == *m) {
            x.enabled = false;
        }
    }
    q.ts_pack = Some((full, newest));
    let out = q.clone();
    profiles::save(&app.config_dir, &all)?;
    Ok(out)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct R2Profile {
    name: String,
    path: String,
}

/// r2modman's profiles for this game on this computer.
#[tauri::command]
async fn r2_profiles(app: State<'_, App>, game: String) -> Res<Vec<R2Profile>> {
    let Some(g) = ts_game_of(&app, &game).await? else { return Ok(vec![]) };
    Ok(thunderstore::r2_profiles(&g).into_iter().map(|(name, p)| R2Profile { name, path: p.to_string_lossy().into_owned() }).collect())
}

/// Bring an r2modman profile across: from a profile code, or one of its folders here. Its
/// mods (at their versions, with what they need) and its config files, which become the new
/// profile's own settings.
#[tauri::command]
async fn r2_import(handle: AppHandle, app: State<'_, App>, game: String, code: Option<String>, folder: Option<String>) -> Res<TsAdded> {
    let g = ts_game_of(&app, &game).await?.ok_or("This game isn't on Thunderstore.")?;
    if !g.supported {
        return Err(format!("{} uses {} on Thunderstore, which Zom's Nexus can't put in yet.", g.name, g.loader));
    }
    let (name, list_text, configs): (String, String, Vec<(String, Vec<u8>)>) = match (code, folder) {
        (Some(c), _) => {
            let (text, files) = thunderstore::r2_code(app.nexus.http(), &c).await?;
            let n = text.lines().find_map(|l| l.strip_prefix("profileName:")).map(|n| n.trim().trim_matches('"').to_string()).unwrap_or_else(|| "From r2modman".into());
            (n, text, files)
        }
        (None, Some(f)) => {
            // Only one of r2modman's own profile folders for this game.
            let known = thunderstore::r2_profiles(&g);
            let (n, dir) = known.into_iter().find(|(_, p)| p.to_string_lossy() == f).ok_or("That isn't one of r2modman's profiles for this game.")?;
            let text = fs::read_to_string(dir.join("mods.yml")).map_err(|e| e.to_string())?;
            let mut files = vec![];
            for base in ["BepInEx/config"] {
                let root = dir.join(base);
                let mut stack = vec![root.clone()];
                while let Some(d) = stack.pop() {
                    for e in fs::read_dir(&d).into_iter().flatten().flatten() {
                        let p = e.path();
                        if p.is_dir() {
                            stack.push(p);
                        } else if let Ok(rel) = p.strip_prefix(&dir) {
                            let rel = rel.to_string_lossy().into_owned();
                            if modsettings::is_setting(&rel) && fs::metadata(&p).map(|m| m.len() < 4 * 1024 * 1024).unwrap_or(false) {
                                files.push((rel, fs::read(&p).unwrap_or_default()));
                            }
                        }
                    }
                }
            }
            (n, text, files)
        }
        _ => return Err("Paste a profile code, or pick one of r2modman's profiles.".into()),
    };
    let wanted = thunderstore::r2_mods(&list_text);
    if wanted.is_empty() {
        return Err("That profile has no mods in it.".into());
    }
    let list = thunderstore::listing(app.nexus.http(), &app.cache_dir, &g.community).await?;
    let lib = lib_dir(&app);
    let keep = app.settings.lock().unwrap().keep_archives;
    let (mut added, mut had, mut missing) = (vec![], 0, vec![]);
    let mut mods: Vec<deploy::ProfileMod> = vec![];
    for (i, (full, ver, on)) in wanted.iter().enumerate() {
        let (chain, miss) = thunderstore::resolve(&list, full, ver.as_deref());
        missing.extend(miss);
        for (p, v) in chain {
            let _ = handle.emit("thunderstore:progress", serde_json::json!({ "name": p.name.replace('_', " "), "at": i + 1, "of": wanted.len() }));
            if thunderstore::fetch_into(app.nexus.http(), &lib, &game, &g, p, v, keep, queue::now()).await? {
                added.push(p.name.replace('_', " "));
            } else {
                had += 1;
            }
            let m = thunderstore::mod_id(&p.full_name);
            if !mods.iter().any(|x| x.mod_id == m) {
                // The listed mod keeps its on/off; what it needs comes on.
                let enabled = if p.full_name.eq_ignore_ascii_case(full) { *on } else { true };
                mods.push(deploy::ProfileMod { mod_id: m, file_id: thunderstore::file_id(&format!("{}-{}", p.full_name, v.number)), enabled });
            }
        }
    }
    let _ = handle.emit("library:changed", 0);
    missing.sort();
    missing.dedup();
    let _turn = app.profiles.lock().await;
    let mut all = profiles::load(&app.config_dir);
    let mut pname = format!("{name} (from r2modman)");
    let mut n = 2;
    while all.iter().any(|p| p.game == game && p.name == pname) {
        pname = format!("{name} (from r2modman {n})");
        n += 1;
    }
    let p = deploy::Profile { id: format!("p{}{}", queue::now(), all.len()), name: pname, game: game.clone(), mods, collection: None, manual_requirements: false, new_downloads: false, own_saves: false, base: None, addons: vec![], rules: vec![], note: Some("Brought across from r2modman, with its config files.".into()), ts_pack: None };
    // Its config files are this profile's own settings.
    let store = modsettings::store(&app.data_dir, &p.id);
    for (rel, bytes) in configs {
        if Path::new(&rel).components().all(|c| matches!(c, std::path::Component::Normal(_))) {
            let to = store.join(&rel);
            let _ = fs::create_dir_all(to.parent().unwrap());
            let _ = fs::write(to, bytes);
        }
    }
    all.push(p.clone());
    profiles::save(&app.config_dir, &all)?;
    Ok(TsAdded { added, had, missing, profile: Some(p) })
}

/// Open one of Steam's own pages: only these, never an address from the page.
#[tauri::command]
fn open_steam(handle: AppHandle, what: String) -> Res<()> {
    let url = match what.as_str() {
        "keyboard" => "steam://open/keyboard",
        "tmodloader-workshop" => "steam://url/SteamWorkshopPage/1281930",
        "games" => "steam://open/games",
        _ => return Err("Unknown Steam page.".into()),
    };
    handle.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SteamEntry {
    /// Zom's Nexus is in Steam's library already.
    added: bool,
    steam_running: bool,
    /// What Steam would start.
    program: Option<String>,
    /// Running from an AppImage (the way to install it on a Steam Deck).
    app_image: bool,
    /// A development build: the entry would point at a build folder.
    dev: bool,
    has_steam: bool,
}

#[tauri::command]
fn steam_entry() -> SteamEntry {
    let user = steamshortcut::user_dir();
    let program = steamshortcut::program();
    let added = user
        .as_ref()
        .and_then(|u| fs::read(u.join("config/shortcuts.vdf")).ok())
        .and_then(|b| steamshortcut::parse(&b).ok())
        .map(|t| format!("{t:?}").contains(&format!("Str(\"{}\")", steamshortcut::NAME)))
        .unwrap_or(false);
    let dev = program.as_ref().is_some_and(|p| p.to_string_lossy().contains("/target/debug/") || p.to_string_lossy().contains("/target/release/"));
    SteamEntry { added, steam_running: steamshortcut::steam_running(), program: program.map(|p| p.to_string_lossy().into_owned()), app_image: std::env::var_os("APPIMAGE").is_some(), dev, has_steam: user.is_some() }
}

/// Put Zom's Nexus in Steam's library as a game, with artwork, for Game Mode on a Steam Deck.
/// Steam must be closed (it rewrites the file when it closes); with `restart`, Zom's Nexus closes
/// Steam itself, adds the entry and starts Steam again.
#[tauri::command]
async fn steam_add(restart: bool) -> Res<bool> {
    let user = steamshortcut::user_dir().ok_or("Steam isn't set up on this computer (no Steam user found).")?;
    let exe = steamshortcut::program().ok_or("Couldn't tell where Zom's Nexus is installed.")?;
    let mut closed = false;
    if steamshortcut::steam_running() {
        // In Game Mode, Steam *is* the session: closing it would end it.
        if launchers::device().game_mode {
            return Err("In Game Mode, Steam can't be closed to add Zom's Nexus. Switch to Desktop Mode (STEAM › Power › Switch to Desktop), then press Add to Steam there.".into());
        }
        if !restart {
            return Err("steamRunning".into());
        }
        let _ = host::command("steam").arg("-shutdown").spawn().or_else(|_| host::command("xdg-open").arg("steam://exit").spawn());
        for _ in 0..60 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            if !steamshortcut::steam_running() {
                break;
            }
        }
        if steamshortcut::steam_running() {
            return Err("Steam didn't close. Close it yourself (Steam › Exit), then press Add to Steam again.".into());
        }
        // Steam writes its files as it closes: give it a moment to finish.
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        closed = true;
    }
    let art: [(&str, &[u8]); 5] = [
        ("p.png", include_bytes!("../steam-art/portrait.png")),
        (".png", include_bytes!("../steam-art/wide.png")),
        ("_hero.png", include_bytes!("../steam-art/hero.png")),
        ("_logo.png", include_bytes!("../steam-art/logo.png")),
        ("_icon.png", include_bytes!("../steam-art/icon.png")),
    ];
    let was = tokio::task::spawn_blocking(move || steamshortcut::install(&user, &exe, &art)).await.map_err(|e| e.to_string())??;
    log::info(&format!("Added to Steam's library ({})", if was { "updated" } else { "new" }));
    if closed {
        let _ = host::command("steam").spawn().or_else(|_| host::command("flatpak").args(["run", "com.valvesoftware.Steam"]).spawn());
    }
    Ok(was)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiStatus {
    settings: Option<ai::AiSettings>,
    has_key: bool,
}

#[tauri::command]
fn ai_status(app: State<'_, App>) -> AiStatus {
    AiStatus { settings: app.settings.lock().unwrap().ai.clone(), has_key: ai::load_key(&app.config_dir).is_some() }
}

/// Set up AI help (or switch it off with `settings: None`). `key`: a new key, "" to forget it,
/// or nothing to keep the one there is.
#[tauri::command]
fn ai_set(app: State<'_, App>, settings: Option<ai::AiSettings>, key: Option<String>) -> Res<AiStatus> {
    if let Some(a) = &settings {
        if a.provider != "anthropic" && !a.base_url.trim().is_empty() && !ai::allowed_url(&a.base_url) {
            return Err("The address has to start with https:// (or be on this computer: http://localhost…).".into());
        }
    }
    if let Some(k) = key {
        ai::save_key(&app.config_dir, Some(&k).filter(|k| !k.trim().is_empty()).map(|k| k.as_str()))?;
    }
    if settings.is_none() {
        ai::save_key(&app.config_dir, None)?;
    }
    let mut s = app.settings.lock().unwrap().clone();
    s.ai = settings;
    settings::save(&app.config_dir, &s)?;
    *app.settings.lock().unwrap() = s;
    Ok(ai_status(app))
}

/// What the AI gets to know about a profile (or a game), as text the person sees first: the game
/// and how it runs, the mods in order, the check's findings, what the game's loaders logged, the
/// last runs, and the app's own recent log. Home folder and names taken out; never any key.
#[tauri::command]
async fn ai_context(app: State<'_, App>, profile: Option<String>, game: Option<String>) -> Res<String> {
    use std::fmt::Write as _;
    let p = profile.as_deref().and_then(|id| find_profile(&app, id).ok());
    let domain = p.as_ref().map(|p| p.game.clone()).or(game).ok_or("Pick a game or a profile.")?;
    let g = list_games(&app).into_iter().find(|g| g.domain == domain);
    let mods = library::list(&lib_dir(&app));
    let dev = launchers::device();
    let mut t = String::new();
    let os = Some(host::os_release()).and_then(|o| o.lines().find_map(|l| l.strip_prefix("PRETTY_NAME=")).map(|v| v.trim_matches('"').to_string())).unwrap_or_else(|| "Linux".into());
    let _ = writeln!(t, "App: Zom's Nexus {} on {os}{}{}", nexus::APP_VERSION, if dev.deck { ", Steam Deck" } else { "" }, if dev.game_mode { " (Game Mode)" } else { "" });
    if let Some(g) = &g {
        let _ = writeln!(t, "\n## Game: {} ({})", g.name, g.domain);
        let _ = writeln!(t, "From: {}. Runs with: {}. Install folder: {}. Mod folder: {}.", g.source, g.proton.clone().unwrap_or_else(|| if g.proton_prefix.is_some() { "Proton".into() } else { "native Linux or unknown".into() }), g.path.clone().unwrap_or_default(), g.mod_folder.clone().unwrap_or_else(|| "(not set)".into()));
        if let Some(id) = g.steam_app_id {
            let opts = launchopts::launch_options(id).unwrap_or_default();
            let _ = writeln!(t, "Steam app {id}. Launch options: {}.", if opts.is_empty() { "(none)".into() } else { opts });
        }
        if let Some(u) = g.last_updated {
            let _ = writeln!(t, "Game last updated by Steam: {}.", ai::date(u));
        }
        let _ = writeln!(t, "Mods in the game now: profile {}.", g.active_profile.clone().unwrap_or_else(|| "none".into()));
        if g.steam_cloud {
            let _ = writeln!(t, "Steam Cloud is on for its saves.");
        }
    }
    if let Some(p) = &p {
        let _ = writeln!(t, "\n## Profile: {}", p.name);
        let _ = writeln!(t, "Mods in load order (later ones win file clashes):");
        for (i, pm) in p.mods.iter().enumerate() {
            let m = mods.iter().find(|m| m.game == p.game && m.mod_id == pm.mod_id);
            let f = m.and_then(|m| m.files.iter().find(|f| f.file_id == pm.file_id));
            let src = m.and_then(|m| m.source.clone()).map(|s| format!(", {s}")).unwrap_or_default();
            let _ = writeln!(t, "{}. {} [{}] version {}{}{}{}", i + 1, m.map(|m| m.name.as_str()).unwrap_or("(not downloaded)"), if library::is_local(pm.mod_id) { "own file".to_string() } else { format!("Nexus Mods {}", pm.mod_id) }, f.and_then(|f| f.version.clone()).unwrap_or_else(|| "?".into()), if pm.enabled { "" } else { " (switched off)" }, src, f.map(|f| format!(", file from {}", ai::date(f.downloaded_at))).unwrap_or_default());
        }
        if let Some(r) = app.checks.lock().unwrap().get(&p.id).cloned() {
            let _ = writeln!(t, "\n## The app's check ({})", ai::date(r.at));
            let mut v = serde_json::to_value(&r).unwrap_or_default();
            // Long lists the AI doesn't need in full.
            if let Some(o) = v.as_object_mut() {
                o.remove("loadOrder");
            }
            let _ = writeln!(t, "{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        }
        let names: std::collections::HashMap<u64, String> = mods.iter().filter(|m| m.game == p.game).map(|m| (m.mod_id, m.name.clone())).collect();
        let now: Vec<(u64, u64)> = p.mods.iter().filter(|m| m.enabled).map(|m| (m.mod_id, m.file_id)).collect();
        let all_runs = runs::list(&app.data_dir, &p.id);
        if !all_runs.is_empty() {
            let _ = writeln!(t, "\n## Last runs");
            for r in all_runs.iter().rev().take(6) {
                let _ = writeln!(t, "- {}: ran {} s{}", ai::date(r.start), r.secs, if r.stopped { " (stopped from the app)" } else if r.quick() { " (closed right after starting)" } else { "" });
            }
            if let Some(tr) = runs::trouble(&all_runs, &now, &names) {
                let _ = writeln!(t, "Since the last run that went fine: added {:?}, updated {:?}, removed {:?}.", tr.added.iter().map(|c| &c.name).collect::<Vec<_>>(), tr.updated.iter().map(|c| &c.name).collect::<Vec<_>>(), tr.removed.iter().map(|c| &c.name).collect::<Vec<_>>());
            }
        }
    }
    // What the loaders logged: their newest logs, the lines that matter.
    if let Some(root) = g.as_ref().and_then(|g| g.path.clone()) {
        let prefix = g.as_ref().and_then(|g| g.proton_prefix.clone());
        let mut logs: Vec<(String, PathBuf)> = loaders::logs(Path::new(&root)).into_iter().map(|(n, p)| (n.to_string(), p)).collect();
        for p in [prefix.as_ref().map(|x| PathBuf::from(x).join("drive_c/users/steamuser/AppData/Roaming/StardewValley/ErrorLogs/SMAPI-latest.txt")), std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config/StardewValley/ErrorLogs/SMAPI-latest.txt"))].into_iter().flatten() {
            if p.is_file() && domain == "stardewvalley" {
                logs.push(("SMAPI".into(), p));
            }
        }
        for (name, path) in logs {
            let Ok(b) = fs::read(&path) else { continue };
            let text = String::from_utf8_lossy(&b[b.len().saturating_sub(4 * 1024 * 1024)..]).into_owned();
            let lines: Vec<&str> = text.lines().collect();
            let wanted: Vec<&str> = lines.iter().copied().filter(|l| { let x = l.to_lowercase(); ["error", "exception", "fail", "warn", "skipped", "couldn't", "could not", "missing"].iter().any(|w| x.contains(w)) }).collect();
            let pick: Vec<&str> = wanted.iter().rev().take(80).rev().copied().collect();
            let when = fs::metadata(&path).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| ai::date(d.as_secs())).unwrap_or_default();
            let _ = writeln!(t, "\n## {name}'s log ({when}): {} problem lines of {}; the last {}:", wanted.len(), lines.len(), pick.len());
            for l in pick {
                let _ = writeln!(t, "{}", l.chars().take(300).collect::<String>());
            }
        }
    }
    let _ = writeln!(t, "\n## Zom's Nexus's own recent log");
    let tail = log::tail(400);
    for l in tail.lines().filter(|l| l.contains(&domain) || l.contains("WARN") || l.contains("ERROR")).rev().take(30).collect::<Vec<_>>().into_iter().rev() {
        let _ = writeln!(t, "{l}");
    }
    let mut names = vec![];
    if let Some(u) = std::env::var_os("USER") {
        names.push(u.to_string_lossy().into_owned());
    }
    if let Some(u) = app.user.lock().unwrap().as_ref() {
        names.push(u.name.clone());
    }
    let mut out = ai::scrub(&t, &names);
    // Keep it to a size every service takes.
    if out.len() > 120_000 {
        let mut cut = 120_000;
        while !out.is_char_boundary(cut) {
            cut -= 1;
        }
        out.truncate(cut);
        out.push_str("\n(cut short)");
    }
    Ok(out)
}

/// Ask the AI, with the context the person saw. The answer streams as `ai:text` events
/// ({ id, text }) and comes back whole at the end.
#[tauri::command]
async fn ai_ask(handle: AppHandle, app: State<'_, App>, context: String, messages: Vec<ai::Message>, id: Option<String>) -> Res<String> {
    let s = app.settings.lock().unwrap().ai.clone().ok_or("Set up AI help in Settings first.")?;
    if context.len() > 400_000 || messages.iter().map(|m| m.content.len()).sum::<usize>() > 400_000 {
        return Err("That's more than the AI can take at once. Start a new chat.".into());
    }
    let key = ai::load_key(&app.config_dir);
    let id = id.unwrap_or_default();
    ai::ask(app.nexus.http(), &s, key.as_deref(), &context, &messages, |t| {
        let _ = handle.emit("ai:text", serde_json::json!({ "id": id, "text": t }));
    })
    .await
}

/// A game's save folder now, and whose saves are in it.
fn live_saves(app: &App, game: &str) -> Res<(PathBuf, String, games::Game)> {
    let g = list_games(app).into_iter().find(|g| g.domain == game).ok_or("That game isn't on the Games page.")?;
    let live = g.save_folder.clone().map(PathBuf::from).ok_or("Zom's Nexus doesn't know where this game keeps its saves. Set the save folder on the Games page.")?;
    let owner = saves::owner(&live);
    Ok((live, owner, g))
}

#[tauri::command]
fn snapshot_list(app: State<'_, App>, game: String) -> Vec<snapshots::Snapshot> {
    snapshots::list(&app.data_dir, &game)
}

#[tauri::command]
async fn snapshot_take(app: State<'_, App>, game: String, label: String) -> Res<snapshots::Snapshot> {
    let (live, owner, _) = live_saves(&app, &game)?;
    let data = app.data_dir.clone();
    let label = if label.trim().is_empty() { "Snapshot".to_string() } else { label.trim().to_string() };
    tokio::task::spawn_blocking(move || snapshots::take(&data, &game, &live, &owner, &label, false)).await.map_err(|e| e.to_string())?
}

/// Put a snapshot's saves back (the ones there now are kept: snapshotted and moved aside).
#[tauri::command]
async fn snapshot_restore(app: State<'_, App>, game: String, id: String) -> Res<snapshots::Snapshot> {
    let (live, owner, g) = live_saves(&app, &game)?;
    if g.path.as_deref().is_some_and(|p| activity::running(Path::new(p))) {
        return Err("The game is running: close it first, so it doesn't write over the saves.".into());
    }
    let data = app.data_dir.clone();
    let s = tokio::task::spawn_blocking(move || snapshots::restore(&data, &game, &live, &owner, &id)).await.map_err(|e| e.to_string())??;
    log::info(&format!("Restored a save snapshot ({})", s.label));
    Ok(s)
}

#[tauri::command]
fn snapshot_delete(app: State<'_, App>, game: String, id: String) -> Res<()> {
    snapshots::delete(&app.data_dir, &game, &id)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LootProposal {
    now: Vec<String>,
    sorted: loot::Sorted,
}

/// What LOOT's masterlist says about a profile's plugins: the order it would load them in, and
/// its notes. Nothing changes until `loot_apply`.
#[tauri::command]
async fn loot_sort(app: State<'_, App>, id: String) -> Res<LootProposal> {
    let p = find_profile(&app, &id)?;
    let (root, folder, _, _) = game_target(&app, &p.game)?;
    let ml = loot::masterlist(app.nexus.http(), &app.cache_dir, &p.game).await?;
    let lib = lib_dir(&app);
    let data = app.data_dir.clone();
    tokio::task::spawn_blocking(move || {
        let mods = library::list(&lib);
        let mut r = check::check(&p, &mods, &folder, Some(&root), &HashSet::new());
        check::use_order(&mut r, &loot::saved(&data, &p.id));
        let files: std::collections::HashMap<String, PathBuf> = r.plugin_files.iter().map(|(n, _, f)| (n.to_lowercase(), f.clone())).collect();
        let plugins: Vec<loot::Plugin> = r
            .load_order
            .iter()
            .filter_map(|n| {
                let f = files.get(&n.to_lowercase())?;
                let crc = fs::metadata(f).ok().filter(|m| m.len() < 64 * 1024 * 1024).and_then(|_| fs::read(f).ok()).map(|b| steamshortcut::crc32(&b));
                Some(loot::Plugin { name: n.clone(), master: check::is_master(f), masters: check::masters(f), crc })
            })
            .collect();
        // What the game has in its data folder (its own plugins, and what mods put there).
        let data_dir = root.join(&folder);
        let have: HashSet<String> = fs::read_dir(&data_dir).into_iter().flatten().flatten().map(|e| e.file_name().to_string_lossy().to_lowercase()).chain(files.keys().cloned()).collect();
        let sorted = loot::sort(&ml, &plugins, &|f| have.contains(&f.to_lowercase()));
        LootProposal { now: plugins.iter().map(|p| p.name.clone()).collect(), sorted }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Keep this plugin order for the profile (or go back to the mod order with `None`). It's used
/// the next time the mods go in.
#[tauri::command]
fn loot_apply(app: State<'_, App>, id: String, order: Option<Vec<String>>) -> Res<()> {
    let p = find_profile(&app, &id)?;
    if let Some(o) = &order {
        if o.len() > 10_000 || o.iter().any(|n| n.contains(['/', '\\']) || n.len() > 260) {
            return Err("That isn't a plugin list.".into());
        }
    }
    loot::save(&app.data_dir, &p.id, order.as_deref())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppUpdate {
    /// A newer release, when there is one.
    release: Option<release::Release>,
    /// Installed from Flathub: it updates the app.
    flatpak: bool,
    version: String,
}

/// Whether a newer Zom's Nexus is out (asked at most once a day, when the setting is on).
#[tauri::command]
async fn app_update(app: State<'_, App>) -> Res<AppUpdate> {
    let flatpak = host::in_flatpak();
    let on = app.settings.lock().unwrap().check_app_updates;
    let release = if flatpak || !on { None } else { release::check(app.nexus.http(), &app.cache_dir, nexus::APP_VERSION).await };
    Ok(AppUpdate { release, flatpak, version: nexus::APP_VERSION.to_string() })
}

/// What's in a Mod Organizer 2 folder: its profiles and how many mods.
#[tauri::command]
async fn mo2_look(path: String) -> Res<mo2::Found> {
    tokio::task::spawn_blocking(move || mo2::look(Path::new(&path))).await.map_err(|e| e.to_string())?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Mo2Imported {
    profile: deploy::Profile,
    added: usize,
    skipped: Vec<String>,
    bytes_copied: u64,
}

/// Bring a Mod Organizer 2 profile across as a new profile, with its mods as MO2 installed them.
#[tauri::command]
async fn mo2_import(app: State<'_, App>, game: String, path: String, profile: String) -> Res<Mo2Imported> {
    let lib = lib_dir(&app);
    let (g2, p2, pr2) = (game.clone(), path.clone(), profile.clone());
    let r = tokio::task::spawn_blocking(move || mo2::import(&lib, &g2, Path::new(&p2), &pr2, queue::now())).await.map_err(|e| e.to_string())??;
    let _turn = app.profiles.lock().await;
    let mut all = profiles::load(&app.config_dir);
    let mut name = format!("{profile} (from MO2)");
    let mut n = 2;
    while all.iter().any(|p| p.game == game && p.name == name) {
        name = format!("{profile} (from MO2 {n})");
        n += 1;
    }
    let p = deploy::Profile { id: format!("p{}{}", queue::now(), all.len()), name, game, mods: r.mods, collection: None, manual_requirements: false, new_downloads: false, own_saves: false, base: None, addons: vec![], rules: vec![], note: Some("Brought across from Mod Organizer 2.".into()), ts_pack: None };
    all.push(p.clone());
    profiles::save(&app.config_dir, &all)?;
    activity::add("done", format!("Brought “{profile}” across from Mod Organizer 2"), Some(format!("{} mods new in the library.", r.added)), Some(&p.game), Some("profiles"), Some(&p.id));
    Ok(Mo2Imported { profile: p, added: r.added, skipped: r.skipped, bytes_copied: r.bytes_copied })
}

/// Open a mod pack: make its profile and queue whatever isn't in the library yet.
#[tauri::command]
async fn profile_import(app: State<'_, App>, path: String) -> Res<PackImported> {
    let meta = fs::metadata(&path).map_err(|e| format!("Couldn't open that file: {e}"))?;
    if meta.len() > 5_000_000 {
        return Err("That file is too big to be a mod pack.".into());
    }
    import_pack(&app, &fs::read_to_string(&path).map_err(|e| format!("Couldn't read that file: {e}"))?).await
}

async fn import_pack(app: &App, text: &str) -> Res<PackImported> {
    let pack = profiles::read_pack(text)?;
    if pack.is_addon() {
        return import_addon(app, &pack).await;
    }
    let have = library::list(&lib_dir(app));
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
    Ok(PackImported { profile, queued, note: None })
}

/// An add-on pack goes on a profile of its collection: the one there is (the newest when
/// there are several), or a new one made from the collection at the pack's revision.
async fn import_addon(app: &App, pack: &profiles::Pack) -> Res<PackImported> {
    let want = pack.collection.clone().ok_or("That add-on pack doesn't say which collection it goes on.")?;
    let found = profiles::load(&app.config_dir).into_iter().rev().find(|p| p.game == pack.game && p.base.as_ref().map(|b| &b.slug).or(p.collection.as_ref()) == Some(&want.slug));
    let mut queued = 0;
    let mut note = None;
    let target = match found {
        Some(p) if p.base.is_some() => p,
        Some(p) => {
            profile_collection_link_inner(app, &p).await?;
            find_profile(app, &p.id)?
        }
        None => {
            let address = match want.revision {
                Some(r) => format!("nxm://{}/collections/{}/revisions/{r}", pack.game, want.slug),
                None => format!("nxm://{}/collections/{}", pack.game, want.slug),
            };
            let added = add_collection(app, &address, false).await?;
            queued += added.queued;
            note = Some(format!("Added the collection {} for it.", if want.name.is_empty() { &want.slug } else { &want.name }));
            added.profile
        }
    };
    if let (Some(have), Some(need)) = (target.base.as_ref().and_then(|b| b.revision), want.revision) {
        if have < need {
            note = Some(format!("The add-on was made for revision {need} of the collection; this profile has revision {have}. Update the collection if something's missing."));
        }
    }
    let addon = profiles::addon_from_pack(pack, format!("a{}", queue::now()));
    let (profile, _) = edit_profile(app, &target.id, |p| {
        layers::apply(&mut p.mods, &addon.changes);
        p.addons.push(addon.clone());
        Ok(())
    })
    .await?;
    let have = library::list(&lib_dir(app));
    for m in pack.mods.iter().chain(&pack.files).filter(|m| m.enabled) {
        if !have.iter().any(|s| s.game == pack.game && s.mod_id == m.mod_id && s.files.iter().any(|f| f.file_id == m.file_id)) {
            app.queue.add(&pack.game, m.mod_id, m.file_id, None, None, None);
            if !m.name.is_empty() {
                app.queue.name_mod(&pack.game, m.mod_id, &m.name);
            }
            queued += 1;
        }
    }
    Ok(PackImported { profile, queued, note })
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
    let out = host::command("xdg-mime").args(["query", "default", "x-scheme-handler/nxm"]).output();
    let current = out.ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let ours = current == NXM_DESKTOP || current == FLATPAK_DESKTOP || current == "zoms-nexus.desktop" || current == "Zom's Nexus.desktop";
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

/// The desktop file Flathub installs (it lists nxm links itself).
const FLATPAK_DESKTOP: &str = "com.zombeatz.zoms-nexus.desktop";

#[tauri::command]
fn nxm_register() -> Res<LinkStatus> {
    // From Flathub: the installed desktop file already handles nxm links; just make it the one.
    if host::in_flatpak() {
        let ok = host::command("xdg-mime").args(["default", FLATPAK_DESKTOP, "x-scheme-handler/nxm"]).status().map(|s| s.success()).unwrap_or(false);
        if !ok {
            return Err("Couldn't set Zom's Nexus as the program for Nexus Mods links.".into());
        }
        return Ok(nxm_status());
    }
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
            log::warn(&format!("Couldn't use a Nexus Mods link: {e}"));
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
                // Remember where and how big it was, whether it closes or hides in the tray.
                {
                    use tauri_plugin_window_state::AppHandleExt;
                    let _ = window.app_handle().save_window_state(tauri_plugin_window_state::StateFlags::SIZE | tauri_plugin_window_state::StateFlags::POSITION | tauri_plugin_window_state::StateFlags::MAXIMIZED);
                }
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
        // The window opens at the size (and, where the desktop allows it, the place) it had
        // last time. Showing it stays the app's own choice.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(tauri_plugin_window_state::StateFlags::SIZE | tauri_plugin_window_state::StateFlags::POSITION | tauri_plugin_window_state::StateFlags::MAXIMIZED)
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            log::init(&data_dir);
            activity::init(&data_dir, app.handle().clone());
            gamepad::start(app.handle().clone());
            log::info(&format!("Zom's Nexus {} starting", nexus::APP_VERSION));
            let cache_dir = app.path().app_cache_dir()?;
            fs::create_dir_all(&config_dir)?;
            fs::create_dir_all(&data_dir)?;
            let s = settings::load(&config_dir);
            activity::set_off(&s.activity_off);
            library::set_roots(&s.game_libraries);
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
                // Someone who keeps the app in the tray on purpose keeps it there.
                let stays = h.state::<App>().settings.lock().unwrap().start_hidden && h.state::<App>().has_tray.load(std::sync::atomic::Ordering::Relaxed);
                if hidden && !stays {
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
            } else if app.state::<App>().settings.lock().unwrap().start_hidden && app.state::<App>().has_tray.load(std::sync::atomic::Ordering::Relaxed) {
                // Starts in the tray, as asked (Settings). Opened from its icon.
            } else if let Some(w) = app.get_webview_window("main") {
                // Opening by itself: show the window without taking the keyboard from what
                // you're doing (opening it from the tray or menu still brings it forward).
                let _ = w.show();
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

            // A game already running when the app starts (after a restart, say) is shown and timed
            // from now, with the profile that's in it.
            let handle2 = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                let app = handle2.state::<App>();
                let all = profiles::load(&app.config_dir);
                for g in list_games(&app) {
                    let Some(path) = g.path.clone() else { continue };
                    let p2 = PathBuf::from(&path);
                    if !tokio::task::spawn_blocking(move || activity::running(&p2)).await.unwrap_or(false) {
                        continue;
                    }
                    let profile = g.active_profile.as_ref().and_then(|n| all.iter().find(|p| p.game == g.domain && &p.name == n));
                    activity::watch(app.data_dir.clone(), profile.map(|p| p.id.clone()).unwrap_or_default(), profile.map(|p| p.name.clone()).unwrap_or_default(), g.domain.clone(), g.name.clone(), PathBuf::from(path));
                }
            });

            // Learn whether the account is Premium, in the background.
            let handle = app.handle().clone();
            // Tries again a few times when the network isn't up yet (after sign-in, say); a
            // key Nexus Mods turns down (401) isn't asked about again.
            tauri::async_runtime::spawn(async move {
                if !nexus.has_key() {
                    return;
                }
                for wait in [0u64, 3, 10, 30, 60, 120] {
                    tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
                    if handle.state::<App>().user.lock().unwrap().is_some() {
                        return; // Checked meanwhile (Check again, or a new key).
                    }
                    match nexus.validate().await {
                        Ok(u) => {
                            handle.state::<App>().set_user(Some(u));
                            let _ = handle.emit("account:changed", ());
                            return;
                        }
                        Err(e) if e.status == 401 => {
                            log::warn("Nexus Mods turned down the API key; check it in Settings.");
                            return;
                        }
                        Err(e) => log::warn(&format!("Couldn't check the account yet: {}", e.message)),
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
            log_tail,
            log_page,
            log_open,
            queue_clear_finished,
            queue_reorder,
            library_list,
            library_unpack,
            library_remove_file,
            library_remove_mod,
            library_import,
            library_usage,
            disk_usage,
            activity_list,
            activity_mark,
            profile_play_times,
            game_states,
            saves_copies,
            saves_move_copies,
            library_drop_archive,
            open_folder,
            games_list,
            games_add,
            steam_libraries,
            game_catalog,
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
            profile_layers,
            activity_ignored,
            activity_ignore,
            profiles_order,
            profile_collection_link,
            profile_collection_update,
            profile_addon_save,
            profile_addon_set,
            profile_addon_export,
            profile_import,
            fomod_read,
            fomod_install,
            fomod_reset,
            fomod_image,
            profile_history,
            profile_restore,
            games_gone,
            games_gone_answer,
            mod_tags,
            set_mod_tags,
            profile_share_code,
            profile_import_code,
            library_move_game,
            app_backup,
            app_restore,
            game_stop,
            profile_history_note,
            mod_changelog,
            profile_last_run,
            profile_settings,
            profile_setting_read,
            profile_setting_write,
            profile_setting_reset,
            mo2_look,
            mo2_import,
            games_clear_all,
            ts_game,
            ts_search,
            ts_add,
            tml_state,
            tml_set,
            tml_save_pack,
            device,
            custom_protons,
            ts_updates,
            ts_update,
            ts_update_pack,
            r2_profiles,
            r2_import,
            steam_entry,
            steam_add,
            ai_status,
            ai_set,
            ai_context,
            ai_ask,
            open_steam,
            snapshot_list,
            snapshot_take,
            snapshot_restore,
            snapshot_delete,
            loot_sort,
            loot_apply,
            app_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Zom's Nexus");
}
