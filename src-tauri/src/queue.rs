//! The download queue.
//!
//! Files wait in a list and start in that order. **Sequential** runs one at a time (download,
//! then store and unpack, then the next one). **Concurrent** runs up to `max_concurrent` at once.
//! Switching between them takes effect right away: running downloads carry on, and new ones
//! start only while there's a free slot.
//!
//! A download writes to `<library>/.incoming/<id>.part` and picks up where it stopped after a
//! pause, a lost connection or a restart of the app (an HTTP Range request). Download links from
//! Nexus Mods run out, so a fresh one is asked for each time a file starts.
//!
//! The list is saved in `queue.json` in the app's data folder whenever a file changes state.

use crate::{
    library,
    nexus::{FileInfo, ModInfo, Nexus},
    settings::Settings,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter};
use tokio::{io::AsyncWriteExt, sync::Notify, task::AbortHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// Waiting for a free slot.
    Queued,
    /// Asking Nexus Mods about the file and where to download it.
    Starting,
    Downloading,
    /// Moving it into the library and unpacking it.
    Storing,
    Paused,
    /// Lost the connection; tries again by itself in a few seconds.
    Waiting,
    /// A free account's link ran out, or the file needs a click on Mod manager download.
    NeedsLink,
    Failed,
    Done,
}

impl Status {
    /// Takes up one of the slots.
    fn running(self) -> bool {
        matches!(self, Status::Starting | Status::Downloading | Status::Storing)
    }
    fn finished(self) -> bool {
        matches!(self, Status::Done)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub game: String,
    pub mod_id: u64,
    pub file_id: u64,
    pub mod_name: String,
    pub file_name: String,
    /// The file's title on Nexus Mods ("Main file", "SkyUI 5.2").
    pub name: String,
    pub version: Option<String>,
    /// Size in bytes, 0 until it's known.
    pub size: u64,
    pub received: u64,
    pub status: Status,
    pub error: Option<String>,
    pub added_at: u64,
    pub finished_at: Option<u64>,
    /// Where it was stored in the library.
    pub path: Option<String>,
    /// Bytes a second, while downloading.
    #[serde(default)]
    pub speed: u64,
    /// How many times it has been tried again by itself since it last worked.
    #[serde(default)]
    pub retries: u32,
    /// From an nxm link; lets a free account download this one file. Never sent to the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<u64>,
    /// Filled in from Nexus Mods when the file first starts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mod_info: Option<ModInfo>,
    /// The file's details from Nexus Mods, when they were looked up as it was added (so the
    /// download doesn't ask again). Never sent to the page.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_info: Option<FileInfo>,
    /// For the page: whether a link from the website's button has come in for this file (the
    /// key itself stays here). A free account's file without one is waiting for that click.
    #[serde(default, skip_deserializing)]
    pub has_link: bool,
}

impl Job {
    /// The job as the page sees it: without the download key.
    fn public(&self) -> Job {
        let mut j = self.clone();
        j.has_link = self.key.is_some();
        j.key = None;
        j.mod_info = None;
        j.file_info = None;
        j
    }
}

/// "1.24 GB", for messages.
pub fn size_words(n: u64) -> String {
    let units = ["bytes", "kB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 999.5 && i < units.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 { format!("{n} bytes") } else if v >= 99.95 { format!("{v:.0} {}", units[i]) } else if v >= 9.995 { format!("{v:.1} {}", units[i]) } else { format!("{v:.2} {}", units[i]) }
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

struct State {
    jobs: Vec<Job>,
    tasks: HashMap<String, AbortHandle>,
}

#[derive(Clone)]
pub struct Queue {
    state: Arc<Mutex<State>>,
    wake: Arc<Notify>,
    settings: Arc<Mutex<Settings>>,
    nexus: Nexus,
    app: Option<AppHandle>,
    file: PathBuf,
    /// Saves take turns, so an older list never lands on disk after a newer one.
    saving: Arc<Mutex<()>>,
    /// With a speed limit: when the downloads, all together, may next take in more data.
    next_slot: Arc<Mutex<Instant>>,
    /// The account is Premium (for the automatic mode).
    premium: Arc<std::sync::atomic::AtomicBool>,
}

fn game_running() -> bool {
    crate::activity::game_states().iter().any(|g| g.state == "running")
}

/// What a download attempt ended with.
enum Outcome {
    Stored(PathBuf),
    NeedsLink(String),
    /// Try again by itself (a lost connection).
    Retry(String),
    Failed(String),
}

impl Queue {
    pub fn new(app: Option<AppHandle>, data_dir: PathBuf, settings: Arc<Mutex<Settings>>, nexus: Nexus) -> Self {
        let file = data_dir.join("queue.json");
        let mut jobs: Vec<Job> = std::fs::read_to_string(&file).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        // Anything that was running when the app closed waits for its turn again, and so does
        // anything waiting to try again (its timer ended with the app).
        // Unless the person wants them kept paused at start.
        let resume = settings.lock().unwrap().resume_on_start != "no";
        for j in &mut jobs {
            if j.status.running() || j.status == Status::Waiting {
                j.status = if resume { Status::Queued } else { Status::Paused };
            }
            j.speed = 0;
        }
        let q = Queue { state: Arc::new(Mutex::new(State { jobs, tasks: HashMap::new() })), wake: Arc::new(Notify::new()), settings, nexus, app, file, saving: Arc::default(), next_slot: Arc::new(Mutex::new(Instant::now())), premium: Arc::default() };
        q.save();
        q
    }

    /// Start the loop that hands out slots. Call once, from inside the async runtime.
    pub fn start(&self) {
        let q = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                q.schedule();
                // Also look once a minute, for download hours and games starting or closing.
                tokio::select! {
                    _ = q.wake.notified() => {}
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                }
            }
        });
    }

