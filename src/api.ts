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
  /** Mods marked adult: shown like others, pictures blurred, or hidden in Browse too. */
  adultContent?: "show" | "blur" | "hide";
  /** Request saver: Browse waits for a click before asking Nexus Mods. */
  requestSaver?: boolean;
  /** After the quick tries, keep trying later (5 min, 30 min, 2 h) instead of failing. */
  retryLater?: boolean;
  /** Only start downloads between these hours (local, 0–23); equal means any time. */
  hoursFrom?: number;
  hoursTo?: number;
  /** While a game runs: carry on, hold new downloads, or pause them all. */
  whilePlaying?: "carry" | "hold" | "pause";
  /** A second speed limit for some hours (bytes a second, 0 = off). */
  timedLimit?: number;
  timedFrom?: number;
  timedTo?: number;
  /** Open in the tray instead of a window. */
  startHidden?: boolean;
  /** Carry on with unfinished downloads at start, or keep them paused. */
  resumeOnStart?: "yes" | "no";
  /** Activity kinds not noted. */
  activityOff?: string[];
  /** Games whose mods live in another folder than the library (changed by moving them). */
  gameLibraries?: Record<string, string>;
  /** Games whose mods are copied in, never hard-linked. */
  copyGames?: string[];
  games: { domain: string; name: string; path?: string | null; modFolder?: string | null; launch?: string | null }[];
  /** Snapshot a game's saves by itself before each Play. */
  snapshotBeforePlay?: boolean;
  batteryFloor?: number;
  unpackOnPower?: boolean;
  /** Ask GitHub once a day whether a newer Zom's Modzilla is out. */
  checkAppUpdates?: boolean;
}

export interface User {
  userId: number;
  name: string;
  isPremium: boolean;
  profileUrl?: string | null;
}

export interface Overview {
  settings: Settings;
  /** Why new downloads wait now (download hours, a game running). */
  held?: string | null;
  hasKey: boolean;
  user: User | null;
  limits: { dailyRemaining?: number | null; hourlyRemaining?: number | null };
  version: string;
  /** Free space and size of the library's drive, in bytes. */
  freeSpace?: number | null;
  totalSpace?: number | null;
}

/** What this computer has for Zom's Modzilla (Settings > This computer). */
export interface SystemReport {
  distro: string;
  items: { what: string; ok: boolean; detail: string; fix?: string | null }[];
}

/** Something worth knowing about later, on the Activity page. */
export interface ActivityEvent {
  id: number;
  at: number;
  kind: "failed" | "updates" | "gone" | "worse" | "saves" | "played" | "done" | string;
  title: string;
  detail?: string | null;
  game?: string | null;
  page?: "downloads" | "library" | "profiles" | null;
  profile?: string | null;
  read: boolean;
}

/** Where a game is after Play. */
export interface GameState {
  game: string;
  state: "applying" | "starting" | "running" | "notStarted" | "stopped" | "failed";
  since: number;
  profile?: string | null;
  /** Why it failed, or how long it ran. */
  detail?: string | null;
}

/** Time spent in the game after Play, per profile. */
export interface PlayTime {
  seconds: number;
  sessions: number;
  lastPlayed?: number | null;
}

/** What fills a drive (Settings and the bottom left). */
export interface DriveUse {
  holds: string[];
  total: number;
  free: number;
  mods: number;
  archives: number;
  steam: number;
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
  /** Marked as adult content on Nexus Mods. */
  adult?: boolean;
  /** Its category on Nexus Mods. */
  category?: string | null;
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
  /** The choices made in the file's installer (FOMOD), when it has been run. */
  installed?: { folder: string; choices: FomodChoice[] } | null;
  /** The unpacked file has an installer (FOMOD). */
  hasInstaller?: boolean;
}

