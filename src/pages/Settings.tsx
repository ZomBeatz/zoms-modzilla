import { useEffect, useState } from "react";
import { AlertCircle, Check, CheckCircle2, Copy, FileDown, FileUp, FolderOpen, ScrollText, ExternalLink, Eye, EyeOff, KeyRound, Link2, MonitorCheck } from "lucide-react";
import { getDevice, readUi, saveUi } from "../deck";
import { AddToSteam } from "./AddToSteam";
import { AiSettings } from "./AiSettings";
import { api, isDesktop, openUrl, pickFile, pickFolder, pickSave, type LinkStatus, type Settings, type SystemReport } from "../api";
import type { Ctx } from "../App";
import { bytes } from "../format";
import { Confirm, DiskBar, Stepper } from "../ui";
import { LIMITS } from "./Downloads";

type Theme = "system" | "light" | "dark";

function readTheme(): Theme {
  try {
    const t = localStorage.getItem("zoms-nexus:theme");
    return t === "light" || t === "dark" ? t : "system";
  } catch {
    return "system";
  }
}

function setTheme(t: Theme) {
  try {
    if (t === "system") localStorage.removeItem("zoms-nexus:theme");
    else localStorage.setItem("zoms-nexus:theme", t);
  } catch {
    /* no storage */
  }
  if (t === "system") delete document.documentElement.dataset.theme;
  else document.documentElement.dataset.theme = t;
}

function ApiKey({ overview, setOverview, toast }: Ctx) {
  const [key, setKey] = useState("");
  const [show, setShow] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const user = overview.user;

  // A shown key hides again after 30 seconds.
  useEffect(() => {
    if (!show) return;
    const t = window.setTimeout(() => setShow(false), 30000);
    return () => window.clearTimeout(t);
  }, [show]);

  const save = async () => {
    setBusy(true);
    setError("");
    try {
      const o = await api.setApiKey(key);
      setOverview(o);
      setKey("");
      toast(`Signed in as ${o.user?.name}`);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="panel">
      <h3>
        <KeyRound size={15} /> Nexus Mods account
      </h3>
      <p className="lead">Zom's Nexus uses your personal API key to ask Nexus Mods about mods and to download them. The key stays on this computer, in a file only you can read.</p>
      {overview.hasKey && (
        <div className="account">
          {user ? (
            <>
              <span>
                Signed in as <b>{user.name}</b>
              </span>
              {user.isPremium ? <span className="pill accent">Premium</span> : <span className="pill">Free account</span>}
              {overview.limits.dailyRemaining != null && (
                <span className="muted small num">
                  {overview.limits.dailyRemaining.toLocaleString()} requests left today, {overview.limits.hourlyRemaining?.toLocaleString()} this hour
                </span>
              )}
            </>
          ) : (
            <span className="muted">Checking the key…</span>
          )}
          <span className="grow" />
          <button className="btn sm" onClick={() => api.refreshAccount().then(setOverview, (e) => toast((e as Error).message))}>
            Check again
          </button>
          <button
            className="btn sm danger"
            onClick={async () => {
              setOverview(await api.forgetApiKey());
              toast("The API key was removed from this computer");
            }}
          >
            Remove key
          </button>
        </div>
      )}
      <label className="field">
        <span>{overview.hasKey ? "Replace the API key" : "API key"}</span>
        <div className="secret">
          <input
            type={show ? "text" : "password"}
            autoComplete="off"
            spellCheck={false}
            placeholder="Paste your personal API key"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && key.trim() && void save()}
          />
          <button className="icon-btn" aria-label={show ? "Hide the key" : "Show the key"} onClick={() => setShow(!show)}>
            {show ? <EyeOff size={16} /> : <Eye size={16} />}
          </button>
          <button className="btn primary" disabled={busy || !key.trim()} onClick={save}>
            {busy ? "Checking…" : "Save key"}
          </button>
        </div>
        <span className="hint">
          Find it on nexusmods.com under <b>Settings &gt; API keys</b> (the personal key at the bottom).{" "}
          <a
            href="#"
            onClick={(e) => {
              e.preventDefault();
              void openUrl("https://www.nexusmods.com/settings/api-keys");
            }}
          >
            Open the API keys page <ExternalLink size={11} />
          </a>
        </span>
        {error && <span className="err">{error}</span>}
      </label>
      {isDesktop && (
        <label className="check" style={{ marginTop: 12 }} title="Through secret-tool. The keyring may ask to be unlocked when Zom's Nexus starts.">
          <input
            type="checkbox"
            checked={!!overview.settings.keyInKeyring}
            onChange={async (e) => {
              try {
                setOverview(await api.saveSettings({ ...overview.settings, keyInKeyring: e.target.checked }));
                toast(e.target.checked ? "The key is in your desktop keyring now" : "The key is back in a file only you can read");
              } catch (err) {
                toast((err as Error).message);
              }
            }}
          />{" "}
          Keep the key in my desktop keyring (KDE Wallet, GNOME Keyring) instead of a file
        </label>
      )}
    </div>
  );
}

