import { useEffect, useState } from "react";
import { ArrowDownWideNarrow, ArrowUpNarrowWide, ArrowUpToLine, ChevronDown, ChevronUp, Download, ExternalLink, FolderOpen, Gauge, Layers, MousePointerClick, Pause, Play, Plus, RotateCcw, Trash2, Zap } from "lucide-react";
import { api, fileUrl, modUrl, openUrl, type Job, type ModFacts, type StoredMod } from "../api";
import { GameTag } from "../art";
import { awaitingClick, type Ctx } from "../App";
import { bytes, eta, percent, plural, speed } from "../format";
import { Confirm, StatusPill, Stepper, tidy } from "../ui";
import { AddToProfile } from "./AddToProfile";
import { NeedsDialog, NeedsLine } from "./Needs";

const MAX = 8;

/** Mods whose requirements were asked for this run, and the answers. */
const asked = new Set<string>();
const known = new Map<string, ModFacts>();

/** Speed limit choices, in bytes a second (0 = no limit). */
export const LIMITS: [number, string][] = [
  [0, "No limit"],
  [1_000_000, "1 MB/s"],
  [2_000_000, "2 MB/s"],
  [5_000_000, "5 MB/s"],
  [10_000_000, "10 MB/s"],
  [25_000_000, "25 MB/s"],
  [50_000_000, "50 MB/s"],
];

/** How files download: automatic by account, one at a time, or several at once; and how fast. */
function ModePanel({ overview, setOverview, toast }: Ctx) {
  const s = overview.settings;
  const premium = overview.user?.isPremium ?? false;
  const save = async (patch: Partial<typeof s>) => {
    try {
      setOverview(await api.saveSettings({ ...s, ...patch }));
    } catch (e) {
      toast((e as Error).message);
    }
  };
  const several = s.mode === "concurrent" || (s.mode === "auto" && premium);
  const hint =
    s.mode === "auto"
      ? premium
        ? `Your Premium account downloads up to ${plural(s.maxConcurrent, "file")} at once, at full speed.`
        : "Your free account downloads one file at a time: Nexus Mods slows free downloads down, so several at once would only share the same speed."
      : s.mode === "sequential"
        ? "Each file downloads, goes into the library and is unpacked before the next one starts, in the order of the list."
        : `Up to ${plural(s.maxConcurrent, "file")} download at the same time, in the order of the list.`;
  return (
    <div className="panel mode">
      <b>Download</b>
      <div className="seg" role="radiogroup" aria-label="How files download">
        {(
          [
            ["auto", "Match my account"],
            ["sequential", "One at a time"],
            ["concurrent", "Several at once"],
          ] as const
        ).map(([m, label]) => (
          <button key={m} role="radio" aria-checked={s.mode === m} className={s.mode === m ? "active" : ""} onClick={() => save({ mode: m })}>
            {label}
          </button>
        ))}
      </div>
      {several && (
        <span className="row gap-sm">
          <span className="muted">Up to</span>
          <Stepper label="downloads at once" value={s.maxConcurrent} min={1} max={MAX} onChange={(n) => save({ maxConcurrent: n })} />
          <span className="muted">at once</span>
        </span>
      )}
      <span className="grow" />
      <label className="row gap-sm">
        <Gauge size={15} className="muted" />
        <span className="muted">Speed</span>
        <select value={s.speedLimit} onChange={(e) => save({ speedLimit: Number(e.target.value) })} aria-label="Speed limit">
          {LIMITS.some(([v]) => v === s.speedLimit) ? null : <option value={s.speedLimit}>{speed(s.speedLimit)}</option>}
          {LIMITS.map(([v, label]) => (
            <option key={v} value={v}>
              {label}
            </option>
          ))}
        </select>
      </label>
      <span className="hint">
        {hint}
        {s.speedLimit > 0 && ` All downloads together stay under ${speed(s.speedLimit)}, so games and calls on the same connection stay smooth.`}
      </span>
    </div>
  );
}

function Progress({ j }: { j: Job }) {
  const p = percent(j.received, j.size);
  const kind = j.status === "done" ? "ok" : j.status === "failed" ? "danger" : ["paused", "needsLink", "queued", "waiting"].includes(j.status) ? "idle" : "";
  const busy = j.status === "starting" || j.status === "storing" || (j.status === "downloading" && !j.size);
  return (
    <>
      <div className={`progress ${kind}${busy ? " busy" : ""}`} role="progressbar" aria-valuenow={p} aria-valuemin={0} aria-valuemax={100}>
        <div style={{ width: `${j.status === "done" ? 100 : p}%` }} />
      </div>
      <div className="sub num">
        {j.status === "downloading"
          ? `${bytes(j.received)} of ${bytes(j.size)} · ${speed(j.speed)}${j.speed ? ` · ${eta(j.size - j.received, j.speed)} left` : ""}`
          : j.status === "done"
            ? bytes(j.size)
            : j.size
              ? `${bytes(j.received)} of ${bytes(j.size)}`
              : ""}
      </div>
    </>
  );
}

