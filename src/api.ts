// The bridge to the app's Rust side (src-tauri). In a plain browser (the demo, screenshots)
// the same calls go to a made-up stand-in in demo.ts instead.

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Mode = "sequential" | "concurrent" | "auto";

export interface Settings {
  libraryDir: string;
  mode: Mode;
  maxConcurrent: number;
  extract: boolean;
  keepArchives: boolean;
  autoRetry: boolean;
  /** Bytes a second for all downloads together; 0 is no limit. */
  speedLimit: number;
  /** Open in the background when you sign in, to carry on with unfinished downloads. */
  startWithComputer?: boolean;
  /** Keep the API key in the desktop keyring instead of a file. */
  keyInKeyring?: boolean;
  /** Check each finished download against Nexus Mods' checksum. */
  verifyDownloads?: boolean;
  games: { domain: string; name: string; path?: string | null; modFolder?: string | null; launch?: string | null }[];
}

export interface User {
  userId: number;
  name: string;
  isPremium: boolean;
  profileUrl?: string | null;
}

export interface Overview {
  settings: Settings;
  hasKey: boolean;
  user: User | null;
  limits: { dailyRemaining?: number | null; hourlyRemaining?: number | null };
  version: string;
  /** Free space and size of the library's drive, in bytes. */
  freeSpace?: number | null;
  totalSpace?: number | null;
}

/** What this computer has for Zom's Nexus (Settings > This computer). */
export interface SystemReport {
  distro: string;
  items: { what: string; ok: boolean; detail: string; fix?: string | null }[];
}

export interface FileUsage {
  game: string;
  modId: number;
  fileId: number;
  archive: number;
  unpacked: number;
}

/** What Nexus Mods says about a mod: what it needs, and whether it's current. */
export interface ModFacts {
  modId: number;
  game: string;
  name: string;
  status: string;
  updatedAt?: number | null;
  requires: { modId: number; name: string; url?: string | null; otherGame: boolean; notes?: string | null }[];
  dlc: string[];
}

/** A Steam library folder, searched for games. */
export interface SteamLibrary {
  path: string;
  installed: number;
  nexusGames: string[];
  freeSpace?: number | null;
  totalSpace?: number | null;
}

export type Status = "queued" | "starting" | "downloading" | "storing" | "paused" | "waiting" | "needsLink" | "failed" | "done";

export interface Job {
  id: string;
  game: string;
  modId: number;
  fileId: number;
  modName: string;
  fileName: string;
  name: string;
  version?: string | null;
  size: number;
  received: number;
  status: Status;
  error?: string | null;
  addedAt: number;
  finishedAt?: number | null;
  path?: string | null;
  speed: number;
  retries: number;
  /** A link from the website's Mod manager download button has come in for this file. */
  hasLink?: boolean;
}

export interface ModInfo {
  modId: number;
  game: string;
  name: string;
  summary?: string | null;
  version?: string | null;
  author?: string | null;
  pictureUrl?: string | null;
  adult: boolean;
  endorsements?: number | null;
  downloads?: number | null;
  updated?: number | null;
}

export interface FileInfo {
  fileId: number;
  name: string;
  fileName: string;
  version?: string | null;
  category?: string | null;
  size: number;
  uploaded?: number | null;
  isPrimary: boolean;
  description?: string | null;
}

export interface StoredFile {
  fileId: number;
  name: string;
  fileName: string;
  version?: string | null;
  category?: string | null;
  size: number;
  downloadedAt: number;
  archive?: string | null;
  unpacked?: string | null;
  /** Matched Nexus Mods' checksum (true), didn't (false), or not checked. */
  verified?: boolean | null;
}

export interface StoredMod {
  game: string;
  modId: number;
  name: string;
  author?: string | null;
  version?: string | null;
  summary?: string | null;
  pictureUrl?: string | null;
  files: StoredFile[];
  /** Added from a file on this computer, not from Nexus Mods. */
  local?: boolean;
  folder: string;
}

export interface Game {
  domain: string;
  name: string;
  path?: string | null;
  source: "steam" | "added";
  steamAppId?: number | null;
  protonPrefix?: string | null;
  modFolder?: string | null;
  launch?: string | null;
  activeProfile?: string | null;
  /** Free space and size of the drive the game is on, in bytes. */
  freeSpace?: number | null;
  totalSpace?: number | null;
}

export interface Update {
  game: string;
  modId: number;
  modName: string;
  fileId: number;
  newer?: FileInfo | null;
  outdated: boolean;
}

export interface UpdateReport {
  updates: Update[];
  checked: number;
  problems: [string, string][];
  at: number;
}

export interface CollectionFile {
  game: string;
  modId: number;
  fileId: number;
  modName: string;
  name: string;
  version?: string | null;
  size: number;
  optional: boolean;
}

export interface Collection {
  game: string;
  slug: string;
  revision?: number | null;
  name: string;
  author?: string | null;
  summary?: string | null;
  pictureUrl?: string | null;
  files: CollectionFile[];
}