/** A condition from a mod installer; file and version checks come already answered. */
export type FomodCond = { all: FomodCond[] } | { any: FomodCond[] } | { flag: string; value: string } | { fixed: boolean };
export interface FomodPlugin {
  name: string;
  description: string;
  image?: string | null;
  flags: [string, string][];
  /** Required, Recommended, Optional, CouldBeUsable or NotUsable */
  kind: string;
  patterns: { when: FomodCond; kind: string }[];
}
export interface FomodGroup {
  name: string;
  /** SelectExactlyOne, SelectAtMostOne, SelectAtLeastOne, SelectAll or SelectAny */
  kind: string;
  plugins: FomodPlugin[];
}
export interface FomodStep {
  name: string;
  visible?: FomodCond | null;
  groups: FomodGroup[];
}
export interface FomodChoice {
  step: string;
  group: string;
  plugins: string[];
}
export interface Installer {
  name: string;
  image?: string | null;
  steps: FomodStep[];
  previous?: FomodChoice[] | null;
  current?: FomodChoice[] | null;
  /** `previous` are the collection curator's picks. */
  fromCollection?: boolean;
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
  /** Marked as adult content on Nexus Mods when it was downloaded. */
  adult?: boolean;
  /** Where it came from when not Nexus Mods: "thunderstore:Owner-Name". */
  source?: string | null;
  folder: string;
}

export interface Game {
  domain: string;
  name: string;
  path?: string | null;
  source: "steam" | "added" | "heroic" | "lutris";
  steamAppId?: number | null;
  protonPrefix?: string | null;
  modFolder?: string | null;
  launch?: string | null;
  activeProfile?: string | null;
  /** Free space and size of the drive the game is on, in bytes. */
  freeSpace?: number | null;
  totalSpace?: number | null;
  /** When the game's mods go in through its own tool (not a folder): what to say. */
  modTool?: string | null;
  /** The library is on another drive, so mods are copied into this game rather than linked. */
  modsCopied?: boolean;
  /** Where the game keeps its saves (set by you, or known), for saves per profile. */
  saveFolder?: string | null;
  /** Steam keeps this game's saves in its cloud (it puts saves back into an emptied folder). */
  steamCloud?: boolean;
  /** The Wine/Proton build it runs with ("Proton 9.0-302", "GE-Proton9-20"). */
  proton?: string | null;
}

/** A web search that answers "where do this game's mods go". */
export const modFolderHelp = (gameName: string) => `https://www.google.com/search?q=${encodeURIComponent(`${gameName} mods folder location install mods`)}`;

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
  /** Keeps its own saves instead of sharing the usual ones. */
  ownSaves?: boolean;
  /** The collection at its revision, kept apart from your changes (only the app changes it). */
  base?: { slug: string; revision?: number | null; name: string; optional?: boolean; files: { modId: number; fileId: number }[] } | null;
  /** Made from a Thunderstore mod pack: [Owner-Name, version]. */
  tsPack?: [string, string] | null;
  addons?: unknown[];
  /** "Load this after that" rules, kept whenever the order changes. */
  rules?: { modId: number; after: number }[];
  /** Your notes on the profile. */
  note?: string | null;
}

/** SMAPI's last run, worked out by the app (Stardew Valley). */
export interface Smapi {
  at: number;
  smapiVersion?: string | null;
  gameVersion?: string | null;
  /** Root causes: mods SMAPI wouldn't load for their own reason, and who needed them. */
  broken: { name: string; version: string; modId?: number | null; kind: "old" | "missing" | "other"; why: string; needs: string[]; dependents: string[] }[];
  skipped: number;
  updates: { name: string; have: string; version: string; nexusId?: number | null }[];
  errors: [string, number][];
  /** Mods named in the details of errors, most often first. */
  culprits: { name: string; modId?: number | null; count: number; what: string; atTheEnd: boolean }[];
  crashed: boolean;
}

/** A game uninstalled while its mods are still in the library. */
export interface GoneGame {
  domain: string;
  name: string;
  path: string;
  mods: number;
  bytes: number;
  profiles: number;
}

/** What went into a game at one time, to go back to. */
export interface RestorePoint {
  at: number;
  profileId: string;
  profileName: string;
  mods: ProfileMod[];
  rules?: { modId: number; after: number }[];
  /** Why it changed, in your words. */
  note?: string | null;
}