export function Downloads(ctx: Ctx) {
  const { jobs: allJobs, loaded, toast, openAdd, game, games, guiding, guide, menu } = ctx;
  const clicks = allJobs.filter(awaitingClick);
  const failed = allJobs.filter((j) => j.status === "failed").length;

  // What each listed mod needs: one request per game for the mods not looked up yet (and the
  // app keeps the answers for a day).
  const [library, setLibrary] = useState<StoredMod[]>([]);
  const [facts, setFacts] = useState<Record<string, ModFacts>>({});
  const [needsOf, setNeedsOf] = useState<ModFacts | null>(null);
  useEffect(() => {
    api.library().then(setLibrary, () => {});
  }, [ctx.libraryVersion]);
  const modKeys = [...new Set(allJobs.map((j) => `${j.game}:${j.modId}`))].sort().join(",");
  useEffect(() => {
    const byGame = new Map<string, number[]>();
    for (const k of modKeys ? modKeys.split(",") : []) {
      if (asked.has(k)) continue;
      asked.add(k);
      const [g, m] = k.split(":");
      byGame.set(g, [...(byGame.get(g) ?? []), Number(m)]);
    }
    for (const [g, ids] of byGame)
      api.modRequirements(g, ids).then(
        (found) => {
          for (const f of found) known.set(`${f.game}:${f.modId}`, f);
          setFacts(Object.fromEntries(known));
        },
        () => ids.forEach((id) => asked.delete(`${g}:${id}`)),
      );
    setFacts(Object.fromEntries(known));
  }, [modKeys]);
  const [removing, setRemoving] = useState<Job | null>(null);
  const [profiling, setProfiling] = useState<Job | null>(null);
  // Only the picked game's files; the order (and moving up and down) is still the whole list's.
  const jobs = game ? allJobs.filter((j) => j.game === game) : allJobs;
  const elsewhere = allJobs.length - jobs.length;
  const gameName = (d: string) => games.find((g) => g.domain === d)?.name ?? d;
  const act = (p: Promise<unknown>) => p.catch((e) => toast((e as Error).message));
  const waiting = jobs.filter((j) => j.status !== "done");
  const done = jobs.filter((j) => j.status === "done");
  const canPause = allJobs.some((j) => ["queued", "starting", "downloading", "waiting"].includes(j.status));
  const canResume = allJobs.some((j) => j.status === "paused");
  const total = allJobs.filter((j) => j.status === "downloading").reduce((n, j) => n + j.speed, 0);

  const rowMenu = (j: Job, i: number) =>
    tidy([
      ["queued", "paused", "failed", "waiting"].includes(j.status) && { label: "Download now", icon: <Zap size={14} />, run: () => act(api.now(j.id)) },
      ["queued", "starting", "downloading", "waiting"].includes(j.status) && { label: "Pause", icon: <Pause size={14} />, run: () => act(api.pause(j.id)) },
      j.status === "paused" && { label: "Resume", icon: <Play size={14} />, run: () => act(api.resume(j.id)) },
      j.status === "failed" && { label: "Try again", icon: <RotateCcw size={14} />, run: () => act(api.resume(j.id)) },
      j.status === "needsLink" && { label: awaitingClick(j) ? "Open its download page" : "Get a new link", icon: <ExternalLink size={14} />, run: () => act(openUrl(fileUrl(j.game, j.modId, j.fileId))) },
      j.status === "done" && { label: "Add to profile…", icon: <Layers size={14} />, run: () => setProfiling(j) },
      j.status === "done" && { label: "Open folder", icon: <FolderOpen size={14} />, run: () => act(api.openFolder({ game: j.game, modId: j.modId })) },
      "sep",
      j.status !== "done" && i > 0 && { label: "Move to the top", icon: <ArrowUpToLine size={14} />, run: () => act(api.reorder(j.id, allJobs.indexOf(waiting[0]))) },
      { label: "Open the mod's page", icon: <ExternalLink size={14} />, run: () => act(openUrl(modUrl(j.game, j.modId))) },
      "sep",
      j.status !== "storing" && {
        label: j.status === "done" ? "Take off the list" : "Cancel download",
        icon: <Trash2 size={14} />,
        danger: j.status !== "done",
        run: () => (j.status === "done" || j.received === 0 ? act(api.remove(j.id)) : setRemoving(j)),
      },
    ]);

  const row = (j: Job, i: number) => (
    <tr key={j.id} onContextMenu={(e) => menu(e, rowMenu(j, j.status === "done" ? 0 : i))}>
      <td>
        <div className="name">
          <button className="link-name" title="Open the mod's page on Nexus Mods" onClick={() => act(openUrl(modUrl(j.game, j.modId)))}>
            {j.modName}
            <ExternalLink size={12} />
          </button>
        </div>
        <div className="sub">
          {j.name}
          {j.version ? ` · ${j.version}` : ""}
        </div>
        {facts[`${j.game}:${j.modId}`] && <NeedsLine facts={facts[`${j.game}:${j.modId}`]} library={library} jobs={allJobs} onOpen={() => setNeedsOf(facts[`${j.game}:${j.modId}`])} />}
        {j.error && !awaitingClick(j) && <div className={j.status === "failed" ? "err-text" : "warn-text"}>{j.error}</div>}
      </td>
      <td className="hide-narrow muted small">
        <GameTag domain={j.game} />
      </td>
      <td>{awaitingClick(j) ? <span className="pill" title="Waiting for you to click Slow download on the file's page">Waiting for you</span> : <StatusPill status={j.status} />}</td>
      <td className="prog">
        <Progress j={j} />
      </td>
      <td className="actions">
        {j.status === "needsLink" && (
          <button className="btn sm" onClick={() => act(openUrl(fileUrl(j.game, j.modId, j.fileId)))} title="Opens the file's mod-manager download page on Nexus Mods; click Slow download there">
            <ExternalLink size={14} /> {awaitingClick(j) ? "Open page" : "Get a new link"}
          </button>
        )}
        {["queued", "paused", "failed", "waiting"].includes(j.status) && (
          <button className="btn sm" title="Start this one now. If every download slot is busy, the newest one steps aside and carries on right after." onClick={() => act(api.now(j.id))}>
            <Zap size={13} /> Download now
          </button>
        )}
        {j.status === "needsLink" && i > 0 && (
          <button className="icon-btn" aria-label="Move to the top" title="Move to the top" onClick={() => act(api.reorder(j.id, allJobs.indexOf(waiting[0])))}>
            <ArrowUpToLine size={15} />
          </button>
        )}
        {["queued", "paused", "failed", "waiting", "needsLink"].includes(j.status) && i > 0 && (
          <button className="icon-btn" aria-label="Move up" title="Move up" onClick={() => act(api.reorder(j.id, allJobs.indexOf(waiting[i - 1])))}>
            <ChevronUp size={15} />
          </button>
        )}
        {["queued", "paused", "failed", "waiting", "needsLink"].includes(j.status) && i < waiting.length - 1 && (
          <button className="icon-btn" aria-label="Move down" title="Move down" onClick={() => act(api.reorder(j.id, allJobs.indexOf(waiting[i + 1])))}>
            <ChevronDown size={15} />
          </button>
        )}
        {["queued", "starting", "downloading", "waiting"].includes(j.status) && (
          <button className="icon-btn" aria-label="Pause" title="Pause" onClick={() => act(api.pause(j.id))}>
            <Pause size={15} />
          </button>
        )}
        {j.status === "paused" && (
          <button className="icon-btn" aria-label="Resume" title="Resume" onClick={() => act(api.resume(j.id))}>
            <Play size={15} />
          </button>
        )}
        {(j.status === "failed" || j.status === "needsLink") && (
          <button className="icon-btn" aria-label="Try again" title="Try again" onClick={() => act(api.resume(j.id))}>
            <RotateCcw size={15} />
          </button>
        )}
        {j.status === "done" && (
          <button className="btn sm" title="Put this mod in one of the game's profiles" onClick={() => setProfiling(j)}>
            <Layers size={13} /> Add to profile
          </button>
        )}
        {j.status === "done" && (
          <button className="icon-btn" aria-label="Open folder" title="Open folder" onClick={() => act(api.openFolder({ game: j.game, modId: j.modId }))}>
            <FolderOpen size={15} />
          </button>
        )}
        {j.status !== "storing" && (
          <button
            className="icon-btn danger"
            aria-label={j.status === "done" ? "Take off the list" : "Cancel"}
            title={j.status === "done" ? "Take off the list (the file stays in the library)" : "Cancel"}
            onClick={() => (j.status === "done" || j.received === 0 ? act(api.remove(j.id)) : setRemoving(j))}
          >
            <Trash2 size={15} />
          </button>
        )}
      </td>
    </tr>
  );

  return (
    <div className="page">
      <div className="page-head">
        <h1>Downloads</h1>
        {total > 0 && <span className="muted num">{speed(total)}</span>}
        {ctx.overview.freeSpace != null && <span className="muted num hide-narrow">{bytes(ctx.overview.freeSpace)} free</span>}
        <button className="btn" disabled={!canPause} onClick={() => act(api.pauseAll())}>
          <Pause size={15} /> Pause all
        </button>
        <button className="btn" disabled={!canResume} onClick={() => act(api.resumeAll())}>
          <Play size={15} /> Resume all
        </button>
        <button className="btn primary" onClick={() => openAdd()} title="Add a mod or collection (N)">
          <Plus size={15} /> Add mod…
        </button>
      </div>
      <ModePanel {...ctx} />
      {clicks.length > 0 && (
        <div className="panel banner-panel">
          <MousePointerClick size={18} className="accent-text" />
          <div className="grow">
            <b>
              {clicks.length === 1 ? "1 file is" : `${clicks.length} files are`} waiting for your click on Nexus Mods
            </b>
            <p className="muted small" style={{ margin: "2px 0 0" }}>
              {guiding
                ? `In the Nexus Mods window, click Slow download (not Manual download). When the file arrives here, the same window moves on to the next one, and it closes after the last.`
                : "Free accounts download each file by clicking Slow download on its page, as Nexus Mods asks. Zom's Nexus shows the pages one after another in its own Nexus Mods window (sign in there the first time)."}
            </p>
          </div>
          {guiding ? (
            <>
              <button className="btn" onClick={() => act(api.nexusWindow(fileUrl(clicks[0].game, clicks[0].modId, clicks[0].fileId)))}>
                <ExternalLink size={15} /> Show the page again
              </button>
              <button
                className="btn ghost"
                onClick={() => {
                  guide(false);
                  void api.closeNexusWindow();
                }}
              >
                Stop
              </button>
            </>
          ) : (
            <button className="btn primary" onClick={() => guide(true)}>
              <ExternalLink size={15} /> Get them one by one
            </button>
          )}
        </div>
      )}
      {game && elsewhere > 0 && (
        <p className="other-games muted small">
          Showing {gameName(game)}. {plural(elsewhere, "more file")} for other games {elsewhere === 1 ? "is" : "are"} in the list too; pick that game to see {elsewhere === 1 ? "it" : "them"}.
        </p>
      )}
      {!loaded ? null : jobs.length === 0 ? (
        <div className="empty">
          <Download size={32} />
          <p>{game ? `${gameName(game)} files you download from Nexus Mods show up here while they download.` : "Files you download from Nexus Mods show up here while they download."}</p>
          <button className="btn primary" onClick={() => openAdd()}>
            Add mod…
          </button>
        </div>
      ) : (
        <>
        {(waiting.filter((j) => !j.status.match(/starting|downloading|storing/)).length > 1 || failed > 0) && (
          <div className="toolbar">
            {waiting.filter((j) => !j.status.match(/starting|downloading|storing/)).length > 1 && (
              <span className="row gap-sm">
                <span className="muted small">Order the waiting files</span>
                <button className="btn sm" title="Smallest first: the most mods done soonest" onClick={() => act(api.sortBySize(true))}>
                  <ArrowUpNarrowWide size={13} /> Smallest first
                </button>
                <button className="btn sm" onClick={() => act(api.sortBySize(false))}>
                  <ArrowDownWideNarrow size={13} /> Biggest first
                </button>
              </span>
            )}
            <span className="grow" />
            {failed > 0 && (
              <button className="btn sm" onClick={() => act(api.retryFailed())}>
                <RotateCcw size={13} /> Try {failed === 1 ? "the failed one" : `all ${failed} failed`} again
              </button>
            )}
          </div>
        )}
        <div className="table-wrap">
          <table className="table">
            <thead>
              <tr>
                <th style={{ minWidth: 220 }}>Mod</th>
                <th className="hide-narrow">Game</th>
                <th>Status</th>
                <th>Progress</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {waiting.map(row)}
              {done.length > 0 && (
                <tr className="group-row">
                  <td colSpan={5}>
                    <div className="row between">
                      <span>Finished · {plural(done.length, "file")}</span>
                      <button className="btn sm ghost" onClick={() => act(api.clearFinished())}>
                        Clear finished
                      </button>
                    </div>
                  </td>
                </tr>
              )}
              {done.map(row)}
            </tbody>
          </table>
        </div>
        </>
      )}
      {needsOf && <NeedsDialog facts={needsOf} library={library} jobs={allJobs} onClose={() => setNeedsOf(null)} toast={toast} />}
      {profiling && (
        <AddToProfile
          game={profiling.game}
          gameName={gameName(profiling.game)}
          modId={profiling.modId}
          fileId={profiling.fileId}
          modName={profiling.modName}
          onClose={() => setProfiling(null)}
          onDone={(t) => toast(t)}
        />
      )}
      {removing && (
        <Confirm
          title="Cancel this download?"
          body={`“${removing.name}” stops, and the ${bytes(removing.received)} downloaded so far is deleted.`}
          action="Cancel download"
          onConfirm={() => void act(api.remove(removing.id))}
          onClose={() => setRemoving(null)}
        />
      )}
    </div>
  );
}