    pub fn set_premium(&self, premium: bool) {
        self.premium.store(premium, std::sync::atomic::Ordering::Relaxed);
        self.poke();
    }

    /// Look at the list again (after a change of mode, a new file, a finished one).
    pub fn poke(&self) {
        self.wake.notify_one();
    }

    /// Why new downloads wait now, if they do: outside the download hours, or a game is running.
    pub fn held(&self) -> Option<String> {
        let s = self.settings.lock().unwrap().clone();
        if !s.may_start(crate::log::hour_now()) {
            return Some(format!("Downloads start between {}:00 and {}:00 (Settings).", s.hours_from, s.hours_to));
        }
        if s.while_playing != "carry" && game_running() {
            return Some("A game is running, so downloads wait until it closes (Settings).".into());
        }
        None
    }

    fn schedule(&self) {
        if self.held().is_some() {
            return;
        }
        let slots = self.settings.lock().unwrap().slots(self.premium.load(std::sync::atomic::Ordering::Relaxed));
        let mut st = self.state.lock().unwrap();
        let mut running = st.jobs.iter().filter(|j| j.status.running()).count();
        let mut start = vec![];
        for j in st.jobs.iter_mut() {
            if running >= slots {
                break;
            }
            if j.status == Status::Queued {
                j.status = Status::Starting;
                j.error = None;
                start.push(j.id.clone());
                running += 1;
            }
        }
        for id in start {
            let q = self.clone();
            let task_id = id.clone();
            let handle = tauri::async_runtime::spawn(async move { q.run(task_id).await });
            st.tasks.insert(id.clone(), handle.inner().abort_handle());
            if let Some(j) = st.jobs.iter().find(|j| j.id == id) {
                self.emit_job(j);
            }
        }
        drop(st);
        self.save();
    }

    pub fn list(&self) -> Vec<Job> {
        self.state.lock().unwrap().jobs.iter().map(Job::public).collect()
    }

    fn save(&self) {
        let _turn = self.saving.lock().unwrap();
        let jobs = self.state.lock().unwrap().jobs.clone();
        if let Ok(json) = serde_json::to_vec_pretty(&jobs) {
            let _ = crate::settings::write_atomic(&self.file, &json);
        }
    }

    /// Whether anything is downloading or waiting its turn (not paused, finished, or waiting for
    /// a click on the website).
    pub fn busy(&self) -> bool {
        self.state.lock().unwrap().jobs.iter().any(|j| matches!(j.status, Status::Queued | Status::Starting | Status::Downloading | Status::Storing | Status::Waiting))
    }

    fn emit_job(&self, j: &Job) {
        if let Some(app) = &self.app {
            let _ = app.emit("queue:job", j.public());
        }
    }

    fn emit_all(&self) {
        if let Some(app) = &self.app {
            let _ = app.emit("queue:all", self.list());
        }
    }

    fn emit_library(&self) {
        if let Some(app) = &self.app {
            let _ = app.emit("library:changed", ());
        }
    }

    /// Change a job under the lock, tell the page and save. Returns false when it's gone.
    fn update(&self, id: &str, f: impl FnOnce(&mut Job)) -> bool {
        let snapshot = {
            let mut st = self.state.lock().unwrap();
            let Some(j) = st.jobs.iter_mut().find(|j| j.id == id) else { return false };
            f(j);
            j.clone()
        };
        self.emit_job(&snapshot);
        self.save();
        true
    }

    fn get(&self, id: &str) -> Option<Job> {
        self.state.lock().unwrap().jobs.iter().find(|j| j.id == id).cloned()
    }

    /// Still ours to work on (not paused, removed or put back in the queue meanwhile).
    fn still(&self, id: &str, s: Status) -> bool {
        self.get(id).map(|j| j.status == s).unwrap_or(false)
    }

    /// Add a file. If it's already waiting, a new key (from a fresh nxm link) is passed to it and
    /// it goes back in the queue. Gives the job.
    pub fn add(&self, game: &str, mod_id: u64, file_id: u64, key: Option<String>, expires: Option<u64>, hint: Option<(&ModInfo, &FileInfo)>) -> Job {
        let job = {
            let mut st = self.state.lock().unwrap();
            if let Some(j) = st.jobs.iter_mut().find(|j| j.game == game && j.mod_id == mod_id && j.file_id == file_id && !j.status.finished()) {
                if key.is_some() {
                    j.key = key;
                    j.expires = expires;
                }
                if matches!(j.status, Status::NeedsLink | Status::Failed | Status::Paused | Status::Waiting) {
                    j.status = Status::Queued;
                    j.error = None;
                    j.retries = 0;
                }
                j.clone()
            } else {
                let (mod_name, mod_info) = match hint {
                    Some((m, _)) => (m.name.clone(), Some(m.clone())),
                    None => (format!("Mod {mod_id}"), None),
                };
                let j = Job {
                    id: format!("{game}-{mod_id}-{file_id}-{}", now()),
                    game: game.to_string(),
                    mod_id,
                    file_id,
                    mod_name,
                    file_name: hint.map(|(_, f)| f.file_name.clone()).unwrap_or_default(),
                    name: hint.map(|(_, f)| f.name.clone()).unwrap_or_else(|| format!("File {file_id}")),
                    version: hint.and_then(|(_, f)| f.version.clone()),
                    size: hint.map(|(_, f)| f.size).unwrap_or(0),
                    received: 0,
                    status: Status::Queued,
                    error: None,
                    added_at: now(),
                    finished_at: None,
                    path: None,
                    speed: 0,
                    retries: 0,
                    key,
                    expires,
                    mod_info,
                    file_info: None,
                    has_link: false,
                };
                st.jobs.push(j.clone());
                j
            }
        };
        self.save();
        self.emit_all();
        self.poke();
        job.public()
    }

