import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, ArrowUpCircle, Bell, CheckCircle2, CircleOff, Clock, Copy, Download, EyeOff, Save, ShieldAlert, ShieldCheck, Trash2, Upload, Wand2 } from "lucide-react";
import { api, on, type ActivityEvent, type CheckReport, type Profile, type StoredMod } from "../api";
import type { Ctx, Page } from "../App";
import { GameTag } from "../art";
import { plural, when } from "../format";
import { wantProfile } from "../nav";
import { autoFix } from "../progress";
import { Confirm } from "../ui";
import { OnlyGameToggle, useOnlyGame } from "../onlyGame";

const LOOK: Record<string, { icon: typeof Bell; tone: string; open: string }> = {
  failed: { icon: Download, tone: "danger", open: "Go to Downloads" },
  worse: { icon: ShieldAlert, tone: "danger", open: "Open the profile" },
  gone: { icon: CircleOff, tone: "warn", open: "Go to Library" },
  updates: { icon: ArrowUpCircle, tone: "accent", open: "Go to Library" },
  saves: { icon: Save, tone: "", open: "Go to Profiles" },
  played: { icon: Clock, tone: "ok", open: "Open the profile" },
};

/** One thing wrong with a profile right now, from its latest check. */
interface Problem {
  /** Stays the same while the problem does, so ignoring it sticks. */
  key: string;
  tone: "danger" | "warn";
  title: string;
  detail?: string;
  /** The quick fix this problem has, if any. */
  fix?: "auto" | "apply" | "copy";
  copy?: string;
}

const NEED_WORDS: Record<string, string> = { notDownloaded: "isn't downloaded", notInProfile: "isn't in the profile", switchedOff: "is switched off", unavailable: "is no longer on Nexus Mods" };

/** What a check says needs doing, worst first. Warnings that don't stop the game are left out. */
export function problemsOf(r: CheckReport): Problem[] {
  const out: Problem[] = [];
  for (const n of r.needs ?? []) {
    if (!NEED_WORDS[n.kind]) continue;
    out.push({
      key: `need:${n.kind}:${n.modId || n.name}`,
      tone: n.kind === "unavailable" ? "warn" : "danger",
      title: `${n.name} ${NEED_WORDS[n.kind]}`,
      detail: `Needed by ${n.neededBy.slice(0, 3).join(", ")}${n.neededBy.length > 3 ? ` and ${n.neededBy.length - 3} more` : ""}`,
      fix: n.kind === "unavailable" ? undefined : "auto",
    });
  }
  for (const p of r.plugins) out.push({ key: `plugin:${p.plugin}:${p.master}`, tone: "danger", title: p.kind === "missing" ? `${p.plugin} needs ${p.master}, which isn't there` : `${p.plugin} loads before ${p.master}`, detail: `From ${p.modName}` });
  if (r.launchFix) out.push({ key: "launch", tone: "danger", title: `The game's launch options don't let ${r.launchFix.missing.join(", ")} load`, detail: "Paste the suggested ones into Steam › the game › Properties › Launch options.", fix: "copy", copy: r.launchFix.suggested });
  const log = r.gameLog && !r.gameLog.beforeChange ? r.gameLog : null;
  if (log?.crash) out.push({ key: `crash:${log.crash.at}`, tone: "danger", title: `The game crashed ${log.crash.minutes ? `after ${plural(log.crash.minutes, "minute")}` : "at the start"}`, detail: log.crash.outOfMemory ? "It ran out of memory." : "Open the profile for what was going on and what to try." });
  for (const i of log?.issues ?? []) out.push({ key: `log:${i.kind}:${i.modId}:${i.otherId ?? ""}`, tone: "danger", title: `${i.modName}: ${i.detail}`, detail: "From the game's own log" });
  if (r.outOfDate) out.push({ key: "outOfDate", tone: "warn", title: "The mods in the game are older than the profile", detail: "Something changed since they went in.", fix: "apply" });
  return out;
}

