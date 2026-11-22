import { useCallback, useEffect, useRef, useState } from "react";
import { Compass, Download, FolderOpen, Gamepad2, Layers, Settings as Cog } from "lucide-react";
import { api, fileUrl, isDesktop, on, type Game, type Job, type Overview } from "./api";
import { GamePicker } from "./GamePicker";
import { Setup } from "./Setup";
import { pendingUpdates, profileProgress, takeUpdate } from "./progress";
import { ContextMenu, DiskBar, Toast, Wordmark, type MenuEntry, type MenuState, type ToastMsg } from "./ui";
import { bytes, eta, speed } from "./format";
import { Downloads } from "./pages/Downloads";
import { Library } from "./pages/Library";
import { Games } from "./pages/Games";
import { SettingsPage } from "./pages/Settings";
import { AddDialog } from "./pages/AddDialog";
import { Browse } from "./pages/Browse";
import { Profiles } from "./pages/Profiles";

export type Page = "downloads" | "library" | "browse" | "profiles" | "games" | "settings";

export interface Ctx {
  overview: Overview;
  setOverview: (o: Overview) => void;
  jobs: Job[];
  /** The list has been read once (so an empty list really is empty). */
  loaded: boolean;
  toast: (text: string, action?: ToastMsg["action"]) => void;
  /** Open a page, optionally picking a game too. */
  go: (p: Page, game?: string) => void;
  /** The game picked in the sidebar; null shows every game. */
  game: string | null;
  setGame: (g: string | null) => void;
  /** The games on this computer (Steam's and the ones added by hand). */
  games: Game[];
  reloadGames: () => void;
  openAdd: (address?: string) => void;
  libraryVersion: number;
  /** Show a right-click menu for something on the page. */
  menu: (e: React.MouseEvent, items: MenuEntry[]) => void;
  /** Walking through files that wait for a Mod manager download click, one page at a time. */
  guiding: boolean;
  guide: (on: boolean) => void;
}

/** A free account's file that hasn't had its website click yet. */
export const awaitingClick = (j: Job) => j.status === "needsLink" && !j.hasLink;

const PAGES: { id: Page; label: string; icon: typeof Download }[] = [
  { id: "downloads", label: "Downloads", icon: Download },
  { id: "library", label: "Library", icon: FolderOpen },
  { id: "browse", label: "Browse", icon: Compass },
  { id: "profiles", label: "Profiles", icon: Layers },
  { id: "games", label: "Games", icon: Gamepad2 },
  { id: "settings", label: "Settings", icon: Cog },
];

/** Put a finished download into its game's "new downloads" profile, with what it needs. */
async function intoProfile(j: Job) {
  const ps = await api.profiles().catch(() => []);
  // Profiles waiting for this file as an update: it takes the old file's place.
  const waiting = pendingUpdates();
  for (const p of ps) {
    if (!(waiting[p.id] ?? []).some((u) => u.modId === j.modId && u.fileId === j.fileId)) continue;
    await api.saveProfile({ ...p, mods: p.mods.map((m) => (m.modId === j.modId ? { ...m, fileId: j.fileId } : m)) }).catch(() => {});
    takeUpdate(p.id, j.modId, j.fileId);
  }
  const p = ps.find((x) => x.game === j.game && x.newDownloads);
  if (!p || p.mods.some((m) => m.modId === j.modId && m.fileId === j.fileId)) return;
  // A newer file of a mod already there replaces the old one, in its place.
  const at = p.mods.findIndex((m) => m.modId === j.modId);
  const mods = [...p.mods];
  if (at >= 0) mods[at] = { ...mods[at], fileId: j.fileId };
  else mods.push({ modId: j.modId, fileId: j.fileId, enabled: true });
  await api.saveProfile({ ...p, mods }).catch(() => {});
  await api.resolveProfile(p.id).catch(() => {});
}

function stored<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    return v == null ? fallback : (JSON.parse(v) as T);
  } catch {
    return fallback;
  }
}
function store(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* no storage */
  }
}

function lastPage(): Page {
  try {
    const p = localStorage.getItem("zoms-nexus:page");
    if (p && PAGES.some((x) => x.id === p)) return p as Page;
  } catch {
    /* no storage */
  }
  return "downloads";
}