/** One set of changes on top of a collection: an add-on, or your own changes. */
export interface Layer {
  id: string;
  name: string;
  enabled: boolean;
  added: number[];
  off: number[];
  files: number[];
}

export interface Layers {
  base?: { slug: string; revision?: number | null; name: string; count: number } | null;
  /** The collection a profile without a base came from, so it can be linked. */
  collection?: string | null;
  revision?: number | null;
  addons: Layer[];
  yours?: Layer | null;
  names: Record<string, string>;
}

export interface CollectionUpdate {
  from?: number | null;
  to?: number | null;
  changes: { added: number[]; removed: number[]; updated: number[]; kept: number[]; joined: number[] };
  names: Record<string, string>;
  queued: number;
  profile?: Profile | null;
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
  needs: { name: string; modId: number; kind: "notDownloaded" | "notInProfile" | "switchedOff" | "external" | "otherGame" | "dlc" | "unavailable" | "optional"; url?: string | null; notes?: string | null; neededBy: string[] }[];
  /** Mods that may not work with the game as it is now. */
  compat: { modId: number; modName: string; kind: "newerFile" | "notUpdated" | "removed"; fileDate?: number | null; modUpdated?: number | null }[];
  requirementsChecked: boolean;
  /** Mods with programs or scripts in them. */
  programs?: { modName: string; files: string[]; count: number }[];
  /** Mod loader DLLs (RED4ext's winmm.dll, …) the Steam launch options don't let run under Proton. */
  launchFix?: { missing: string[]; current: string; suggested: string } | null;
  /** This profile is in the game, but its files changed since. */
  outOfDate?: boolean;
  /** What the game's own logs said the last time it started with this profile. */
  gameLog?: {
    at: number;
    beforeChange?: boolean;
    /** Mods whose files ArchiveXL or TweakXL complained about; nearCrash ones first. */
    assets?: { modId: number; modName: string; count: number; sample: string; nearCrash: boolean }[];
    crash?: { at: number; minutes: number; place?: string | null; inGameCode: boolean; outOfMemory: boolean } | null;
    issues: { modId: number; modName: string; kind: "oldPlugin" | "needsNewer" | "needsMod" | "scripts" | "loaderRefused" | "loaderErrors"; otherId?: number | null; otherName?: string | null; detail: string }[]; smapi?: Smapi | null } | null;
  gameUpdated?: number | null;
  /** Mod loaders some mods need that aren't in the game or the profile. */
  loadersMissing?: { loader: string; mods: string[] }[];
  /** Mod loaders in the game or the profile. */
  loaders?: string[];
  /** The plugin order is one kept for the profile (sorted by LOOT). */
  sortedOrder?: boolean;
  /** Mods whose files sit inside an extra folder, so they land where the game won't look. */
  misplaced?: { modId: number; modName: string; extra: string; lands: string; should: string }[];
}

/** tModLoader's mods and mod packs. */
export interface TmlState {
  modsDir: string;
  mods: { name: string; label: string; workshopId?: number | null; enabled: boolean; size: number }[];
  packs: { name: string; mods: string[] }[];
  missing: string[];
}

/** A game on Thunderstore; `supported` when Zom's Modzilla can put its mods in (BepInEx). */
export interface TsGame {
  community: string;
  name: string;
  loader: string;
  supported: boolean;
}

/** A Thunderstore mod in search results. */
export interface TsFound {
  fullName: string;
  owner: string;
  name: string;
  description: string;
  icon: string;
  version: string;
  downloads: number;
  rating: number;
  size: number;
  url: string;
  deprecated: boolean;
  nsfw: boolean;
  modpack: boolean;
  categories: string[];
  updated: string;
  needs: number;
}

/** LOOT's sorted plugin order for a profile, and its notes. */
export interface LootProposal {
  now: string[];
  sorted: { order: string[]; changed: boolean; notes: { plugin: string; kind: "say" | "warn" | "error"; text: string }[]; unknown: number };
}

/** A copy of a game's saves. */
export interface Snapshot {
  id: string;
  at: number;
  label: string;
  owner: string;
  files: number;
  size: number;
  auto: boolean;
}