export interface ProfileMod {
  modId: number;
  fileId: number;
  enabled: boolean;
}

export interface Profile {
  id: string;
  name: string;
  game: string;
  mods: ProfileMod[];
  collection?: string | null;
  /** Leave requirements alone (don't add, switch on or sort them by themselves). */
  manualRequirements?: boolean;
  /** Finished downloads for this game go into this profile by themselves. */
  newDownloads?: boolean;
}

/** What putting a profile's requirements in changed. */
export interface Resolved {
  profile: Profile;
  added: string[];
  switchedOn: string[];
  moved: number;
  /** Requirements not downloaded: [mod number, name]. */
  missing: [number, string][];
}

export interface CheckReport {
  conflicts: { winner: string; losers: string[]; files: number; examples: string[] }[];
  plugins: { plugin: string; modName: string; master: string; kind: "missing" | "order" }[];
  loadOrder: string[];
  missing: string[];
  installers: string[];
  files: number;
  at: number;
  /** What the mods need that the profile doesn't have switched on (from Nexus Mods). */
  needs: { name: string; modId: number; kind: "notDownloaded" | "notInProfile" | "switchedOff" | "external" | "otherGame" | "dlc"; url?: string | null; notes?: string | null; neededBy: string[] }[];
  /** Mods that may not work with the game as it is now. */
  compat: { modId: number; modName: string; kind: "newerFile" | "notUpdated" | "removed"; fileDate?: number | null; modUpdated?: number | null }[];
  requirementsChecked: boolean;
  /** Mods with programs or scripts in them. */
  programs?: { modName: string; files: string[]; count: number }[];
  gameUpdated?: number | null;
}

export interface Applied {
  files: number;
  mods: number;
  backedUp: number;
  plugins: number;
  installers: string[];
  missing: string[];
  clashes: number;
}

export interface LinkStatus {
  ours: boolean;
  other?: string | null;
}

export const isDesktop = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Demo = typeof import("./demo");
let demo: Promise<Demo> | null = null;
const demoApi = () => (demo ??= import("./demo"));

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!isDesktop) return (await demoApi()).call(cmd, args) as Promise<T>;
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw new Error(typeof e === "string" ? e : e instanceof Error ? e.message : String(e));
  }
}

export async function on<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  if (!isDesktop) return (await demoApi()).on(event, handler as (p: unknown) => void);
  return listen<T>(event, (e) => handler(e.payload));
}