    fn part_path(&self, id: &str) -> PathBuf {
        let lib = self.settings.lock().unwrap().library();
        lib.join(library::INCOMING).join(format!("{}.part", library::safe_name(id)))
    }

    fn stop_task(&self, id: &str) {
        if let Some(h) = self.state.lock().unwrap().tasks.remove(id) {
            h.abort();
        }
    }

    pub fn pause(&self, id: &str) {
        if self.get(id).map(|j| matches!(j.status, Status::Queued | Status::Starting | Status::Downloading | Status::Waiting)) != Some(true) {
            return;
        }
        self.update(id, |j| {
            j.status = Status::Paused;
            j.speed = 0;
        });
        self.stop_task(id);
        self.poke();
    }

    pub fn resume(&self, id: &str) {
        self.update(id, |j| {
            if matches!(j.status, Status::Paused | Status::Failed | Status::NeedsLink | Status::Waiting) {
                j.status = Status::Queued;
                j.error = None;
                j.retries = 0;
            }
        });
        self.poke();
    }

    /// Stop it (if it's running), delete what was downloaded so far and take it off the list.
    /// Files already in the library stay there.
    pub fn remove(&self, id: &str) {
        let Some(j) = self.get(id) else { return };
        if j.status == Status::Storing {
            return; // Let it finish moving into the library first.
        }
        self.stop_task(id);
        let _ = std::fs::remove_file(self.part_path(id));
        self.state.lock().unwrap().jobs.retain(|j| j.id != id);
        self.save();
        self.emit_all();
        self.poke();
    }

    pub fn pause_all(&self) {
        let ids: Vec<String> = self.list().into_iter().filter(|j| matches!(j.status, Status::Queued | Status::Starting | Status::Downloading | Status::Waiting)).map(|j| j.id).collect();
        for id in ids {
            self.pause(&id);
        }
    }

    /// Download this file now: first in the list and started straight away. When every slot is
    /// taken, the most recently started download steps aside to second place, keeping what it
    /// has downloaded, so the number running at once never goes over the setting. A file still
    /// waiting for its click on Nexus Mods only moves to the top.
    pub fn now(&self, id: &str) {
        let slots = self.settings.lock().unwrap().slots(self.premium.load(std::sync::atomic::Ordering::Relaxed));
        let bumped = {
            let mut st = self.state.lock().unwrap();
            let Some(from) = st.jobs.iter().position(|j| j.id == id) else { return };
            let mut j = st.jobs.remove(from);
            let can_start = match j.status {
                Status::NeedsLink => j.key.is_some(),
                Status::Paused | Status::Failed | Status::Waiting | Status::Queued => true,
                _ => false,
            };
            if can_start && j.status != Status::Queued {
                j.status = Status::Queued;
                j.error = None;
                j.retries = 0;
            }
            let ready = j.status == Status::Queued;
            st.jobs.insert(0, j);
            let mut bumped = None;
            if ready && st.jobs.iter().filter(|j| j.status.running()).count() >= slots {
                // Storing is nearly done; let it finish. Step aside the newest download instead.
                if let Some(i) = st.jobs.iter().rposition(|j| matches!(j.status, Status::Starting | Status::Downloading)) {
                    let mut b = st.jobs.remove(i);
                    b.status = Status::Queued;
                    b.speed = 0;
                    bumped = Some(b.id.clone());
                    st.jobs.insert(1, b);
                }
            }
            bumped
        };
        if let Some(b) = bumped {
            self.stop_task(&b);
        }
        self.save();
        self.emit_all();
        self.poke();
    }

    /// Put the files that haven't started in order of size (unknown sizes last). Files that are
    /// running or finished keep their places.
    pub fn sort_by_size(&self, smallest_first: bool) {
        {
            let mut st = self.state.lock().unwrap();
            let slots: Vec<usize> = st.jobs.iter().enumerate().filter(|(_, j)| matches!(j.status, Status::Queued | Status::Paused | Status::Waiting | Status::NeedsLink | Status::Failed)).map(|(i, _)| i).collect();
            let mut waiting: Vec<Job> = slots.iter().map(|&i| st.jobs[i].clone()).collect();
            waiting.sort_by_key(|j| (j.size == 0, if smallest_first { j.size } else { u64::MAX - j.size }));
            for (i, j) in slots.into_iter().zip(waiting) {
                st.jobs[i] = j;
            }
        }
        self.save();
        self.emit_all();
        self.poke();
    }

    /// Name a mod's files that were added before its name was known ("Mod 11077"), as soon as
    /// it is (the download itself fills in the rest when it starts).
    pub fn name_mod(&self, game: &str, mod_id: u64, name: &str) {
        let placeholder = format!("Mod {mod_id}");
        let changed: Vec<Job> = {
            let mut st = self.state.lock().unwrap();
            st.jobs
                .iter_mut()
                .filter(|j| j.game == game && j.mod_id == mod_id && j.mod_name == placeholder)
                .map(|j| {
                    j.mod_name = name.to_string();
                    j.clone()
                })
                .collect()
        };
        if !changed.is_empty() {
            self.save();
            for j in &changed {
                self.emit_job(j);
            }
        }
    }

    /// Fill in a waiting file's name, version and size as soon as they're known (instead of
    /// when it starts), keeping the details for the download when it's the full file info.
    pub fn describe_file(&self, game: &str, mod_id: u64, file: &FileInfo, full: bool) {
        let changed: Vec<Job> = {
            let mut st = self.state.lock().unwrap();
            st.jobs
                .iter_mut()
                .filter(|j| j.game == game && j.mod_id == mod_id && j.file_id == file.file_id && !j.status.finished() && !j.status.running())
                .map(|j| {
                    j.name = file.name.clone();
                    if file.version.is_some() {
                        j.version = file.version.clone();
                    }
                    if file.size > 0 {
                        j.size = file.size;
                    }
                    if full {
                        j.file_name = file.file_name.clone();
                        j.file_info = Some(file.clone());
                    }
                    j.clone()
                })
                .collect()
        };
        if !changed.is_empty() {
            self.save();
            for j in &changed {
                self.emit_job(j);
            }
        }
    }