/** A Mod Organizer 2 folder: its profiles and how many mods. */
export interface Mo2Found {
  folder: string;
  profiles: string[];
  mods: number;
  gameName?: string | null;
}

/** A settings file of a profile's mods; `own` when the profile has its own version. */
export interface SettingFile {
  path: string;
  modId?: number | null;
  modName?: string | null;
  own: boolean;
}

/** The last time a profile was played, the game closed right after starting. */
export interface Trouble {
  at: number;
  secs: number;
  /** Runs in a row that closed quickly. */
  times: number;
  goodAt?: number | null;
  goodSecs?: number | null;
  added: { modId: number; name: string }[];
  removed: { modId: number; name: string }[];
  updated: { modId: number; name: string }[];
  /** The profile's mods changed since: maybe fixed already. */
  changedSince: boolean;
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
    const message = typeof e === "string" ? e : e instanceof Error ? e.message : String(e);
    // Every command that fails goes in the log too (never the arguments: they may hold a key).
    if (!cmd.startsWith("log_")) void invoke("log_page", { level: "info", message: `${cmd}: ${message}` }).catch(() => {});
    throw new Error(message);
  }
}

/** Put the page's own errors in the log. */
export function logPageErrors() {
  if (!isDesktop) return;
  const send = (message: string) => void invoke("log_page", { level: "error", message }).catch(() => {});
  window.addEventListener("error", (e) => send(`${e.message} (${e.filename}:${e.lineno})`));
  window.addEventListener("unhandledrejection", (e) => send(`Unhandled: ${e.reason instanceof Error ? e.reason.message : String(e.reason)}`));
}

export async function on<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  if (!isDesktop) return (await demoApi()).on(event, handler as (p: unknown) => void);
  return listen<T>(event, (e) => handler(e.payload));
}

export interface Extender {
  name: string;
  loader: string;
  launcher: string;
  on: boolean;
  installed: boolean;
  swapped: boolean;
}

export interface PrefixStatus {
  via: "protontricks" | "protontricksFlatpak" | "winetricks" | null;
  steam: boolean;
  hasPrefix: boolean;
  installed: string[];
  verbs: { verb: string; name: string; wantedBy: string }[];
  suggested: string[];
  flatpak: boolean;
  deck: boolean;
}

export interface Tool {
  id: string;
  game: string;
  name: string;
  exe: string;
  args: string[];
}

export interface ToolDone {
  id: string;
  game: string;
  name: string;
  ok: boolean;
  made: string[];
}

export interface LoaderOffer {
  id: string;
  name: string;
  what: string;
  dll: string;
}

export interface ModFileEntry {
  path: string;
  size: number;
  kind: "text" | "image" | "other";
  lostTo?: string | null;
}

export interface Marks {
  endorsed: [string, number][];
  tracked: [string, number][];
}