export const api = {
  overview: () => call<Overview>("get_overview"),
  saveSettings: (settings: Settings) => call<Overview>("save_settings", { settings }),
  setApiKey: (key: string) => call<Overview>("set_api_key", { key }),
  forgetApiKey: () => call<Overview>("forget_api_key"),
  refreshAccount: () => call<Overview>("refresh_account"),
  lookupMod: (address: string) => call<{ mod: ModInfo; files: FileInfo[] }>("lookup_mod", { address }),

  queue: () => call<Job[]>("queue_list"),
  addLink: (link: string) => call<Job>("queue_add_link", { link }),
  addFiles: (address: string, fileIds: number[]) => call<Job[]>("queue_add_files", { address, fileIds }),
  pause: (id: string) => call<void>("queue_pause", { id }),
  resume: (id: string) => call<void>("queue_resume", { id }),
  remove: (id: string) => call<void>("queue_remove", { id }),
  pauseAll: () => call<void>("queue_pause_all"),
  resumeAll: () => call<void>("queue_resume_all"),
  /** First in the list and started straight away. */
  now: (id: string) => call<void>("queue_now", { id }),
  sortBySize: (smallestFirst: boolean) => call<void>("queue_sort", { smallestFirst }),
  retryFailed: () => call<void>("queue_retry_failed"),
  /** What these mods (of one game) need, in one request. */
  modRequirements: (game: string, modIds: number[]) => call<ModFacts[]>("mod_requirements", { game, modIds }),
  /** Queue the main file of each of these mods. */
  queueRequirements: (game: string, modIds: number[]) => call<{ queued: string[]; skipped: string[] }>("queue_requirements", { game, modIds }),
  clearFinished: () => call<void>("queue_clear_finished"),
  reorder: (id: string, to: number) => call<void>("queue_reorder", { id, to }),

  library: () => call<StoredMod[]>("library_list"),
  unpack: (game: string, modId: number, fileId: number) => call<void>("library_unpack", { game, modId, fileId }),
  removeFile: (game: string, modId: number, fileId: number) => call<void>("library_remove_file", { game, modId, fileId }),
  removeMod: (game: string, modId: number) => call<void>("library_remove_mod", { game, modId }),
  importMod: (game: string, path: string) => call<StoredMod>("library_import", { game, path }),
  /** The space each library file takes on disk. */
  usage: () => call<FileUsage[]>("library_usage"),
  /** Delete a file's archive, keeping its unpacked folder. Gives the bytes freed. */
  dropArchive: (game: string, modId: number, fileId: number) => call<number>("library_drop_archive", { game, modId, fileId }),
  openFolder: (opts: { game?: string; modId?: number; install?: boolean } = {}) => call<void>("open_folder", opts),

  games: () => call<Game[]>("games_list"),
  steamLibraries: () => call<SteamLibrary[]>("steam_libraries"),
  /** Show a Nexus Mods page in the app's own Nexus Mods window (the browser in the demo). */
  nexusWindow: (url: string) => (isDesktop ? call<void>("nexus_window", { url }) : openUrl(url)),
  closeNexusWindow: () => (isDesktop ? call<void>("nexus_window_close") : Promise.resolve()),
  addGame: (domain: string, name?: string, path?: string, modFolder?: string, launch?: string) => call<void>("games_add", { domain, name, path, modFolder, launch }),
  art: (domain: string, kind: "cover" | "hero" | "logo") => call<string | null>("game_art", { domain, kind }),
  launch: (domain: string) => call<void>("game_launch", { domain }),

  checkUpdates: () => call<UpdateReport>("library_check_updates"),
  lastUpdates: () => call<UpdateReport | null>("library_updates"),
  add: (game: string, modId: number, fileId: number) => call<Job>("queue_add", { game, modId, fileId }),
  browse: (game: string, list: "trending" | "latest_added" | "latest_updated") => call<ModInfo[]>("browse", { game, list }),
  modFiles: (game: string, modId: number) => call<{ mod: ModInfo; files: FileInfo[] }>("mod_files", { game, modId }),
  lookupCollection: (address: string) => call<Collection>("lookup_collection", { address }),
  addCollection: (address: string, optional: boolean) => call<{ queued: number; skipped: number; profile: Profile }>("queue_add_collection", { address, optional }),

  profiles: () => call<Profile[]>("profiles_list"),
  saveProfile: (profile: Profile) => call<Profile>("profile_save", { profile }),
  /** Put what the profile's mods need into it, switched on and sorted above them. */
  resolveProfile: (id: string) => call<Resolved>("profile_resolve", { id }),
  /** Newer versions of the files a profile uses. */
  profileUpdates: (id: string) => call<UpdateReport>("profile_updates", { id }),
  /** Your notes on mods, by "game:mod number". */
  notes: () => call<Record<string, string>>("mod_notes"),
  setNote: (game: string, modId: number, note: string) => call<void>("set_mod_note", { game, modId, note }),
  /** Save a report for getting help (no API key in it). */
  diagnostics: (path: string) => call<void>("diagnostics", { path }),
  deleteProfile: (id: string) => call<void>("profile_delete", { id }),
  checkProfile: (id: string) => call<CheckReport>("profile_check", { id }),
  profileChecks: () => call<Record<string, CheckReport>>("profile_checks"),
  applyProfile: (id: string) => call<Applied>("profile_apply", { id }),
  clearGame: (game: string) => call<void>("profile_clear", { game }),
  play: (id: string) => call<Applied>("profile_play", { id }),
  exportPack: (id: string, path: string) => call<void>("profile_export", { id, path }),
  importPack: (path: string) => call<{ profile: Profile; queued: number }>("profile_import", { path }),
  removeGame: (domain: string) => call<void>("games_remove", { domain }),

  linkStatus: () => call<LinkStatus>("nxm_status"),
  registerLinks: () => call<LinkStatus>("nxm_register"),
  systemCheck: () => call<SystemReport>("system_check"),
};

/** Open a web page in the person's browser. */
export async function openUrl(url: string) {
  if (!isDesktop) {
    window.open(url, "_blank", "noopener");
    return;
  }
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}

/** Pick a folder (the library, a game's install folder). */
export async function pickFolder(title: string, defaultPath?: string): Promise<string | null> {
  if (!isDesktop) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: true, multiple: false, title, defaultPath });
  return typeof r === "string" ? r : null;
}

/** Where to save a file (a mod pack). */
export async function pickSave(title: string, defaultPath: string): Promise<string | null> {
  if (!isDesktop) return null;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return (await save({ title, defaultPath, filters: [{ name: "Mod pack", extensions: ["json"] }] })) ?? null;
}

/** A file to open: a mod pack, or a mod's archive. */
export async function pickFile(title: string, kind: "pack" | "mod" = "pack"): Promise<string | null> {
  if (!isDesktop) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const filters = kind === "pack" ? [{ name: "Mod pack", extensions: ["json"] }] : [{ name: "Mod archives", extensions: ["zip", "7z", "rar"] }];
  const r = await open({ title, multiple: false, filters });
  return typeof r === "string" ? r : null;
}

export const modUrl = (game: string, modId: number) => `https://www.nexusmods.com/${game}/mods/${modId}`;
/** A file's mod-manager download page: its Slow download (or Premium's Fast download) button
 * hands the file to Zom's Nexus through an nxm link. Without nmm=1 it's the manual download,
 * which only saves the file in the browser. */
export const fileUrl = (game: string, modId: number, fileId: number) => `${modUrl(game, modId)}?tab=files&file_id=${fileId}&nmm=1`;