/** Things worth knowing about: what needs fixing in every profile now (live), then what happened. */
export function Activity({ go, toast, libraryVersion, gameStates, game, games }: Ctx) {
  const [events, setEvents] = useState<ActivityEvent[] | null>(null);
  const [clearing, setClearing] = useState(false);
  const [show, setShow] = useState<"all" | "problems">("all");
  const [allProfiles, setProfiles] = useState<Profile[]>([]);
  // Every game by default; the toggle narrows it to the game picked at the top.
  const [onlyGame, setOnlyGame] = useOnlyGame("activity");
  const narrow = !!game && onlyGame;
  const profiles = useMemo(() => (narrow ? allProfiles.filter((p) => p.game === game) : allProfiles), [allProfiles, narrow, game]);
  const [checks, setChecks] = useState<Record<string, CheckReport>>({});
  const [library, setLibrary] = useState<StoredMod[]>([]);
  const [ignored, setIgnored] = useState<string[]>([]);
  const [showIgnored, setShowIgnored] = useState(false);
  const [busy, setBusy] = useState("");

  useEffect(() => {
    const load = () => api.activity().then(setEvents, () => setEvents([]));
    void load();
    // Opening the page reads everything (the sidebar count goes away), a moment after it shows.
    const t = setTimeout(() => void api.markActivity(false), 1500);
    const off = on<number>("activity:changed", () => void load());
    return () => {
      clearTimeout(t);
      void off.then((f) => f());
      void api.markActivity(false);
    };
  }, []);

  // Live: each check anywhere in the app (the Profiles page, after a game closes, a fix here)
  // shows up at once.
  useEffect(() => {
    const read = () => {
      void api.profileChecks().then(setChecks, () => {});
      void api.profiles().then(setProfiles, () => {});
    };
    read();
    api.ignored().then(setIgnored, () => {});
    const off = on<number>("checks:changed", read);
    window.addEventListener("focus", read);
    return () => {
      void off.then((f) => f());
      window.removeEventListener("focus", read);
    };
  }, []);
  useEffect(() => {
    api.library().then(setLibrary, () => {});
  }, [libraryVersion]);

  const ikey = (p: Profile, pr: Problem) => `${p.id}|${pr.key}`;
  const now = useMemo(
    () =>
      profiles
        .filter((p) => checks[p.id])
        .map((p) => {
          const all = problemsOf(checks[p.id]);
          return { p, open: all.filter((x) => !ignored.includes(ikey(p, x))), quiet: all.filter((x) => ignored.includes(ikey(p, x))) };
        })
        .filter((x) => x.open.length || (showIgnored && x.quiet.length)),
    [profiles, checks, ignored, showIgnored],
  );
  const unchecked = profiles.filter((p) => !checks[p.id]);
  const ignoredCount = profiles.reduce((n, p) => n + (checks[p.id] ? problemsOf(checks[p.id]).filter((x) => ignored.includes(ikey(p, x))).length : 0), 0);

  const recheck = async (id: string) => {
    await api.resolveProfile(id).catch(() => null);
    const r = await api.checkProfile(id).catch(() => null);
    if (r) setChecks((c) => ({ ...c, [id]: r }));
  };
  const busyRun = async (label: string, f: () => Promise<void>) => {
    setBusy(label);
    try {
      await f();
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setBusy("");
    }
  };
  /** Fix for me: the same as on the Profiles page. */
  const fixProfile = (p: Profile) =>
    busyRun(`fix:${p.id}`, async () => {
      const f = autoFix(p, library, checks[p.id]);
      if (f.done.length) await api.saveProfile(f.profile);
      const queued = f.download.length ? (await api.queueRequirements(p.game, f.download)).queued.length : 0;
      await recheck(p.id);
      const said = [...f.done, queued ? `added ${plural(queued, "missing requirement")} to downloads` : ""].filter(Boolean);
      toast(said.length ? `“${p.name}”: ${said.join("; ")}.` : `Nothing in “${p.name}” that Zom's Modzilla can fix by itself. Open it for a look.`);
    });
  const applyProfile = (p: Profile) =>
    busyRun(`apply:${p.id}`, async () => {
      if (gameStates[p.game]?.state === "running") throw new Error("Quit the game first: mods can't move while it's running.");
      await api.applyProfile(p.id);
      await recheck(p.id);
      toast(`Put “${p.name}” in the game again`);
    });
  const ignore = (key: string, yes: boolean) => void api.ignore(key, yes).then(setIgnored, (e) => toast((e as Error).message));
  const openProfile = (p: Profile) => {
    wantProfile(p.id);
    go("profiles", p.game);
  };

  const problems = (e: ActivityEvent) => ["failed", "worse", "gone"].includes(e.kind);
  /** Problems since dealt with: the profile has nothing left to fix, the downloads are in the
   * library, the gone mod was deleted. They're hidden unless asked for. */
  const resolved = (e: ActivityEvent) => {
    const have = (name: string) => library.some((m) => m.name === name);
    if (e.kind === "worse") {
      const p = allProfiles.find((x) => x.id === e.profile);
      if (!p) return true;
      const r = checks[p.id];
      return !!r && problemsOf(r).every((x) => ignored.includes(ikey(p, x)));
    }
    if (e.kind === "failed") {
      const names = e.detail?.split("page: ")[1]?.replace(/\.$/, "").split(", ") ?? [];
      return names.length > 0 && library.length > 0 && names.every(have);
    }
    if (e.kind === "gone") {
      const name = e.title.match(/“(.+)”/)?.[1];
      return !!name && library.length > 0 && !have(name);
    }
    return false;
  };
  const [showResolved, setShowResolved] = useState(false);
  const resolvedCount = (events ?? []).filter(resolved).length;
  const shown = (events ?? []).filter((e) => (!narrow || !e.game || e.game === game) && (show === "all" || problems(e)) && (showResolved || !resolved(e)));

  return (
    <div className="page">
      <div className="page-head">
        <h1>Activity</h1>
        <OnlyGameToggle only={onlyGame} onChange={setOnlyGame} gameName={game ? (games.find((g) => g.domain === game)?.name ?? game) : undefined} />
      </div>
      <p className="muted" style={{ marginTop: -8 }}>
        What needs fixing in your profiles now, kept up to date as checks run, and below it what happened while you weren't looking. For troubleshooting, the detailed log is in Settings.
      </p>

      <div className="section-head">
        <h2>Needs attention</h2>
        <span className="grow" />
        {ignoredCount > 0 && (
          <button className="btn ghost sm" onClick={() => setShowIgnored((s) => !s)}>
            <EyeOff size={14} /> {showIgnored ? "Hide ignored" : `Show ${ignoredCount} ignored`}
          </button>
        )}
        {unchecked.length > 0 && (
          <button
            className="btn sm"
            disabled={!!busy}
            title="Checks profiles not checked since the app started. It reads this computer, plus Nexus Mods for mods not asked about today."
            onClick={() =>
              busyRun("checkAll", async () => {
                for (const p of unchecked) await recheck(p.id);
              })
            }
          >
            <ShieldCheck size={14} /> {busy === "checkAll" ? "Checking…" : `Check ${plural(unchecked.length, "other profile")}`}
          </button>
        )}
      </div>
      {now.length === 0 ? (
        <div className="panel attention-clear">
          <CheckCircle2 size={18} color="var(--zb-ok)" />
          <span>{Object.keys(checks).length ? "Nothing to fix in the profiles checked." : "No profiles checked yet since the app started."}</span>
        </div>
      ) : (
        <div className="attention">
          {now.map(({ p, open, quiet }) => {
            const autoable = open.some((x) => x.fix === "auto");
            return (
              <div key={p.id} className="panel attention-profile">
                <div className="row gap wrap attention-head">
                  <b className="ellipsis">{p.name}</b>
                  <GameTag domain={p.game} />
                  <span className="muted small">{open.length ? plural(open.length, "problem") : "all ignored"}</span>
                  <span className="grow" />
                  {autoable && (
                    <button className="btn primary sm" disabled={!!busy} onClick={() => void fixProfile(p)} title="Puts in and switches on requirements, downloads missing ones, and points mods at files you have">
                      <Wand2 size={14} /> {busy === `fix:${p.id}` ? "Fixing…" : "Fix for me"}
                    </button>
                  )}
                  <button className="btn sm" onClick={() => openProfile(p)}>
                    Open the profile
                  </button>
                </div>
                {[...open, ...(showIgnored ? quiet : [])].map((pr) => {
                  const isIgnored = quiet.includes(pr);
                  return (
                    <div key={pr.key} className={`attention-row${isIgnored ? " ignored" : ""}`}>
                      <AlertTriangle size={15} color={pr.tone === "danger" ? "var(--zb-danger)" : "var(--zb-warn)"} />
                      <div className="grow">
                        <div className="small">{pr.title}</div>
                        {pr.detail && <div className="muted small">{pr.detail}</div>}
                      </div>
                      {pr.fix === "apply" && !isIgnored && (
                        <button className="btn sm" disabled={!!busy} onClick={() => void applyProfile(p)}>
                          <Upload size={14} /> {busy === `apply:${p.id}` ? "Putting in…" : "Put mods in game"}
                        </button>
                      )}
                      {pr.fix === "copy" && pr.copy && !isIgnored && (
                        <button className="btn sm" onClick={() => void navigator.clipboard.writeText(pr.copy!).then(() => toast("Copied. Paste it into the game's launch options in Steam."))}>
                          <Copy size={14} /> Copy launch options
                        </button>
                      )}
                      <button className="btn ghost sm" onClick={() => ignore(`${p.id}|${pr.key}`, !isIgnored)} title={isIgnored ? "Show this problem again" : "Stop showing this problem here (the profile's check still lists it)"}>
                        {isIgnored ? "Stop ignoring" : "Ignore"}
                      </button>
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>
      )}

      <div className="section-head">
        <h2>What happened</h2>
        <span className="grow" />
        {events && events.length > 0 && (
          <>
            <div className="seg">
              <button className={show === "all" ? "active" : ""} onClick={() => setShow("all")}>
                Everything
              </button>
              <button className={show === "problems" ? "active" : ""} onClick={() => setShow("problems")}>
                Problems only
              </button>
            </div>
            {resolvedCount > 0 && (
              <label className="check" title="Problems that have been dealt with since">
                <input type="checkbox" checked={showResolved} onChange={(e) => setShowResolved(e.target.checked)} /> Show {resolvedCount} resolved
              </label>
            )}
            <button className="btn ghost sm" onClick={() => setClearing(true)}>
              <Trash2 size={14} /> Clear
            </button>
          </>
        )}
      </div>
      {events === null ? null : shown.length === 0 ? (
        <div className="empty">
          <Bell size={32} />
          <p>{events.length ? (resolvedCount ? "Nothing open. Problems that were dealt with are hidden." : "No problems. Everything else is under Everything.") : "Nothing yet. Downloads that didn't finish, updates found, mods gone from Nexus Mods, saves switched and time played show up here."}</p>
        </div>
      ) : (
        <div className="activity">
          {shown.map((e) => {
            const look = LOOK[e.kind] ?? { icon: AlertTriangle, tone: "", open: "Open" };
            const Icon = look.icon;
            return (
              <div key={e.id} className={`panel activity-row${e.read ? "" : " unread"}${resolved(e) ? " resolved" : ""}`}>
                <span className={`activity-icon ${look.tone}`}>
                  <Icon size={16} />
                </span>
                <div className="grow">
                  <b>{e.title}</b>
                  {resolved(e) && <span className="pill ok" style={{ marginLeft: 8 }}>Resolved</span>}
                  {e.detail && <div className="muted small">{e.detail}</div>}
                  <div className="meta">
                    <span>{when(e.at)}</span>
                    {e.game && <GameTag domain={e.game} />}
                  </div>
                </div>
                {e.page && (
                  <button
                    className="btn sm"
                    onClick={() => {
                      if (e.profile) wantProfile(e.profile);
                      go(e.page as Page, e.game ?? undefined);
                    }}
                  >
                    {look.open}
                  </button>
                )}
              </div>
            );
          })}
        </div>
      )}
      {clearing && <Confirm title="Clear what happened?" body="The list under What happened goes. Problems that still need fixing stay above, and the log in Settings is kept." action="Clear" onConfirm={() => void api.markActivity(true).then(() => setEvents([]))} onClose={() => setClearing(false)} />}
    </div>
  );
}