export const api = {
  extender: (game: string) => call<Extender | null>("extender_info", { game }),
  setExtender: (game: string, on: boolean) => call<Extender | null>("extender_set", { game, on }),
  prefixStatus: (game: string) => call<PrefixStatus>("prefix_status", { game }),
  prefixInstall: (game: string, verb: string) => call<string>("prefix_install", { game, verb }),
  tools: (game: string) => call<Tool[]>("tools_list", { game }),
  saveTool: (tool: Tool) => call<Tool>("tool_save", { tool }),
  deleteTool: (id: string) => call<void>("tool_delete", { id }),
  runTool: (id: string) => call<void>("tool_run", { id }),
  keepToolFiles: (game: string, name: string, profile: string | null) => call<StoredMod>("tool_keep", { game, name, profile }),
  loaderOffers: (game: string) => call<LoaderOffer[]>("loader_offers", { game }),
  installLoader: (game: string, id: string) => call<StoredMod>("loader_install", { game, id }),
  marks: (fresh = false) => call<Marks>("nexus_marks", { fresh }),
  endorse: (game: string, modId: number, on: boolean) => call<void>("mod_endorse", { game, modId, on }),
  track: (game: string, modId: number, on: boolean) => call<void>("mod_track", { game, modId, on }),
  modContents: (game: string, modId: number, profile: string | null) => call<ModFileEntry[]>("mod_contents", { game, modId, profile }),
  modFileRead: (game: string, modId: number, path: string, profile: string | null) => call<{ text?: string | null; image?: string | null; cut: boolean }>("mod_file_read", { game, modId, path, profile }),
  remoteStart: () => call<{ url: string; width: number; cells: boolean[] }>("remote_start"),
  remoteStop: () => call<void>("remote_stop"),
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
  activity: () => call<ActivityEvent[]>("activity_list"),
  markActivity: (clear: boolean) => call<void>("activity_mark", { clear }),
  savesCopies: (game: string) => call<number>("saves_copies", { game }),
  savesMoveCopies: (game: string) => call<number>("saves_move_copies", { game }),
  gameStates: () => call<GameState[]>("game_states"),
  playTimes: () => call<Record<string, PlayTime>>("profile_play_times"),
  diskUsage: (game?: string | null) => call<DriveUse[]>("disk_usage", { game: game ?? null }),
  /** Delete a file's archive, keeping its unpacked folder. Gives the bytes freed. */
  dropArchive: (game: string, modId: number, fileId: number) => call<number>("library_drop_archive", { game, modId, fileId }),
  openFolder: (opts: { game?: string; modId?: number; install?: boolean } = {}) => call<void>("open_folder", opts),

  games: () => call<Game[]>("games_list"),
  steamLibraries: () => call<SteamLibrary[]>("steam_libraries"),
  /** Show a Nexus Mods page in the app's own Nexus Mods window (the browser in the demo). */
  nexusWindow: (url: string) => (isDesktop ? call<void>("nexus_window", { url }) : openUrl(url)),
  closeNexusWindow: () => (isDesktop ? call<void>("nexus_window_close") : Promise.resolve()),
  /** modFolder: a folder inside the install folder, "" for the install folder itself, null when not known. */
  addGame: (domain: string, name?: string, path?: string, modFolder?: string | null, launch?: string, steamAppId?: number | null, saveFolder?: string | null) =>
    call<void>("games_add", { domain, name, path, modFolder, launch, steamAppId: steamAppId ?? null, saveFolder: saveFolder ?? null }),
  /** Every game on Nexus Mods (kept a week), and what Steam has installed. */
  gameCatalog: () => call<{ games: { domain: string; name: string; mods: number }[]; installed: { appId: number; name: string; path: string }[] }>("game_catalog"),
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
  changelog: (game: string, modId: number) => call<{ version: string; notes: string[] }[]>("mod_changelog", { game, modId }),
  profileUpdates: (id: string) => call<UpdateReport>("profile_updates", { id }),
  /** Your notes on mods, by "game:mod number". */
  notes: () => call<Record<string, string>>("mod_notes"),
  setNote: (game: string, modId: number, note: string) => call<void>("set_mod_note", { game, modId, note }),
  /** Save a report for getting help (no API key in it). */
  diagnostics: (path: string) => call<void>("diagnostics", { path }),
  logTail: (lines = 300) => call<string>("log_tail", { lines }),
  openLogFolder: () => call<void>("log_open"),
  deleteProfile: (id: string) => call<void>("profile_delete", { id }),
  checkProfile: (id: string) => call<CheckReport>("profile_check", { id }),
  appUpdate: () => call<{ release?: { version: string; url: string; notes: string; appImage?: string | null } | null; flatpak: boolean; version: string }>("app_update"),
  lootSort: (id: string) => call<LootProposal>("loot_sort", { id }),
  lootApply: (id: string, order: string[] | null) => call<void>("loot_apply", { id, order }),
  snapshots: (game: string) => call<Snapshot[]>("snapshot_list", { game }),
  takeSnapshot: (game: string, label: string) => call<Snapshot>("snapshot_take", { game, label }),
  restoreSnapshot: (game: string, id: string) => call<Snapshot>("snapshot_restore", { game, id }),
  deleteSnapshot: (game: string, id: string) => call<void>("snapshot_delete", { game, id }),
  aiStatus: () => call<{ settings?: { provider: string; model: string; baseUrl?: string } | null; hasKey: boolean }>("ai_status"),
  aiSet: (settings: { provider: string; model: string; baseUrl: string } | null, key: string | null) => call<{ settings?: { provider: string; model: string; baseUrl?: string } | null; hasKey: boolean }>("ai_set", { settings, key }),
  aiContext: (profile: string | null, game: string | null) => call<string>("ai_context", { profile, game }),
  aiAsk: (context: string, messages: { role: string; content: string }[], id: string) => call<string>("ai_ask", { context, messages, id }),
  openSteam: (what: "keyboard" | "tmodloader-workshop" | "games") => call<void>("open_steam", { what }),
  steamEntry: () => call<{ added: boolean; steamRunning: boolean; program?: string | null; appImage: boolean; dev: boolean; hasSteam: boolean }>("steam_entry"),
  steamAdd: (restart: boolean) => call<boolean>("steam_add", { restart }),
  device: () => call<{ deck: boolean; steamos: boolean; gameMode: boolean; flatpak: boolean }>("device"),
  customProtons: () => call<string[]>("custom_protons"),
  tmlState: () => call<TmlState | null>("tml_state"),
  tmlSet: (mods: string[]) => call<void>("tml_set", { mods }),
  tmlSavePack: (name: string, mods: string[]) => call<void>("tml_save_pack", { name, mods }),
  tsUpdates: (game: string) => call<{ mods: { fullName: string; name: string; have: string; newest: string; deprecated: boolean }[]; packs: { profileId: string; profileName: string; pack: string; have: string; newest: string }[] }>("ts_updates", { game }),
  tsUpdate: (game: string, fullNames: string[]) => call<number>("ts_update", { game, fullNames }),
  tsUpdatePack: (id: string) => call<Profile>("ts_update_pack", { id }),
  r2Profiles: (game: string) => call<{ name: string; path: string }[]>("r2_profiles", { game }),
  r2Import: (game: string, code: string | null, folder: string | null) => call<{ added: string[]; had: number; missing: string[]; profile?: Profile | null }>("r2_import", { game, code, folder }),
  tsGame: (game: string) => call<TsGame | null>("ts_game", { game }),
  tsSearch: (game: string, q: string, sort: string, offset: number) => call<{ results: TsFound[]; total: number }>("ts_search", { game, q, sort, offset }),
  tsAdd: (game: string, fullName: string, profile: string | null) => call<{ added: string[]; had: number; missing: string[]; profile?: Profile | null }>("ts_add", { game, fullName, profile }),
  mo2Look: (path: string) => call<Mo2Found>("mo2_look", { path }),
  mo2Import: (game: string, path: string, profile: string) => call<{ profile: Profile; added: number; skipped: string[]; bytesCopied: number }>("mo2_import", { game, path, profile }),
  modSettings: (id: string) => call<SettingFile[]>("profile_settings", { id }),
  readSetting: (id: string, path: string) => call<string>("profile_setting_read", { id, path }),
  writeSetting: (id: string, path: string, text: string) => call<void>("profile_setting_write", { id, path, text }),
  resetSetting: (id: string, path: string) => call<void>("profile_setting_reset", { id, path }),
  lastRun: (id: string) => call<Trouble | null>("profile_last_run", { id }),
  profileChecks: () => call<Record<string, CheckReport>>("profile_checks"),
  applyProfile: (id: string) => call<Applied>("profile_apply", { id }),
  clearAllGames: () => call<[string[], string[]]>("games_clear_all"),
  clearGame: (game: string) => call<void>("profile_clear", { game }),
  play: (id: string) => call<Applied>("profile_play", { id }),
  exportPack: (id: string, path: string) => call<void>("profile_export", { id, path }),
  shareCode: (id: string) => call<string>("profile_share_code", { id }),
  importCode: (code: string) => call<{ profile: Profile; queued: number; note?: string | null }>("profile_import_code", { code }),
  importPack: (path: string) => call<{ profile: Profile; queued: number; note?: string | null }>("profile_import", { path }),
  /** Put the profiles in this order. */
  orderProfiles: (ids: string[]) => call<Profile[]>("profiles_order", { ids }),
  layers: (id: string) => call<Layers>("profile_layers", { id }),
  /** Look up the collection at the profile's revision so your changes can be told apart (one request). */
  linkCollection: (id: string) => call<Layers>("profile_collection_link", { id }),
  /** What moving to the newest revision would do (one request); with apply, move to `revision`. */
  collectionUpdate: (id: string, apply: boolean, revision?: number | null) => call<CollectionUpdate>("profile_collection_update", { id, apply, revision: revision ?? null }),
  saveAddon: (id: string, name: string) => call<Profile>("profile_addon_save", { id, name }),
  setAddon: (id: string, addon: string, change: { enabled?: boolean; name?: string; remove?: boolean }) => call<Profile>("profile_addon_set", { id, addon, enabled: change.enabled ?? null, name: change.name ?? null, remove: !!change.remove }),
  exportAddon: (id: string, addon: string, path: string) => call<void>("profile_addon_export", { id, addon, path }),
  tags: () => call<Record<string, string[]>>("mod_tags"),
  setTags: (game: string, modId: number, tags: string[]) => call<Record<string, string[]>>("set_mod_tags", { game, modId, tags }),
  stopGame: (game: string, end: boolean) => call<number>("game_stop", { game, end }),
  backup: (path: string) => call<number>("app_backup", { path }),
  restoreBackup: (path: string) => call<{ made: number; files: string[] }>("app_restore", { path }),
  moveGameLibrary: (game: string, to: string | null) => call<number>("library_move_game", { game, to }),
  gamesGone: () => call<GoneGame[]>("games_gone"),
  answerGone: (domain: string, del: boolean, profilesToo: boolean) => call<number>("games_gone_answer", { domain, delete: del, profilesToo }),
  history: (game: string) => call<RestorePoint[]>("profile_history", { game }),
  historyNote: (game: string, at: number, note: string) => call<void>("profile_history_note", { game, at, note }),
  restore: (game: string, at: number) => call<Profile>("profile_restore", { game, at }),
  installer: (game: string, modId: number, fileId: number) => call<Installer>("fomod_read", { game, modId, fileId }),
  install: (game: string, modId: number, fileId: number, choices: FomodChoice[]) => call<StoredMod>("fomod_install", { game, modId, fileId, choices }),
  resetInstall: (game: string, modId: number, fileId: number) => call<void>("fomod_reset", { game, modId, fileId }),
  installerImage: (game: string, modId: number, fileId: number, path: string) => call<string | null>("fomod_image", { game, modId, fileId, path }),
  /** Activity problems the person said to ignore ("profile id|problem"). */
  ignored: () => call<string[]>("activity_ignored"),
  ignore: (key: string, on: boolean) => call<string[]>("activity_ignore", { key, on }),
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
export async function pickFile(title: string, kind: "pack" | "mod" | "exe" = "pack"): Promise<string | null> {
  if (!isDesktop) return null;
  const { open } = await import("@tauri-apps/plugin-dialog");
  const filters = kind === "pack" ? [{ name: "Mod pack", extensions: ["json"] }] : kind === "exe" ? [{ name: "Windows programs", extensions: ["exe", "EXE"] }] : [{ name: "Mod archives", extensions: ["zip", "7z", "rar"] }];
  const r = await open({ title, multiple: false, filters });
  return typeof r === "string" ? r : null;
}

export const modUrl = (game: string, modId: number) => `https://www.nexusmods.com/${game}/mods/${modId}`;
/** A file's mod-manager download page: its Slow download (or Premium's Fast download) button
 * hands the file to Zom's Modzilla through an nxm link. Without nmm=1 it's the manual download,
 * which only saves the file in the browser. */
export const fileUrl = (game: string, modId: number, fileId: number) => `${modUrl(game, modId)}?tab=files&file_id=${fileId}&nmm=1`;

/** Take the game's mods out (its own files come back, and the usual saves), then start it. */
export async function playWithoutMods(game: string) {
  await api.clearGame(game);
  await api.launch(game);
}