    /// Waiting files whose size and name aren't known yet (added before they were looked up).
    pub fn undescribed(&self) -> Vec<(String, u64, u64)> {
        self.state
            .lock()
            .unwrap()
            .jobs
            .iter()
            .filter(|j| j.file_info.is_none() && j.size == 0 && !j.status.finished() && !j.status.running())
            .map(|j| (j.game.clone(), j.mod_id, j.file_id))
            .collect()
    }

    /// Try every failed file again.
    pub fn retry_failed(&self) {
        {
            let mut st = self.state.lock().unwrap();
            for j in st.jobs.iter_mut().filter(|j| j.status == Status::Failed) {
                j.status = Status::Queued;
                j.error = None;
                j.retries = 0;
            }
        }
        self.save();
        self.emit_all();
        self.poke();
    }

    pub fn resume_all(&self) {
        {
            let mut st = self.state.lock().unwrap();
            for j in st.jobs.iter_mut().filter(|j| j.status == Status::Paused) {
                j.status = Status::Queued;
            }
        }
        self.save();
        self.emit_all();
        self.poke();
    }

    pub fn clear_finished(&self) {
        self.state.lock().unwrap().jobs.retain(|j| !j.status.finished());
        self.save();
        self.emit_all();
    }

    /// Move a job to a new place in the list (which decides what starts next).
    pub fn reorder(&self, id: &str, to: usize) {
        {
            let mut st = self.state.lock().unwrap();
            let Some(from) = st.jobs.iter().position(|j| j.id == id) else { return };
            let j = st.jobs.remove(from);
            let to = to.min(st.jobs.len());
            st.jobs.insert(to, j);
        }
        self.save();
        self.emit_all();
    }