export default function App({ first }: { first: Overview }) {
  const [overview, setOverview] = useState(first);
  const [jobs, setJobs] = useState<Job[]>([]);
  const [loaded, setLoaded] = useState(false);
  const [page, setPage] = useState<Page>(first.hasKey ? lastPage() : "settings");
  const [toastMsg, setToast] = useState<ToastMsg | null>(null);
  const [adding, setAdding] = useState<string | false>(false);
  const [game, setGameState] = useState<string | null>(() => stored<string | null>("zoms-nexus:game", null));
  const [recent, setRecent] = useState<string[]>(() => stored<string[]>("zoms-nexus:recent-games", []));
  const [games, setGames] = useState<Game[]>([]);
  const [libraryVersion, setLibraryVersion] = useState(0);
  const [guiding, setGuiding] = useState(false);
  const finished = useRef(new Set<string>());
  const [menuState, setMenu] = useState<MenuState | null>(null);
  const closeMenu = useCallback(() => setMenu(null), []);
  const menu = useCallback((e: React.MouseEvent, items: MenuEntry[]) => {
    e.preventDefault();
    e.stopPropagation();
    if (items.length) setMenu({ x: e.clientX, y: e.clientY, items });
  }, []);
  // Right-click does nothing, except where the page offers its own menu, in text boxes and on
  // selected text (where copy and paste are useful).
  useEffect(() => {
    const ctx = (e: MouseEvent) => {
      const t = e.target as HTMLElement;
      if (t.closest("input, textarea, [contenteditable]") || (window.getSelection()?.toString() ?? "").trim()) return;
      e.preventDefault();
    };
    window.addEventListener("contextmenu", ctx);
    return () => window.removeEventListener("contextmenu", ctx);
  }, []);
  const guidingRef = useRef(false);
  // Open the next waiting file's page on Nexus Mods (the person clicks Mod manager download
  // there). Stops when none are left.
  const openNext = useCallback(async (after?: string) => {
    const all = await api.queue();
    const next = all.find((j) => awaitingClick(j) && j.id !== after);
    if (!next) {
      guidingRef.current = false;
      setGuiding(false);
      void api.closeNexusWindow();
      return false;
    }
    // One window that moves from file to file (in the app), instead of a browser tab each.
    await api.nexusWindow(fileUrl(next.game, next.modId, next.fileId)).catch(() => {});
    return true;
  }, []);
  const guide = useCallback(
    (start: boolean) => {
      guidingRef.current = start;
      setGuiding(start);
      if (start) void openNext();
    },
    [openNext],
  );

  const toast = useCallback((text: string, action?: ToastMsg["action"]) => setToast({ id: Date.now(), text, action }), []);
  const setGame = useCallback((g: string | null) => {
    setGameState(g);
    store("zoms-nexus:game", g);
    if (g)
      setRecent((r) => {
        const next = [g, ...r.filter((x) => x !== g)].slice(0, 20);
        store("zoms-nexus:recent-games", next);
        return next;
      });
  }, []);
  const reloadGames = useCallback(() => void api.games().then(setGames, () => {}), []);
  // One game is always picked: the last one, or the most recent one still here, or the first.
  useEffect(() => {
    if (!games.length || games.some((g) => g.domain === game)) return;
    const next = recent.find((d) => games.some((g) => g.domain === d)) ?? [...games].sort((a, b) => a.name.localeCompare(b.name))[0].domain;
    setGame(next);
  }, [games]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(reloadGames, [reloadGames]);
  const go = useCallback((p: Page, g?: string) => {
    setPage(p);
    if (g !== undefined) setGame(g);
    try {
      localStorage.setItem("zoms-nexus:page", p);
    } catch {
      /* no storage */
    }
  }, [setGame]);

  useEffect(() => {
    void api.queue().then((j) => {
      setJobs(j);
      setLoaded(true);
    });
    const offs = [
      on<Job>("queue:job", (j) => {
        // A file just finished: into the profile that takes new downloads for its game (once).
        if (j.status === "done" && !finished.current.has(j.id)) {
          finished.current.add(j.id);
          void intoProfile(j);
        }
        setJobs((all) => (all.some((x) => x.id === j.id) ? all.map((x) => (x.id === j.id ? j : x)) : all));
      }),
      on<Job[]>("queue:all", setJobs),
      on<null>("library:changed", () => setLibraryVersion((v) => v + 1)),
      on<Job>("nxm:added", (j) => {
        void api.queue().then(setJobs);
        if (guidingRef.current)
          void openNext(j.id).then((more) => toast(more ? `Got the link for “${j.name}”. Opened the next file.` : `Got the link for “${j.name}”. That was the last one.`));
        else toast(`Added “${j.name}” to downloads`);
      }),
      on<string>("nxm:error", (e) => toast(e)),
      // Every answer from Nexus Mods says how many requests are left; show the latest.
      on<Overview["limits"]>("limits:changed", (limits) => setOverview((o) => ({ ...o, limits }))),
      on<null>("queue:idle", () => toast("All downloads finished")),
      on<null>("nexus:manual", () => toast("That was Manual download, which doesn't reach Zom's Nexus. Click Slow download instead.")),
      on<null>("nexus:closed", () => {
        if (guidingRef.current) {
          guidingRef.current = false;
          setGuiding(false);
        }
      }),
      on<null>("account:changed", () => void api.overview().then(setOverview)),
      on<string>("collection:added", (name) => {
        toast(`Added the collection “${name}” to downloads, with a profile`);
        void api.queue().then(setJobs);
      }),
    ];
    return () => offs.forEach((p) => void p.then((off) => off()));
  }, [toast, openNext]);

  // Say when a profile (one made from a collection or a mod pack, say) has all its mods: the
  // conflict check has run by then, so it's ready to play.
  const complete = useRef<Set<string> | null>(null);
  useEffect(() => {
    // A download landed: put it into profiles of its game that were waiting for it as a
    // requirement (requirement answers are kept for a day, so this rarely asks Nexus Mods).
    if (libraryVersion > 0 && game)
      void api.profiles().then((ps) => ps.filter((p) => p.game === game && !p.manualRequirements).forEach((p) => void api.resolveProfile(p.id).catch(() => {})));
    void Promise.all([api.profiles(), api.library()]).then(([profiles, library]) => {
      const now = new Set(profiles.filter((p) => p.mods.length > 0 && profileProgress(p, library).have === profileProgress(p, library).total).map((p) => p.id));
      const before = complete.current;
      complete.current = now;
      if (!before) return;
      const done = profiles.filter((p) => now.has(p.id) && !before.has(p.id) && profileProgress(p, library).total > 1);
      if (done.length) toast(`“${done[0].name}” has all ${profileProgress(done[0], library).total} mods. It's ready to check and play.`, { label: "Open", run: () => go("profiles", done[0].game) });
    }, () => {});
  }, [libraryVersion]); // eslint-disable-line react-hooks/exhaustive-deps

  // Quick start: each game gets a "My mods" profile the first time it's picked, and finished
  // downloads go into it (or whichever profile is set to take new downloads).
  useEffect(() => {
    if (!game || !games.some((g) => g.domain === game)) return;
    const flag = `zoms-nexus:made-default:${game}`;
    if (stored(flag, false)) return;
    void api.profiles().then(async (ps) => {
      store(flag, true);
      if (ps.some((p) => p.game === game)) return;
      await api.saveProfile({ id: "", name: "My mods", game, mods: [], newDownloads: true }).catch(() => {});
    });
  }, [game, games]);

  // N adds a mod, unless you're typing.
  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (t.closest("input, textarea, select, [contenteditable]") || e.ctrlKey || e.metaKey || e.altKey) return;
      if (e.key === "n" || e.key === "N") {
        e.preventDefault();
        setAdding("");
      }
    };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, []);

  const active = jobs.filter((j) => !["done", "paused"].includes(j.status)).length;
  const ctx: Ctx = { overview, setOverview, jobs, loaded, toast, go, game, setGame, games, reloadGames, openAdd: (a?: string) => setAdding(a ?? ""), libraryVersion, guiding, guide, menu };
  const user = overview.user;
  const left = overview.limits;

  return (
    <div className="app">
      <nav className="sidebar" aria-label="Main">
        <Wordmark />
        <GamePicker games={games} game={game} recent={recent} onPick={setGame} onManage={() => go("games")} />
        {PAGES.map(({ id, label, icon: Icon }) => (
          <button key={id} className={`nav-item${page === id ? " active" : ""}`} onClick={() => go(id)} title={label}>
            <Icon size={16} />
            <span className="label">{label}</span>
            {id === "downloads" && active > 0 && <span className="pill accent label">{active}</span>}
          </button>
        ))}
        <div className="nav-gap" />
        <div className="side-foot">
          <DownloadStatus jobs={jobs} onOpen={() => go("downloads")} />
          {user ? (
            <>
              <span>
                Signed in as <b>{user.name}</b> {user.isPremium ? <span className="pill accent">Premium</span> : <span className="pill">Free</span>}
              </span>
              {left.hourlyRemaining != null && (
                <span className="num" title={`Nexus Mods API requests left${left.dailyRemaining != null ? `; ${left.dailyRemaining.toLocaleString()} today` : ""}. Updated with every answer from Nexus Mods.`}>
                  {left.hourlyRemaining.toLocaleString()} requests left this hour
                </span>
              )}
              <DiskBar free={overview.freeSpace} total={overview.totalSpace} label="mods" />
            </>
          ) : overview.hasKey ? (
            <span>Checking your account…</span>
          ) : (
            <span>Add your API key in Settings to start.</span>
          )}
        </div>
      </nav>
      <main className="main">
        {!isDesktop && (
          <div className="banner demo-banner">
            <b>Live demo.</b> Try anything. Changes are only kept in this browser tab, and downloads are pretend.{" "}
            {overview.user?.isPremium ? <a href="?free">Try it as a free account</a> : <a href="?">Try it as Premium</a>}
            {" · "}
            <a
              href="?"
              onClick={(e) => {
                e.preventDefault();
                // Everything back to the start: the made-up data reloads, and what this browser
                // remembers (picked game, recent games, page, theme) is forgotten.
                try {
                  Object.keys(localStorage)
                    .filter((k) => k.startsWith("zoms-nexus:"))
                    .forEach((k) => localStorage.removeItem(k));
                } catch {
                  /* no storage */
                }
                location.href = location.pathname + location.search;
              }}
            >
              Reset demo
            </a>
          </div>
        )}
        {page !== "settings" && <Setup {...ctx} />}
        {page === "downloads" && <Downloads {...ctx} />}
        {page === "library" && <Library {...ctx} />}
        {page === "browse" && <Browse {...ctx} />}
        {page === "profiles" && <Profiles {...ctx} />}
        {page === "games" && <Games {...ctx} />}
        {page === "settings" && <SettingsPage {...ctx} />}
      </main>
      {adding !== false && <AddDialog {...ctx} initial={adding} onClose={() => setAdding(false)} />}
      <ContextMenu menu={menuState} onClose={closeMenu} />
      <Toast toast={toastMsg} onDone={() => setToast(null)} />
    </div>
  );
}

