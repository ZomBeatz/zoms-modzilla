import { useEffect, useState } from "react";
import { AlertCircle, Check, CheckCircle2, Copy, FileDown, ExternalLink, Eye, EyeOff, KeyRound, Link2, MonitorCheck } from "lucide-react";
import { api, isDesktop, openUrl, pickFolder, pickSave, type LinkStatus, type Settings, type SystemReport } from "../api";
import type { Ctx } from "../App";
import { DiskBar, Stepper } from "../ui";
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
  const toggle = (label: string, hint: string, key: "extract" | "keepArchives" | "autoRetry" | "startWithComputer" | "verifyDownloads", disabled = false) => (
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
          {toggle("Unpack downloads", "Unpack zip, 7z and rar files into a folder next to the archive when they finish, so they're ready to use. Anything still packed is unpacked when you play.", "extract")}
          {toggle("Keep archives after unpacking", "Keeps the original file too, so you can unpack it again later. Uses more space.", "keepArchives", !s.extract)}
        </div>

        <Links {...ctx} />

        <ThisComputer {...ctx} />

        {isDesktop && (
          <div className="panel">
            <h3>When the app closes</h3>
            <p className="muted small" style={{ marginTop: 0 }}>
              Closing the window while files download keeps them going: Zom's Nexus stays in the system tray, and closes itself with a notification when they're done. Downloads that stop partway (a crash, a restart) carry on from where they were the next time it opens.
            </p>
            {toggle("Start with my computer", "Opens in the background when you sign in, to finish any unfinished downloads. If there's nothing to download, it closes again straight away.", "startWithComputer")}
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