    async fn run(&self, id: String) {
        let outcome = self.attempt(&id).await;
        self.state.lock().unwrap().tasks.remove(&id);
        let auto_retry = self.settings.lock().unwrap().auto_retry;
        match outcome {
            Outcome::Stored(path) => {
                self.update(&id, |j| {
                    j.status = Status::Done;
                    j.finished_at = Some(now());
                    j.path = Some(path.to_string_lossy().into_owned());
                    j.received = j.size.max(j.received);
                    j.speed = 0;
                    j.retries = 0;
                    j.key = None; // Used up; never needed again.
                });
                self.emit_library();
                if !self.busy() {
                    if let Some(app) = &self.app {
                        let _ = app.emit("queue:idle", ());
                    }
                }
            }
            Outcome::NeedsLink(msg) => {
                crate::log::info(&format!("{id}: needs a link: {msg}"));
                self.update(&id, |j| {
                    if j.status.running() {
                        j.status = Status::NeedsLink;
                        j.error = Some(msg);
                        j.speed = 0;
                    }
                });
            }
            Outcome::Retry(msg) if auto_retry && self.get(&id).map(|j| j.retries < 3) == Some(true) => {
                crate::log::warn(&format!("{id}: trying again soon: {msg}"));
                let tries = self.get(&id).map(|j| j.retries).unwrap_or(0) + 1;
                self.update(&id, |j| {
                    if j.status.running() {
                        j.status = Status::Waiting;
                        j.error = Some(format!("{msg} Trying again in {} seconds.", 5 * tries));
                        j.speed = 0;
                        j.retries = tries;
                    }
                });
                // Wait outside the slot, so other files carry on meanwhile.
                let q = self.clone();
                let id2 = id.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(5 * tries as u64)).await;
                    // Only if nobody paused or removed it meanwhile.
                    if q.get(&id2).map(|j| j.status == Status::Waiting && j.retries == tries) == Some(true) {
                        q.update(&id2, |j| j.status = Status::Queued);
                        q.poke();
                    }
                });
            }
            // After the quick tries: later, in 5 minutes, 30 minutes, then 2 hours.
            Outcome::Retry(msg) if auto_retry && self.settings.lock().unwrap().retry_later && self.get(&id).map(|j| j.retries < 6) == Some(true) => {
                let tries = self.get(&id).map(|j| j.retries).unwrap_or(0) + 1;
                let wait = [300u64, 1800, 7200][(tries as usize).saturating_sub(4).min(2)];
                crate::log::warn(&format!("{id}: trying again in {} minutes: {msg}", wait / 60));
                self.update(&id, |j| {
                    if j.status.running() {
                        j.status = Status::Waiting;
                        j.error = Some(format!("{msg} Trying again in {} minutes.", wait / 60));
                        j.speed = 0;
                        j.retries = tries;
                    }
                });
                let q = self.clone();
                let id2 = id.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(wait)).await;
                    if q.get(&id2).map(|j| j.status == Status::Waiting && j.retries == tries) == Some(true) {
                        q.update(&id2, |j| j.status = Status::Queued);
                        q.poke();
                    }
                });
            }
            Outcome::Retry(msg) | Outcome::Failed(msg) => {
                crate::log::error(&format!("{id}: didn't work: {msg}"));
                if let Some(j) = self.get(&id) {
                    crate::activity::failed(&j.mod_name, &msg);
                }
                self.update(&id, |j| {
                    if j.status.running() {
                        j.status = Status::Failed;
                        j.error = Some(msg);
                        j.speed = 0;
                    }
                });
            }
        }
        self.poke();
    }

    async fn attempt(&self, id: &str) -> Outcome {
        let Some(job) = self.get(id) else { return Outcome::Failed("It was taken off the list.".into()) };

        // 1. What the file is, if it isn't known yet.
        let info = match &job.mod_info {
            Some(m) => m.clone(),
            None => match self.nexus.mod_info(&job.game, job.mod_id).await {
                Ok(m) => m,
                Err(e) if e.status == 0 => return Outcome::Retry(e.message),
                Err(e) => return Outcome::Failed(e.message),
            },
        };
        let file = match job.file_info.clone() {
            Some(f) => f,
            None => match self.nexus.file(&job.game, job.mod_id, job.file_id).await {
                Ok(f) => f,
                Err(e) if e.status == 0 => return Outcome::Retry(e.message),
                Err(e) => return Outcome::Failed(e.message),
            },
        };
        let _ = self.update(id, |j| {
            j.mod_name = info.name.clone();
            j.name = file.name.clone();
            j.file_name = file.file_name.clone();
            j.version = file.version.clone();
            j.size = file.size;
            j.mod_info = Some(info.clone());
        });

        // No partial file but bytes received: the app may have closed just after moving the
        // finished file into the library. Use that copy instead of downloading it again (and
        // without asking Nexus Mods for a link).
        if job.received > 0 && !self.part_path(id).exists() {
            let (lib, g, i, f) = (self.settings.lock().unwrap().library(), job.game.clone(), info.clone(), file.clone());
            if let Ok(Some((dir, _))) = tokio::task::spawn_blocking(move || library::adopt(&lib, &g, &i, &f, now())).await {
                return Outcome::Stored(dir);
            }
        }

        // 2. Where to download it from.
        if let Some(exp) = job.expires {
            if job.key.is_some() && exp <= now() {
                return Outcome::NeedsLink("The download link from Nexus Mods has run out. Click Mod manager download on the file's page again.".into());
            }
        }
        let url = match self.nexus.download_url(&job.game, job.mod_id, job.file_id, job.key.as_deref(), job.expires).await {
            Ok(u) => u,
            Err(e) if e.status == 403 && job.key.is_none() => {
                return Outcome::NeedsLink("Nexus Mods only lets Premium members download straight from an app. Click Mod manager download on the file's page and Zom's Nexus picks it up from there.".into())
            }
            Err(e) if matches!(e.status, 403 | 410) => return Outcome::NeedsLink(e.message),
            Err(e) if e.status == 0 || e.status == 429 || e.status >= 500 => return Outcome::Retry(e.message),
            Err(e) => return Outcome::Failed(e.message),
        };

        // 3. Download, picking up from what's already there.
        if !self.still(id, Status::Starting) {
            return Outcome::Failed("Stopped.".into());
        }
        let part = self.part_path(id);
        if let Some(dir) = part.parent() {
            if let Err(e) = tokio::fs::create_dir_all(dir).await {
                return Outcome::Failed(format!("Couldn't make the download folder {}: {e}", dir.display()));
            }
        }
        // Room on the drive for the rest of it (and for unpacking it, when that's on).
        let have = tokio::fs::metadata(&part).await.map(|m| m.len()).unwrap_or(0);
        let unpacking = self.settings.lock().unwrap().extract;
        let need = file.size.saturating_sub(have) + if unpacking { file.size * 2 } else { 0 };
        if let Some(free) = crate::settings::free_space(&part) {
            if need > 0 && free < need + 50_000_000 {
                return Outcome::Failed(format!(
                    "There isn't enough free space on the drive: this file needs about {} and the drive has {} left. Free up some space or pick another library folder in Settings, then try again.",
                    size_words(need),
                    size_words(free)
                ));
            }
        }
        if let Err(o) = self.download(id, &url, &part, file.size).await {
            return o;
        }

        // 4. Into the library, and unpacked if asked.
        if !self.update(id, |j| {
            j.status = Status::Storing;
            j.speed = 0;
        }) {
            return Outcome::Failed("It was taken off the list.".into());
        }
        let (lib, unpack, keep) = {
            let s = self.settings.lock().unwrap();
            (s.library(), s.extract, s.keep_archives)
        };
        let game = job.game.clone();
        let verify = self.settings.lock().unwrap().verify_downloads;
        let file_id = file.file_id;
        let stored = tokio::task::spawn_blocking(move || -> Result<(PathBuf, Option<String>), String> {
            let (dir, archive) = library::store(&lib, &part, &game, &info, &file, now())?;
            // The checksum before unpacking (which may delete the archive).
            let md5 = if verify { library::md5_of(&archive).ok() } else { None };
            if unpack && library::kind(&archive) != library::Kind::Other {
                // A file that won't unpack is still kept; the library shows it as an archive.
                match library::unpack(&archive) {
                    Ok(name) => library::set_unpacked(&dir, file.file_id, Some(name), !keep)?,
                    Err(e) => crate::log::warn(&format!("Couldn't unpack {}: {e}", archive.display())),
                }
            }
            Ok((dir, md5))
        })
        .await;
        match stored {
            Ok(Ok((dir, md5))) => {
                // Does it match Nexus Mods' copy? One request; a connection problem just leaves
                // it unchecked.
                if let Some(md5) = md5 {
                    if let Ok(found) = self.nexus.md5_search(&job.game, &md5).await {
                        let ok = found.contains(&(job.mod_id, file_id));
                        if !ok {
                            crate::log::warn(&format!("{id}: the file doesn't match Nexus Mods' checksum ({md5})"));
                        }
                        let _ = library::set_verified(&dir, file_id, ok);
                    }
                }
                Outcome::Stored(dir)
            }
            Ok(Err(e)) => Outcome::Failed(e),
            Err(e) => Outcome::Failed(e.to_string()),
        }
    }

    /// Keep all downloads together under the speed limit: each piece books its share of time,
    /// and waits until that time has passed.
    async fn throttle(&self, n: u64) {
        // "Pause while playing": hold every download where it is until the game closes (the
        // connection may drop meanwhile; it carries on from where it was).
        while self.settings.lock().unwrap().while_playing == "pause" && game_running() {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let rate = self.settings.lock().unwrap().limit_at(crate::log::hour_now());
        if rate == 0 {
            return;
        }
        let wait = {
            let mut next = self.next_slot.lock().unwrap();
            let now = Instant::now();
            let start = (*next).max(now);
            *next = start + Duration::from_secs_f64(n as f64 / rate as f64);
            next.saturating_duration_since(now)
        };
        if wait > Duration::from_millis(5) {
            tokio::time::sleep(wait).await;
        }
    }

    /// Stream the file into `part`, carrying on from its current length.
    async fn download(&self, id: &str, url: &str, part: &PathBuf, size: u64) -> Result<(), Outcome> {
        let have = tokio::fs::metadata(part).await.map(|m| m.len()).unwrap_or(0);
        if size > 0 && have == size {
            return Ok(()); // Finished before the app closed, but not yet stored.
        }
        // Nexus Mods sometimes gives a file's size only in whole kilobytes (rounded down), so
        // `size` can be up to 1023 bytes short. The server's own length is exact; trust it.
        let about = |n: u64| size > 0 && n >= size && n < size + 1024;
        let mut req = self.nexus.http().get(url);
        if have > 0 {
            req = req.header("Range", format!("bytes={have}-"));
        }
        let res = req.send().await.map_err(|e| Outcome::Retry(format!("Lost the connection to the download server. ({e})")))?;
        let status = res.status().as_u16();
        let resumed = status == 206;
        match status {
            200 | 206 => {}
            // Nothing more to send, and what's here is the listed size: it's all here.
            416 if about(have) => return Ok(()),
            416 if have > 0 => {
                // The server says there's nothing more to send; start over to be safe.
                let _ = tokio::fs::remove_file(part).await;
                return Err(Outcome::Retry("The partial download didn't match the file on the server, so it starts over.".into()));
            }
            403 | 410 => return Err(Outcome::Retry("The download server's link ran out. Asking Nexus Mods for a new one.".into())),
            s if s >= 500 => return Err(Outcome::Retry(format!("The download server had a problem ({s})."))),
            s => return Err(Outcome::Failed(format!("The download server answered with an error ({s})."))),
        }
        let total = res.content_length().map(|l| l + if resumed { have } else { 0 }).unwrap_or(size);
        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(resumed)
            .truncate(!resumed)
            .open(part)
            .await
            .map_err(|e| Outcome::Failed(format!("Couldn't write to {}: {e}", part.display())))?;
        let mut received = if resumed { have } else { 0 };
        self.update(id, |j| {
            j.status = Status::Downloading;
            j.received = received;
            if total > 0 {
                j.size = total;
            }
        });
        let mut stream = res.bytes_stream();
        let mut last = Instant::now();
        let mut since = 0u64;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| Outcome::Retry(format!("Lost the connection while downloading. ({e})")))?;
            f.write_all(&chunk).await.map_err(|e| Outcome::Failed(format!("Couldn't write the download (is the drive full?): {e}")))?;
            received += chunk.len() as u64;
            since += chunk.len() as u64;
            self.throttle(chunk.len() as u64).await;
            let dt = last.elapsed();
            if dt >= Duration::from_millis(400) {
                let speed = (since as f64 / dt.as_secs_f64()) as u64;
                let snap = {
                    let mut st = self.state.lock().unwrap();
                    match st.jobs.iter_mut().find(|j| j.id == id) {
                        Some(j) if j.status == Status::Downloading => {
                            j.received = received;
                            j.speed = speed;
                            Some(j.public())
                        }
                        _ => None,
                    }
                };
                match snap {
                    Some(j) => self.emit_job(&j),
                    None => return Err(Outcome::Failed("Stopped.".into())),
                }
                last = Instant::now();
                since = 0;
            }
        }
        f.flush().await.map_err(|e| Outcome::Failed(e.to_string()))?;
        drop(f);
        if total > 0 && received != total {
            if received > total {
                let _ = tokio::fs::remove_file(part).await;
            }
            return Err(Outcome::Retry(format!("The download stopped early ({received} of {total} bytes).")));
        }
        self.update(id, |j| j.received = received);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! The queue against a small stand-in for Nexus Mods and its download servers, on this
    //! computer. Each file is 200 kB sent in 10 slow pieces, so downloads overlap when they can.
    use super::*;
    use crate::settings::Mode;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    const SIZE: usize = 200_000;

    fn byte(i: usize) -> u8 {
        (i % 251) as u8
    }

    #[derive(Clone, Default)]
    struct Seen {
        active: Arc<AtomicUsize>,
        peak: Arc<AtomicUsize>,
        order: Arc<Mutex<Vec<u64>>>,
        ranges: Arc<Mutex<Vec<u64>>>,
    }

    async fn serve(seen: Seen) -> String {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", l.local_addr().unwrap());
        let base2 = base.clone();
        tokio::spawn(async move {
            loop {
                let (mut s, _) = l.accept().await.unwrap();
                let seen = seen.clone();
                let base = base2.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let mut n = 0;
                    while !buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                        let r = s.read(&mut buf[n..]).await.unwrap();
                        if r == 0 {
                            return;
                        }
                        n += r;
                    }
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let path = req.split_whitespace().nth(1).unwrap_or("").split('?').next().unwrap().to_string();
                    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
                    let json = |body: String| format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    match parts.as_slice() {
                        ["v1", "games", "g", "mods", "1.json"] => {
                            let _ = s.write_all(json(r#"{"mod_id":1,"domain_name":"g","name":"Test mod","author":"Alex"}"#.into()).as_bytes()).await;
                        }
                        ["v1", "games", "g", "mods", "1", "files", f] if f.ends_with(".json") => {
                            let id = f.trim_end_matches(".json");
                            // Files 70 and up list their size only in whole kilobytes, rounded down, as
                            // Nexus Mods sometimes does.
                            let size = if id.parse::<u64>().unwrap_or(0) >= 70 { format!(r#""size_kb":{}"#, SIZE / 1024) } else { format!(r#""size_in_bytes":{SIZE}"#) };
                            let _ = s.write_all(json(format!(r#"{{"file_id":{id},"name":"File {id}","file_name":"file{id}.bin",{size},"category_name":"MAIN"}}"#)).as_bytes()).await;
                        }
                        ["v1", "games", "g", "mods", "1", "files", f, "download_link.json"] => {
                            let _ = s.write_all(json(format!(r#"[{{"name":"Test","short_name":"T","URI":"{base}/dl/{f}"}}]"#)).as_bytes()).await;
                        }
                        ["dl", f] => {
                            let id: u64 = f.parse().unwrap();
                            let from = req.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("range: bytes=").map(String::from)).and_then(|r| r.trim_end_matches('-').trim().trim_end_matches('-').parse::<usize>().ok()).unwrap_or(0);
                            seen.order.lock().unwrap().push(id);
                            seen.ranges.lock().unwrap().push(from as u64);
                            let now = seen.active.fetch_add(1, Ordering::SeqCst) + 1;
                            seen.peak.fetch_max(now, Ordering::SeqCst);
                            let head = if from > 0 {
                                format!("HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {from}-{}/{SIZE}\r\nConnection: close\r\n\r\n", SIZE - from, SIZE - 1)
                            } else {
                                format!("HTTP/1.1 200 OK\r\nContent-Length: {SIZE}\r\nConnection: close\r\n\r\n")
                            };
                            let _ = s.write_all(head.as_bytes()).await;
                            let body: Vec<u8> = (from..SIZE).map(byte).collect();
                            for piece in body.chunks(SIZE / 10) {
                                tokio::time::sleep(Duration::from_millis(40)).await;
                                if s.write_all(piece).await.is_err() {
                                    break;
                                }
                            }
                            seen.active.fetch_sub(1, Ordering::SeqCst);
                        }
                        _ => {
                            let _ = s.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                        }
                    }
                });
            }
        });
        base
    }

    async fn setup(mode: Mode, max: u32) -> (Queue, Seen, tempfile::TempDir, Arc<Mutex<Settings>>) {
        let seen = Seen::default();
        let base = serve(seen.clone()).await;
        let dir = tempfile::tempdir().unwrap();
        let settings = Arc::new(Mutex::new(Settings {
            library_dir: dir.path().join("lib").to_string_lossy().into_owned(),
            mode,
            max_concurrent: max,
            ..Settings::default()
        }));
        let nexus = Nexus::new(Some("test-key".into())).with_base(&format!("{base}/v1"));
        let q = Queue::new(None, dir.path().to_path_buf(), settings.clone(), nexus);
        q.start();
        (q, seen, dir, settings)
    }

    async fn wait_until(q: &Queue, what: &str, ok: impl Fn(&[Job]) -> bool) {
        // Long enough to cover a retry after a dropped connection (5 seconds, then 10).
        for _ in 0..800 {
            if ok(&q.list()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("timed out waiting for {what}: {:#?}", q.list().iter().map(|j| (j.file_id, j.status, j.error.clone())).collect::<Vec<_>>());
    }

    fn all_done(jobs: &[Job]) -> bool {
        !jobs.is_empty() && jobs.iter().all(|j| j.status == Status::Done)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sequential_runs_one_at_a_time_in_order() {
        let (q, seen, dir, _) = setup(Mode::Sequential, 3).await;
        for f in 1..=4 {
            q.add("g", 1, f, None, None, None);
        }
        wait_until(&q, "all done", all_done).await;
        assert_eq!(seen.peak.load(Ordering::SeqCst), 1);
        assert_eq!(*seen.order.lock().unwrap(), vec![1, 2, 3, 4]);
        let stored = library::list(&dir.path().join("lib"));
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].name, "Test mod");
        assert_eq!(stored[0].files.len(), 4);
        let file = dir.path().join("lib/g/1 - Test mod/file3.bin");
        let data = std::fs::read(file).unwrap();
        assert_eq!(data.len(), SIZE);
        assert!(data.iter().enumerate().all(|(i, b)| *b == byte(i)));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_size_in_kilobytes_is_not_a_short_download() {
        // The listed size is 200_000 / 1024 = 195 kB = 199_680 bytes; the file is 200_000.
        let (q, _seen, dir, _) = setup(Mode::Sequential, 1).await;
        q.add("g", 1, 70, None, None, None);
        wait_until(&q, "done", all_done).await;
        assert_eq!(q.list()[0].size, SIZE as u64);
        assert_eq!(std::fs::read(dir.path().join("lib/g/1 - Test mod/file70.bin")).unwrap().len(), SIZE);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn concurrent_runs_up_to_the_limit() {
        let (q, seen, _dir, _) = setup(Mode::Concurrent, 3).await;
        for f in 1..=5 {
            q.add("g", 1, f, None, None, None);
        }
        wait_until(&q, "all done", all_done).await;
        assert_eq!(seen.peak.load(Ordering::SeqCst), 3);
        // Files of one mod finishing together are all noted in its nexus.json.
        assert_eq!(library::list(&_dir.path().join("lib"))[0].files.len(), 5);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn switching_mode_takes_effect_for_the_next_file() {
        let (q, seen, _dir, settings) = setup(Mode::Sequential, 4).await;
        for f in 1..=6 {
            q.add("g", 1, f, None, None, None);
        }
        wait_until(&q, "the first to start", |j| j.iter().any(|j| j.status == Status::Downloading)).await;
        assert_eq!(seen.peak.load(Ordering::SeqCst), 1);
        settings.lock().unwrap().mode = Mode::Concurrent;
        q.poke();
        wait_until(&q, "all done", all_done).await;
        assert_eq!(seen.peak.load(Ordering::SeqCst), 4);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn pause_and_resume_picks_up_where_it_stopped() {
        let (q, seen, dir, _) = setup(Mode::Sequential, 1).await;
        let job = q.add("g", 1, 9, None, None, None);
        wait_until(&q, "downloading", |j| j[0].status == Status::Downloading).await;
        tokio::time::sleep(Duration::from_millis(150)).await;
        q.pause(&job.id);
        assert_eq!(q.list()[0].status, Status::Paused);
        tokio::time::sleep(Duration::from_millis(100)).await;
        let part = q.part_path(&job.id);
        let had = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        assert!(had > 0 && (had as usize) < SIZE, "partial file should be kept, had {had}");
        q.resume(&job.id);
        wait_until(&q, "done", all_done).await;
        assert_eq!(*seen.ranges.lock().unwrap(), vec![0, had]);
        let data = std::fs::read(dir.path().join("lib/g/1 - Test mod/file9.bin")).unwrap();
        assert_eq!(data.len(), SIZE);
        assert!(data.iter().enumerate().all(|(i, b)| *b == byte(i)), "resumed file is intact");
        assert!(!part.exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn automatic_mode_follows_the_account() {
        let (q, seen, _dir, _) = setup(Mode::Auto, 3).await;
        for f in 1..=3 {
            q.add("g", 1, f, None, None, None);
        }
        wait_until(&q, "all done", all_done).await;
        assert_eq!(seen.peak.load(Ordering::SeqCst), 1, "free account: one at a time");
        q.set_premium(true);
        for f in 4..=6 {
            q.add("g", 1, f, None, None, None);
        }
        wait_until(&q, "all done", all_done).await;
        assert_eq!(seen.peak.load(Ordering::SeqCst), 3, "Premium: several at once");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_speed_limit_covers_all_downloads_together() {
        let (q, _seen, _dir, settings) = setup(Mode::Concurrent, 3).await;
        // 3 files of 200 kB at 400 kB/s together: at least 1.5 seconds (with a little slack).
        settings.lock().unwrap().speed_limit = 400_000;
        let t = Instant::now();
        for f in 1..=3 {
            q.add("g", 1, f, None, None, None);
        }
        wait_until(&q, "all done", all_done).await;
        assert!(t.elapsed() >= Duration::from_millis(1300), "took {:?}", t.elapsed());
        assert_eq!(size_words(1_240_000_000), "1.24 GB");
        assert_eq!(size_words(512), "512 bytes");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_restart_keeps_the_list_and_carries_on() {
        let (q, _seen, dir, settings) = setup(Mode::Sequential, 1).await;
        let job = q.add("g", 1, 5, Some("secret".into()), Some(now() + 3600), None);
        q.pause(&job.id);
        let saved = std::fs::read_to_string(dir.path().join("queue.json")).unwrap();
        assert!(saved.contains("\"paused\""));
        // The page never sees the key.
        assert!(q.list()[0].key.is_none());
        let again = Queue::new(None, dir.path().to_path_buf(), settings.clone(), Nexus::new(None));
        assert_eq!(again.list().len(), 1);
        assert_eq!(again.list()[0].status, Status::Paused);

        // One waiting to try again when the app closed goes back in line (its timer is gone).
        let file = dir.path().join("queue.json");
        std::fs::write(&file, std::fs::read_to_string(&file).unwrap().replace("\"paused\"", "\"waiting\"")).unwrap();
        let third = Queue::new(None, dir.path().to_path_buf(), settings, Nexus::new(None));
        assert_eq!(third.list()[0].status, Status::Queued);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn sorts_waiting_files_by_size() {
        let (q, _seen, _dir, _settings) = setup(Mode::Sequential, 1).await;
        let ids: Vec<String> = (1..=4).map(|f| q.add("g", 1, f, None, None, None).id).collect();
        q.pause_all();
        for (id, size) in ids.iter().zip([300u64, 0, 100, 200]) {
            q.update(id, |j| j.size = size);
        }
        q.sort_by_size(true);
        assert_eq!(q.list().iter().map(|j| j.size).collect::<Vec<_>>(), vec![100, 200, 300, 0]);
        q.sort_by_size(false);
        assert_eq!(q.list().iter().map(|j| j.size).collect::<Vec<_>>(), vec![300, 200, 100, 0]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn download_now_steps_the_running_one_aside() {
        let (q, seen, _dir, settings) = setup(Mode::Sequential, 1).await;
        settings.lock().unwrap().speed_limit = 300_000;
        let first = q.add("g", 1, 1, None, None, None);
        q.add("g", 1, 2, None, None, None);
        let third = q.add("g", 1, 3, None, None, None);
        wait_until(&q, "the first one downloading", |l: &[Job]| l.iter().any(|j| j.id == first.id && j.received > 0)).await;
        q.now(&third.id);
        // Still one at a time, right after the switch.
        assert!(q.list().iter().filter(|j| j.status.running()).count() <= 1);
        assert_eq!(q.list()[0].id, third.id);
        assert_eq!(q.list()[1].id, first.id);
        wait_until(&q, "all done", all_done).await;
        let order = seen.order.lock().unwrap().clone();
        let at = |f: u64| order.iter().position(|&x| x == f).unwrap();
        assert!(at(3) < at(2), "the file asked for now went before the one waiting: {order:?}");
        assert_eq!(order.iter().filter(|&&x| x == 1).count(), 2, "the stepped-aside file carried on later: {order:?}");
        assert!(seen.ranges.lock().unwrap().iter().any(|&r| r > 0), "and picked up from what it had");
    }
}