/** The corner summary of the download list: what's going, how fast, and what's left. */
function DownloadStatus({ jobs, onOpen }: { jobs: Job[]; onOpen: () => void }) {
  const active = jobs.filter((j) => ["starting", "downloading", "storing"].includes(j.status));
  const queued = jobs.filter((j) => ["queued", "waiting"].includes(j.status)).length;
  const clicks = jobs.filter(awaitingClick).length;
  const paused = jobs.filter((j) => j.status === "paused").length;
  const done = jobs.filter((j) => j.status === "done").length;
  if (!active.length && !queued && !clicks && !paused) return null;
  const rate = active.reduce((n, j) => n + j.speed, 0);
  const total = jobs.filter((j) => j.status !== "failed").length;
  const now = active[0];
  // Overall, by size (files whose size isn't known yet count once they are).
  const counted = jobs.filter((j) => j.status !== "failed");
  const allBytes = counted.reduce((n, j) => n + j.size, 0);
  const gotBytes = counted.reduce((n, j) => n + (j.status === "done" ? j.size : Math.min(j.received, j.size)), 0);
  const parts = [queued ? `${queued} waiting` : "", clicks ? `${clicks} need your click` : "", paused ? `${paused} paused` : ""].filter(Boolean);
  return (
    <button className="dl-status" onClick={onOpen} title="Open Downloads">
      <span className="row between">
        <b>{active.length ? (active.length > 1 ? `Downloading ${active.length}` : now.status === "storing" ? "Storing" : "Downloading") : paused && !queued ? "Paused" : "Waiting"}</b>
        {rate > 0 && <span className="num muted">{speed(rate)}</span>}
      </span>
      {now && (
        <>
          <span className="ellipsis" title={now.modName}>
            {now.modName}
          </span>
          <span className="disk-bar" aria-hidden="true">
            <span style={{ width: `${now.size ? Math.min(100, (now.received / now.size) * 100) : 0}%` }} />
          </span>
          <span className="num muted">
            {now.status === "storing" ? "Moving into the library" : now.size ? `${bytes(now.received)} of ${bytes(now.size)}${now.speed ? ` · ${eta(now.size - now.received, now.speed)} left` : ""}` : "Starting"}
          </span>
        </>
      )}
      <span className="row between dl-total-head">
        <span className="muted">All files</span>
        {allBytes > 0 && <span className="num muted">{Math.floor((gotBytes / allBytes) * 100)}%</span>}
      </span>
      <span className="disk-bar total" aria-hidden="true">
        <span style={{ width: `${allBytes ? (gotBytes / allBytes) * 100 : total ? (done / total) * 100 : 0}%` }} />
      </span>
      <span className="num muted">
        {done} of {total} done{allBytes ? ` · ${bytes(gotBytes)} of ${bytes(allBytes)}` : ""}
        {parts.length ? ` · ${parts.join(" · ")}` : ""}
      </span>
    </button>
  );
}