/** What this computer has, with the install command (for its distro) for anything missing. */
function ThisComputer({ toast }: Ctx) {
  const [r, setR] = useState<SystemReport | null>(null);
  const load = () => api.systemCheck().then(setR, () => setR(null));
  useEffect(() => {
    void load();
  }, []);
  if (!r) return null;
  const missing = r.items.filter((i) => !i.ok).length;
  return (
    <div className="panel">
      <h3>
        <MonitorCheck size={15} /> This computer
        <span className="grow" />
        <span className="muted small">{r.distro}</span>
      </h3>
      <p className="lead">{missing ? "Zom's Nexus works without these, but they make it better. Copy the command into a terminal, then check again." : "Everything Zom's Nexus can use is here."}</p>
      <div className="sys-list">
        {r.items.map((i) => (
          <div key={i.what} className="sys-item">
            {i.ok ? <CheckCircle2 size={16} color="var(--zb-ok)" /> : <AlertCircle size={16} color="var(--zb-warn)" />}
            <div className="grow" style={{ minWidth: 0 }}>
              <b>{i.what}</b>
              <div className="muted small">{i.detail}</div>
              {i.fix && (
                <div className="row gap-sm" style={{ marginTop: 6 }}>
                  <code className="mono small sys-cmd">{i.fix}</code>
                  <button
                    className="btn sm"
                    onClick={() =>
                      navigator.clipboard.writeText(i.fix!).then(
                        () => toast("Copied. Paste it into a terminal."),
                        () => toast(i.fix!),
                      )
                    }
                  >
                    <Copy size={13} /> Copy
                  </button>
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
      <div className="row gap-sm wrap" style={{ marginTop: 12 }}>
        {missing > 0 && (
          <button className="btn sm" onClick={() => void load()}>
            Check again
          </button>
        )}
        {isDesktop && (
          <button
            className="btn sm"
            title="Versions, this computer, settings, games, profiles and downloads. Never your API key."
            onClick={async () => {
              const path = await pickSave("Save a report for getting help", "zoms-nexus-report.json");
              if (path)
                await api.diagnostics(path).then(
                  () => toast("Saved. Share it when asking for help; it doesn't contain your API key."),
                  (e) => toast((e as Error).message),
                );
            }}
          >
            <FileDown size={13} /> Save a report for getting help…
          </button>
        )}
      </div>
    </div>
  );
}

/** The app's log, for troubleshooting: the last lines, with a way to copy them. */
function Logs({ toast }: Ctx) {
  const [text, setText] = useState<string | null>(null);
  const load = () => api.logTail(300).then(setText, (e) => setText((e as Error).message));
  return (
    <div className="panel">
      <h3>
        <ScrollText size={15} /> Logs
      </h3>
      <p className="lead">What Zom's Nexus did and what went wrong, for troubleshooting. Keys and private links are blanked out.</p>
      <div className="row gap-sm wrap">
        <button className="btn sm" onClick={() => void load()}>
          {text === null ? "Show the log" : "Refresh"}
        </button>
        {text !== null && (
          <button className="btn sm" onClick={() => navigator.clipboard.writeText(text).then(() => toast("Copied the log"), () => toast("Couldn't copy"))}>
            <Copy size={13} /> Copy
          </button>
        )}
        <button className="btn sm" onClick={() => void api.openLogFolder().catch((e) => toast((e as Error).message))}>
          <FolderOpen size={13} /> Open folder
        </button>
      </div>
      {text !== null && <pre className="log-view">{text || "Nothing logged yet."}</pre>}
    </div>
  );
}

function Links({ toast }: Ctx) {
  const [status, setStatus] = useState<LinkStatus | null>(null);
  useEffect(() => {
    api.linkStatus().then(setStatus, () => setStatus(null));
  }, []);
  return (
    <div className="panel">
      <h3>
        <Link2 size={15} /> Nexus Mods links
      </h3>
      <p className="lead">The <b>Mod manager download</b> button on nexusmods.com opens an nxm:// link. When Zom's Nexus opens those links, that button adds the file to your downloads, even when the app is closed.</p>
      <div className="row gap wrap">
        {status?.ours ? (
          <span className="pill ok">
            <Check size={12} /> Zom's Nexus opens Nexus Mods links
          </span>
        ) : status?.other ? (
          <span className="pill warn">Another program opens them now ({status.other})</span>
        ) : (
          <span className="pill">Nothing opens them yet</span>
        )}
        {!status?.ours && (
          <button
            className="btn primary"
            onClick={() =>
              api.registerLinks().then(
                (s) => {
                  setStatus(s);
                  toast("Zom's Nexus now opens Nexus Mods links");
                },
                (e) => toast((e as Error).message),
              )
            }
          >
            Open them with Zom's Nexus
          </button>
        )}
      </div>
      <p className="form-note">If your browser asks which program to use for the link the first time, pick Zom's Nexus and tick “Always”.</p>
    </div>
  );
}

const ACTIVITY_KINDS: [string, string][] = [
  ["failed", "Downloads that didn't finish"],
  ["updates", "Updates found"],
  ["gone", "Mods gone from Nexus Mods"],
  ["worse", "Profiles with more to fix"],
  ["played", "Time played"],
  ["quickExit", "Games that closed right after starting"],
  ["saves", "Saves switched"],
];

function HourPick({ value, onChange, label }: { value: number; onChange: (h: number) => void; label: string }) {
  return (
    <select value={value} onChange={(e) => onChange(Number(e.target.value))} aria-label={label}>
      {Array.from({ length: 24 }, (_, h) => (
        <option key={h} value={h}>
          {String(h).padStart(2, "0")}:00
        </option>
      ))}
    </select>
  );
}

/** Where each game's mods are kept: in the library, or a folder of its own (moved there). */
function GameFolders(ctx: Ctx) {
  const { overview, setOverview, toast, games } = ctx;
  const [busy, setBusy] = useState("");
  const own = overview.settings.gameLibraries ?? {};
  const move = async (game: string, to: string | null) => {
    setBusy(game);
    try {
      const n = await api.moveGameLibrary(game, to);
      setOverview(await api.overview());
      toast(n ? `Moved ${bytes(n)} of mods. They're used from there now.` : "Done. There were no mods to move yet.");
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setBusy("");
    }
  };
  return (
    <div className="setting" style={{ display: "block" }}>
      <span className="what">
        <b>A folder of its own for a game</b>
        <span>Keep one game's mods somewhere else, like the drive the game is on. Moving them copies everything there first and only then deletes the old copy. Downloads wait while it moves.</span>
      </span>
      <div className="game-folders">
        {games.map((g) => (
          <div key={g.domain} className="row gap wrap game-folder">
            <b className="grow ellipsis">{g.name}</b>
            <span className="muted small mono ellipsis" style={{ maxWidth: 320 }}>
              {own[g.domain] ? `${own[g.domain]}/${g.domain}` : "In the library"}
            </span>
            {isDesktop && (
              <button
                className="btn sm"
                disabled={!!busy}
                onClick={async () => {
                  const p = await pickFolder(`Where to keep ${g.name}'s mods`, own[g.domain] ?? overview.settings.libraryDir);
                  if (p) await move(g.domain, p);
                }}
              >
                {busy === g.domain ? "Moving…" : "Move…"}
              </button>
            )}
            {own[g.domain] && (
              <button className="btn ghost sm" disabled={!!busy} onClick={() => move(g.domain, null)}>
                Back to the library
              </button>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}

/** Playing on a Steam Deck: Game Mode, the keyboard, downloads, SD cards. Open by itself on a Deck. */
function DeckHelp({ toast }: { toast: (t: string) => void }) {
  const [dev, setDev] = useState<{ deck: boolean; steamos: boolean; gameMode: boolean; flatpak: boolean } | null>(null);
  useEffect(() => {
    void getDevice().then(setDev);
  }, []);
  const deck = !!(dev?.deck || dev?.steamos);
  return (
    <details className="panel deck-help" open={deck || undefined}>
      <summary>
        <h3 style={{ display: "inline-flex", margin: 0 }}>Steam Deck</h3>
        <span className="muted small"> {dev?.gameMode ? "· running in Game Mode" : deck ? "· this is a Steam Deck" : "· how to use Zom's Nexus on one"}</span>
      </summary>
      {isDesktop && <AddToSteam toast={toast} />}
      <ol className="deck-steps">
        <li>
          <b>Put it in Game Mode.</b> Add to Steam (above) puts it in your library with artwork. Or by hand: in Desktop Mode, Steam › Games › <i>Add a Non-Steam Game to My Library…</i>. In Game Mode it's under Library › Non-Steam, and opens full screen.
        </li>
        <li>
          <b>The controls.</b> Steam Deck mode (above, under Appearance) turns on by itself: the D-pad moves, A selects, B goes back, Y plays, L1/R1 change page, the triggers scroll, ☰ finds anything. If the controller does nothing, pick the <i>Gamepad</i> layout for Zom's Nexus in Steam's controller settings.
        </li>
        <li>
          <b>Typing.</b> Choosing a text box opens Steam's keyboard; <kbd>STEAM</kbd> + <kbd>X</kbd> opens it any time.
          {isDesktop && (
            <button className="btn sm ghost" style={{ marginLeft: 8 }} onClick={() => void api.openSteam("keyboard").catch(() => {})}>
              Try it
            </button>
          )}
        </li>
        <li>
          <b>Downloads.</b> The website's Mod manager download button works in Desktop Mode (set up Nexus Mods links above). In Game Mode, paste a mod's address into Add mod, get collections by their address, or use Browse; Premium accounts download straight from the app.
        </li>
        <li>
          <b>SD cards.</b> Mods are linked into a game when they're on the same drive, and copied when they're not. For a game on the SD card, the Games page offers <i>Keep its mods on its drive</i>, so nothing is stored twice.
        </li>
        <li>
          <b>Installing and updating.</b> SteamOS keeps its system read-only, so use the AppImage (keep it in ~/Applications, and right-click › Properties › Permissions › Is executable), never the .deb or .rpm. Your library, profiles and settings live in your home folder, so they survive SteamOS updates.
        </li>
        <li>
          <b>When a modded game won't start.</b> Many mod loaders need GE-Proton: add it with ProtonUp-Qt (in Discover), then pick it in the game's Properties › Compatibility. Zom's Nexus says so when a game keeps closing right after it starts.
        </li>
      </ol>
    </details>
  );
}

/** Steam Deck mode and the interface's size. */
function InterfaceSettings() {
  const [ui, setUi] = useState(readUi());
  const [dev, setDev] = useState<{ deck: boolean; gameMode: boolean } | null>(null);
  useEffect(() => {
    void getDevice().then(setDev);
  }, []);
  const set = (u: typeof ui) => {
    setUi(u);
    saveUi(u);
  };
  return (
    <>
      <div className="setting">
        <span className="what">
          <b>Steam Deck mode</b>
          <span>
            Bigger buttons, and the controller (or the arrow keys) moves between them: A selects, B goes back, Y plays, LB/RB change page, the triggers scroll, ☰ finds anything. A text box opens Steam's keyboard. Automatic turns it on for a Steam Deck and Steam's Game Mode
            {dev?.deck ? " (this is a Steam Deck)" : ""}.
          </span>
        </span>
        <div className="seg">
          {(
            [
              ["auto", "Automatic"],
              ["deck", "On"],
              ["desktop", "Off"],
            ] as const
          ).map(([m, label]) => (
            <button key={m} className={ui.mode === m ? "active" : ""} onClick={() => set({ ...ui, mode: m })}>
              {label}
            </button>
          ))}
        </div>
      </div>
      <div className="setting">
        <span className="what">
          <b>Size</b>
          <span>How big everything is. Usual is 100% on a desktop and 120% in Steam Deck mode.</span>
        </span>
        <select value={ui.scale} onChange={(e) => set({ ...ui, scale: Number(e.target.value) })} aria-label="Interface size">
          <option value={0}>Usual</option>
          {[0.9, 1, 1.1, 1.2, 1.3, 1.5].map((x) => (
            <option key={x} value={x}>
              {Math.round(x * 100)}%
            </option>
          ))}
        </select>
      </div>
    </>
  );
}

/** Before uninstalling: every game back to how it came. */
function LeavePanel({ toast }: { toast: (t: string) => void }) {
  const [sure, setSure] = useState(false);
  const [busy, setBusy] = useState(false);
  return (
    <div className="panel">
      <h3>Before uninstalling</h3>
      <p className="lead">
        Takes the mods out of every game and puts each game's own files and your usual saves back, so the games are as they came. Your library, profiles and settings stay, ready for when you put mods in again. Running games are left alone.
      </p>
      {sure ? (
        <div className="row gap wrap">
          <button
            className="btn primary"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              try {
                const [done, left] = await api.clearAllGames();
                toast([done.length ? `Took the mods out of ${done.join(", ")}` : "No game had mods in it", left.length ? `Not done: ${left.join("; ")}` : ""].filter(Boolean).join(". "));
              } catch (e) {
                toast((e as Error).message);
              } finally {
                setBusy(false);
                setSure(false);
              }
            }}
          >
            {busy ? "Taking mods out…" : "Yes, take them out"}
          </button>
          <button className="btn" onClick={() => setSure(false)}>
            Cancel
          </button>
        </div>
      ) : (
        <button className="btn" onClick={() => setSure(true)}>
          Take mods out of every game…
        </button>
      )}
    </div>
  );
}

function BackupPanel(ctx: Ctx) {
  const { toast, setOverview } = ctx;
  const [confirm, setConfirm] = useState<string | null>(null);
  return (
    <div className="panel">
      <h3>Backup</h3>
      <p className="lead">
        Everything Zom's Nexus knows, in one file: profiles (with their rules, add-ons and notes), your notes and tags on mods, restore points, time played, settings and collections' installer choices. Not the mods themselves, the API key, or the download list. Keep one before reinstalling, or take it to another computer.
      </p>
      <div className="row gap wrap">
        <button
          className="btn"
          onClick={async () => {
            const d = new Date();
            const path = await pickSave("Save a backup", `Zom's Nexus backup ${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}.json`);
            if (path) await api.backup(path).then((n) => toast(`Saved a backup (${n} files)`), (e) => toast((e as Error).message));
          }}
        >
          <FileDown size={15} /> Save a backup…
        </button>
        <button
          className="btn"
          onClick={async () => {
            const path = await pickFile("Open a Zom's Nexus backup");
            if (path) setConfirm(path);
          }}
        >
          <FileUp size={15} /> Restore from a backup…
        </button>
      </div>
      {confirm && (
        <Confirm
          title="Restore this backup?"
          body="Your profiles, notes, tags, restore points and settings are replaced by the backup's. This computer's library folder, per-game folders and API key stay as they are. Mods already in games aren't touched; put a profile in again to use it. Save a backup of how things are now first if you might want them back."
          action="Restore"
          onConfirm={async () => {
            try {
              const r = await api.restoreBackup(confirm);
              setOverview(await api.overview());
              toast(`Restored the backup from ${new Date(r.made * 1000).toLocaleDateString()} (${r.files.length} files). Open Profiles to see them.`);
            } catch (e) {
              toast((e as Error).message);
            }
          }}
          onClose={() => setConfirm(null)}
        />
      )}
    </div>
  );
}

export function SettingsPage(ctx: Ctx) {
  const { overview, setOverview, toast } = ctx;
  const s = overview.settings;
  const [theme, setThemeState] = useState<Theme>(readTheme());
  const save = async (patch: Partial<Settings>) => {
    try {
      setOverview(await api.saveSettings({ ...s, ...patch }));
    } catch (e) {
      toast((e as Error).message);
    }
  };
  const toggle = (label: string, hint: string, key: "extract" | "keepArchives" | "autoRetry" | "startWithComputer" | "verifyDownloads" | "retryLater" | "startHidden", disabled = false) => (
    <label className="setting" style={{ cursor: disabled ? "default" : "pointer", opacity: disabled ? 0.5 : 1 }}>
      <input type="checkbox" checked={!!s[key]} disabled={disabled} onChange={(e) => save({ [key]: e.target.checked })} style={{ marginTop: 3 }} />
      <span className="what">
        <b>{label}</b>
        <span>{hint}</span>
      </span>
    </label>
  );

  return (
    <div className="page">
      <div className="page-head">
        <h1>Settings</h1>
      </div>
      <div className="settings">
        <ApiKey {...ctx} />

        <div className="panel">
          <h3>Downloads</h3>
          <div className="setting">
            <span className="what">
              <b>How files download</b>
              <span>Match my account downloads several at once with Premium and one at a time with a free account (Nexus Mods slows free downloads, so several would only share the speed). One at a time keeps the list in order; several at once finishes a long list sooner.</span>
            </span>
            <div className="seg">
              <button className={s.mode === "auto" ? "active" : ""} onClick={() => save({ mode: "auto" })}>
                Match my account
              </button>
              <button className={s.mode === "sequential" ? "active" : ""} onClick={() => save({ mode: "sequential" })}>
                One at a time
              </button>
              <button className={s.mode === "concurrent" ? "active" : ""} onClick={() => save({ mode: "concurrent" })}>
                Several at once
              </button>
            </div>
          </div>
          {(s.mode === "concurrent" || (s.mode === "auto" && overview.user?.isPremium)) && (
            <div className="setting">
              <span className="what">
                <b>Downloads at once</b>
                <span>{s.mode === "auto" ? "How many your Premium account downloads at the same time (1 to 8)." : "How many download at the same time (1 to 8)."}</span>
              </span>
              <Stepper label="downloads at once" value={s.maxConcurrent} min={1} max={8} onChange={(n) => save({ maxConcurrent: n })} />
            </div>
          )}
          <div className="setting">
            <span className="what">
              <b>Speed limit</b>
              <span>The most all downloads together may use, so games, streams and calls on the same connection stay smooth.</span>
            </span>
            <select value={s.speedLimit} onChange={(e) => save({ speedLimit: Number(e.target.value) })} aria-label="Speed limit">
              {LIMITS.map(([v, label]) => (
                <option key={v} value={v}>
                  {label}
                </option>
              ))}
            </select>
          </div>
          {toggle("Try again by itself", "When the connection drops, try again up to 3 times, carrying on from where it stopped.", "autoRetry")}
          {toggle("Keep trying later", "If those 3 tries don't work, try again in 5 minutes, 30 minutes and 2 hours before giving up. Links that ran out still wait for you.", "retryLater", !s.autoRetry)}
          <div className="setting">
            <span className="what">
              <b>Download hours</b>
              <span>Only start downloads between these hours (ones already going finish). The same hour twice means any time.</span>
            </span>
            <span className="row gap-sm">
              <HourPick value={s.hoursFrom ?? 0} onChange={(h) => save({ hoursFrom: h })} label="From" />
              <span className="muted">to</span>
              <HourPick value={s.hoursTo ?? 0} onChange={(h) => save({ hoursTo: h })} label="To" />
            </span>
          </div>
          <div className="setting">
            <span className="what">
              <b>While a game is running</b>
              <span>Downloads share your connection with the game. Hold waits to start new ones; Pause stops all of them until the game closes. Only games started from Zom's Nexus are noticed.</span>
            </span>
            <select value={s.whilePlaying ?? "carry"} onChange={(e) => save({ whilePlaying: e.target.value as Settings["whilePlaying"] })} aria-label="While a game is running">
              <option value="carry">Carry on</option>
              <option value="hold">Hold new downloads</option>
              <option value="pause">Pause downloads</option>
            </select>
          </div>
          <div className="setting">
            <span className="what">
              <b>Another speed limit for some hours</b>
              <span>For example a lower limit in the evening, when others at home use the connection.</span>
            </span>
            <span className="row gap-sm wrap">
              <select value={s.timedLimit ?? 0} onChange={(e) => save({ timedLimit: Number(e.target.value) })} aria-label="Timed speed limit">
                {LIMITS.map(([v, label]) => (
                  <option key={v} value={v}>
                    {v === 0 ? "Off" : label}
                  </option>
                ))}
              </select>
              {!!s.timedLimit && (
                <>
                  <HourPick value={s.timedFrom ?? 0} onChange={(h) => save({ timedFrom: h })} label="From" />
                  <span className="muted">to</span>
                  <HourPick value={s.timedTo ?? 0} onChange={(h) => save({ timedTo: h })} label="To" />
                </>
              )}
            </span>
          </div>
          {toggle("Check downloads", "After each download, make sure the file matches Nexus Mods' copy (catches damaged or changed files). One request per file.", "verifyDownloads")}
        </div>

        <div className="panel">
          <h3>Storage</h3>
          <div className="setting">
            <span className="what">
              <b>Library folder</b>
              <span className="mono">{s.libraryDir}</span>
              <span>Each mod gets its own folder here, under its game: “skyrimspecialedition/266 - Mod name”.</span>
              <DiskBar free={ctx.overview.freeSpace} total={ctx.overview.totalSpace} label="this drive" />
              <span>Zom's Nexus checks there's room before each download and unpack, and checks the game's drive before copying mods into it.</span>
            </span>
            {isDesktop && (
              <button
                className="btn"
                onClick={async () => {
                  const p = await pickFolder("Where to keep downloaded mods", s.libraryDir);
                  if (p) await save({ libraryDir: p });
                }}
              >
                Choose…
              </button>
            )}
          </div>
          {ctx.games.length > 0 && <GameFolders {...ctx} />}
          {toggle("Unpack downloads", "Unpack zip, 7z and rar files into a folder next to the archive when they finish, so they're ready to use. Anything still packed is unpacked when you play.", "extract")}
          {toggle("Keep archives after unpacking", "Keeps the original file too, so you can unpack it again later. Uses more space.", "keepArchives", !s.extract)}
        </div>

        <div className="panel">
          <h3>Activity</h3>
          <p className="lead">What the Activity page notes. Problems with profiles always show under Needs attention.</p>
          {ACTIVITY_KINDS.map(([k, label]) => (
            <label key={k} className="setting" style={{ cursor: "pointer" }}>
              <input type="checkbox" checked={!(s.activityOff ?? []).includes(k)} onChange={(e) => save({ activityOff: e.target.checked ? (s.activityOff ?? []).filter((x) => x !== k) : [...(s.activityOff ?? []), k] })} style={{ marginTop: 3 }} />
              <span className="what">
                <b>{label}</b>
              </span>
            </label>
          ))}
        </div>

        {ctx.games.length > 0 && (
          <div className="panel">
            <h3>Copy mods instead of linking</h3>
            <p className="lead">Mods go into games as hard links: no extra space, but a tool that changes files in the game folder changes your library's copy too. Tick a game to always copy its mods instead (uses space on its drive).</p>
            {ctx.games.map((g) => (
              <label key={g.domain} className="setting" style={{ cursor: "pointer" }}>
                <input type="checkbox" checked={(s.copyGames ?? []).includes(g.domain)} onChange={(e) => save({ copyGames: e.target.checked ? [...(s.copyGames ?? []), g.domain] : (s.copyGames ?? []).filter((x) => x !== g.domain) })} style={{ marginTop: 3 }} />
                <span className="what">
                  <b>{g.name}</b>
                </span>
              </label>
            ))}
          </div>
        )}

        <Links {...ctx} />

        <ThisComputer {...ctx} />

        {isDesktop && <Logs {...ctx} />}

        {isDesktop && <BackupPanel {...ctx} />}

        <AiSettings toast={ctx.toast} />

        <DeckHelp toast={ctx.toast} />

        {isDesktop && <LeavePanel toast={ctx.toast} />}

        <div className="panel">
          <h3>Getting started</h3>
          <p className="lead">The first-run checklist: connect your account, let the website's buttons open Zom's Nexus, find your games, get your first mod.</p>
          <button
            className="btn"
            onClick={() => {
              try {
                localStorage.setItem("zoms-nexus:setup-show", "1");
                localStorage.removeItem("zoms-nexus:setup-hidden");
              } catch {
                /* no storage */
              }
              ctx.go("downloads");
              location.reload();
            }}
          >
            Show Getting started again
          </button>
        </div>

        {isDesktop && (
          <div className="panel">
            <h3>When the app closes</h3>
            <p className="muted small" style={{ marginTop: 0 }}>
              Closing the window while files download keeps them going: Zom's Nexus stays in the system tray, and closes itself with a notification when they're done. Downloads that stop partway (a crash, a restart) carry on from where they were the next time it opens.
            </p>
            {toggle("Start with my computer", "Opens in the background when you sign in, to finish any unfinished downloads. If there's nothing to download, it closes again straight away.", "startWithComputer")}
            {toggle("Open in the tray", "When you start Zom's Nexus, it waits in the system tray instead of opening its window. Open it from the tray icon. Needs a tray.", "startHidden")}
            <div className="setting">
              <span className="what">
                <b>Unfinished downloads at start</b>
                <span>Carry on with them when Zom's Nexus opens, or keep them paused until you resume them.</span>
              </span>
              <select value={s.resumeOnStart ?? "yes"} onChange={(e) => save({ resumeOnStart: e.target.value as "yes" | "no" })} aria-label="Unfinished downloads at start">
                <option value="yes">Carry on</option>
                <option value="no">Keep paused</option>
              </select>
            </div>
            <p className="muted small">The window opens at the size it had last time. Where it opens is up to your desktop: on KDE Plasma with Wayland, System Settings › Window Management › Window Rules can keep it in one place.</p>
          </div>
        )}

        <div className="panel">
          <h3>Appearance</h3>
          <div className="setting">
            <span className="what">
              <b>Theme</b>
            </span>
            <div className="seg">
              {(
                [
                  ["system", "Match my computer"],
                  ["light", "Light"],
                  ["dark", "Dark"],
                ] as const
              ).map(([t, label]) => (
                <button
                  key={t}
                  className={theme === t ? "active" : ""}
                  onClick={() => {
                    setTheme(t);
                    setThemeState(t);
                  }}
                >
                  {label}
                </button>
              ))}
            </div>
          </div>
          <InterfaceSettings />
          <div className="setting">
            <span className="what">
              <b>Adult mods</b>
              <span>Mods Nexus Mods marks as adult. Blurring covers their pictures everywhere; hiding also leaves them out of Browse. Your library and profiles always list what you have, tagged “Adult”, and every list can filter them.</span>
            </span>
            <div className="seg">
              {(
                [
                  ["show", "Show"],
                  ["blur", "Blur pictures"],
                  ["hide", "Hide in Browse"],
                ] as const
              ).map(([v, label]) => (
                <button key={v} className={(s.adultContent ?? "blur") === v ? "active" : ""} onClick={() => save({ adultContent: v })}>
                  {label}
                </button>
              ))}
            </div>
          </div>
        </div>

        <div className="panel">
          <h3>About</h3>
          <p style={{ margin: 0 }}>
            Zom's Nexus {overview.version} by ZomBeatz Media. MIT license. Not made by or connected with Nexus Mods.
          </p>
          <div className="row gap" style={{ marginTop: 10 }}>
            <button className="btn" onClick={() => openUrl("https://docs.zombeatzmedia.com/nexus/")}>
              Read the guide
            </button>
            <button className="btn" onClick={() => openUrl("https://github.com/ZomBeatz/Zoms-Nexus/issues/new")}>
              Send feedback
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
