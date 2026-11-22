import { useEffect, useMemo, useRef, useState } from "react";
import { AlertTriangle, ArrowDown, ArrowUp, Check, Copy, FileDown, FileUp, GripVertical, Layers, Play, Plus, RefreshCw, RotateCcw, Search, ShieldCheck, StickyNote, Trash2, Upload } from "lucide-react";
import { rememberUpdates } from "../progress";
import { NoteDialog } from "./NoteDialog";
import { api, isDesktop, modUrl, openUrl, pickFile, pickSave, type Applied, type CheckReport, type Game, type Profile, type StoredMod } from "../api";
import type { Ctx } from "../App";
import { Cover, Hero } from "../art";
import { plural, when } from "../format";
import { profileProgress } from "../progress";
import { Confirm, Dialog } from "../ui";

function NewProfile({ games, game, onClose, onMade }: { games: Game[]; game: string | null; onClose: () => void; onMade: (p: Profile) => void }) {
  const [name, setName] = useState("");
  const [g, setG] = useState(game ?? games[0]?.domain ?? "");
  const [error, setError] = useState("");
  const save = async () => {
    try {
      onMade(await api.saveProfile({ id: "", name, game: g, mods: [] }));
      onClose();
    } catch (e) {
      setError((e as Error).message);
    }
  };
  return (
    <Dialog
      title="New profile"
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" disabled={!name.trim() || !g} onClick={save}>
            Make profile
          </button>
        </>
      }
    >
      <div className="form-grid">
        <label className="field full">
          <span>Name</span>
          <input autoFocus placeholder="Winter playthrough" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && name.trim() && void save()} />
        </label>
        <label className="field full">
          <span>Game</span>
          <select value={g} onChange={(e) => setG(e.target.value)}>
            {games.map((x) => (
              <option key={x.domain} value={x.domain}>
                {x.name}
              </option>
            ))}
          </select>
        </label>
        {error && <span className="err full">{error}</span>}
      </div>
    </Dialog>
  );
}

/** Pick mods from the library to add to a profile. */
function AddMods({ profile, mods, onClose, onAdd }: { profile: Profile; mods: StoredMod[]; onClose: () => void; onAdd: (picked: { modId: number; fileId: number }[]) => void }) {
  const [filter, setFilter] = useState("");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const inProfile = new Set(profile.mods.map((m) => `${m.modId}:${m.fileId}`));
  const shown = mods.filter((m) => m.game === profile.game && (!filter || m.name.toLowerCase().includes(filter.toLowerCase())));
  const toggle = (k: string) => {
    const n = new Set(picked);
    if (n.has(k)) n.delete(k);
    else n.add(k);
    setPicked(n);
  };
  return (
    <Dialog
      wide
      title={`Add mods to “${profile.name}”`}
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn primary"
            disabled={picked.size === 0}
            onClick={() => {
              onAdd([...picked].map((k) => ({ modId: Number(k.split(":")[0]), fileId: Number(k.split(":")[1]) })));
              onClose();
            }}
          >
            Add {picked.size ? plural(picked.size, "file") : ""}
          </button>
        </>
      }
    >
      <label className="search" style={{ display: "block", marginBottom: 12 }}>
        <Search size={14} />
        <input autoFocus placeholder="Filter mods" value={filter} onChange={(e) => setFilter(e.target.value)} style={{ width: "100%" }} />
      </label>
      {shown.length === 0 ? (
        <p className="muted">No mods for this game in the library yet. Download some first, from Browse or Add mod.</p>
      ) : (
        <div className="pick-list">
          {shown.flatMap((m) =>
            m.files.map((f) => {
              const k = `${m.modId}:${f.fileId}`;
              const already = inProfile.has(k);
              return (
                <label key={k} className="pick-item" style={already ? { opacity: 0.55, cursor: "default" } : undefined}>
                  <input type="checkbox" disabled={already} checked={already || picked.has(k)} onChange={() => toggle(k)} />
                  <div className="grow">
                    <b>{m.name}</b>
                    <div className="meta">
                      {f.name}
                      {f.version ? ` · ${f.version}` : ""}
                      {already ? " · already in this profile" : ""}
                    </div>
                  </div>
                </label>
              );
            }),
          )}
        </div>
      )}
    </Dialog>
  );
}

/** Problems that stop mods working: missing masters and requirements, files not downloaded. */
export function problemCount(r: CheckReport) {
  return r.plugins.length + r.missing.length + (r.needs ?? []).filter((n) => ["notDownloaded", "notInProfile", "switchedOff"].includes(n.kind)).length;
}

const day = (t?: number | null) => (t ? new Date(t * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" }) : "");

function CheckPanel({ report, gameName, onGet, onAdd, onSwitchOn, onGetAll }: { report: CheckReport; gameName: string; onGet: (modId: number) => void; onAdd: (modId: number) => void; onSwitchOn: (modId: number) => void; onGetAll: (modIds: number[]) => void }) {
  const ok = problemCount(report) === 0;
  const needs = report.needs ?? [];
  const compat = report.compat ?? [];
  const by = (n: { neededBy: string[] }) => (n.neededBy.length > 2 ? `${n.neededBy.slice(0, 2).join(", ")} and ${n.neededBy.length - 2} more` : n.neededBy.join(" and "));
  return (
    <div className="panel check-panel">
      <h3>
        {ok ? <ShieldCheck size={15} color="var(--zb-ok)" /> : <AlertTriangle size={15} color="var(--zb-warn)" />}
        {ok ? (compat.length || (report.programs ?? []).length ? "Ready to play, with warnings" : "Ready to play") : "Problems to look at before playing"}
        <span className="grow" />
        <span className="muted small">Checked {when(report.at)}</span>
      </h3>
      <ul className="check-list">
        {needs.map((n) => (
          <li key={n.kind + n.modId + n.name} className={`check-row ${["notDownloaded", "notInProfile", "switchedOff"].includes(n.kind) ? "err-text" : "warn-text"}`}>
            <span className="grow">
              {n.kind === "dlc" ? (
                <>
                  {by(n)} {n.neededBy.length > 1 ? "need" : "needs"} the <b>{n.name}</b> expansion.
                </>
              ) : (
                <>
                  {by(n)} {n.neededBy.length > 1 ? "need" : "needs"} <b>{n.name}</b>
                  {n.kind === "notDownloaded" && ", which isn't downloaded."}
                  {n.kind === "notInProfile" && ", which is in your library but not in this profile."}
                  {n.kind === "switchedOff" && ", which is switched off in this profile."}
                  {n.kind === "external" && " from outside Nexus Mods. Zom's Nexus can't check this one; get it from its page."}
                  {n.kind === "otherGame" && " (a tool on Nexus Mods). Zom's Nexus can't check this one; see its page."}
                </>
              )}
              {n.notes && <span className="muted"> Note from the author: “{n.notes}”</span>}
            </span>
            {n.kind === "notDownloaded" && (
              <button className="btn sm" onClick={() => onGet(n.modId)}>
                Get…
              </button>
            )}
            {n.kind === "notInProfile" && (
              <button className="btn sm" onClick={() => onAdd(n.modId)}>
                Add to profile
              </button>
            )}
            {n.kind === "switchedOff" && (
              <button className="btn sm" onClick={() => onSwitchOn(n.modId)}>
                Switch on
              </button>
            )}
            {n.url && (
              <button className="btn sm ghost" onClick={() => void openUrl(n.url!)}>
                Open page
              </button>
            )}
          </li>
        ))}
        {(report.programs ?? []).map((p) => (
          <li key={"prog" + p.modName} className="check-row warn-text">
            <span className="grow">
              <b>{p.modName}</b> has {p.count === 1 ? "a program" : `${p.count} programs`} in it ({p.files.join(", ")}
              {p.count > p.files.length ? ", …" : ""}). Mods rarely need these. Only run them if the mod's page says to, and you trust its author.
            </span>
          </li>
        ))}
        {compat.map((c) => (
          <li key={c.kind + c.modId} className="check-row warn-text">
            <span className="grow">
              {c.kind === "removed" ? (
                <>
                  <b>{c.modName}</b> is no longer published on Nexus Mods, so it won't get fixes. Check whether something replaced it.
                </>
              ) : c.kind === "newerFile" ? (
                <>
                  <b>{c.modName}</b> runs code in the game, and your file{c.fileDate ? ` (${day(c.fileDate)})` : ""} is from before {gameName}'s last update ({day(report.gameUpdated)}). A newer version came out {day(c.modUpdated)}: update it before playing.
                </>
              ) : (
                <>
                  <b>{c.modName}</b> runs code in the game and hasn't been updated since {gameName}'s last update ({day(report.gameUpdated)}). It may not work until its author updates it; check its page if the game crashes or the mod doesn't load.
                </>
              )}
            </span>
            {c.kind === "newerFile" && (
              <button className="btn sm" onClick={() => onGet(c.modId)}>
                Get the update…
              </button>
            )}
          </li>
        ))}
        {report.missing.length > 3 ? (
          <li className="warn-text" title={report.missing.join(", ")}>
            {plural(report.missing.length, "mod")} aren't downloaded yet, so they're left out for now.
          </li>
        ) : (
          report.missing.map((m) => (
            <li key={m} className="warn-text">
              “{m}” isn't downloaded yet, so it's left out.
            </li>
          ))
        )}
        {report.plugins.map((p) => (
          <li key={p.plugin + p.master} className="err-text">
            {p.kind === "missing" ? (
              <>
                <b>{p.plugin}</b> ({p.modName}) needs <b>{p.master}</b>, which isn't in the game or this profile. The game will likely crash at start.
              </>
            ) : (
              <>
                <b>{p.plugin}</b> ({p.modName}) needs <b>{p.master}</b>, which loads after it. Move the mod with {p.master} higher in the list.
              </>
            )}
          </li>
        ))}
        {report.installers.map((m) => (
          <li key={m} className="warn-text">
            “{m}” has an installer with choices. Zom's Nexus puts in its main files only; check the mod's page for what to pick.
          </li>
        ))}
        {report.conflicts.map((c) => (
          <li key={c.winner + c.losers.join()}>
            <b>{c.winner}</b> replaces {plural(c.files, "file")} from {c.losers.map((l, i) => (
              <span key={l}>
                {i > 0 ? ", " : ""}
                <b>{l}</b>
              </span>
            ))}
            <span className="muted small"> ({c.examples.slice(0, 2).join(", ")}{c.files > 2 ? ", …" : ""})</span>
          </li>
        ))}
        {ok && report.conflicts.length === 0 && <li className="muted">No conflicts: no two mods have the same file.</li>}
      </ul>
      {needs.filter((n) => n.kind === "notDownloaded").length > 1 && (
        <p className="row gap-sm" style={{ margin: "4px 0 8px" }}>
          <button className="btn sm primary" onClick={() => onGetAll(needs.filter((n) => n.kind === "notDownloaded").map((n) => n.modId))}>
            Download all {needs.filter((n) => n.kind === "notDownloaded").length} missing
          </button>
          <span className="muted small">Their main files go into Downloads; when they're done, they're added to this profile in the right place.</span>
        </p>
      )}
      {report.conflicts.length > 0 && <p className="form-note">When two mods have the same file, the one lower in the list wins. Change the order to change which one.</p>}
      {report.loadOrder.length > 0 && (
        <details>
          <summary className="small muted">Plugin load order ({report.loadOrder.length})</summary>
          <ol className="mono small">
            {report.loadOrder.map((p) => (
              <li key={p}>{p}</li>
            ))}
          </ol>
        </details>
      )}
    </div>
  );
}

export function Profiles({ game, setGame, toast, libraryVersion, go, reloadGames, openAdd }: Ctx) {
  const [profiles, setProfiles] = useState<Profile[] | null>(null);
  const [games, setGames] = useState<Game[]>([]);
  const [mods, setMods] = useState<StoredMod[]>([]);
  const [openId, setOpenId] = useState<string | null>(null);
  const [checks, setChecks] = useState<Record<string, CheckReport>>({});
  const [making, setMaking] = useState(false);
  const [adding, setAdding] = useState(false);
  const [deleting, setDeleting] = useState<Profile | null>(null);
  const [busy, setBusy] = useState("");
  const [notes, setNotes] = useState<Record<string, string>>({});
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [noting, setNoting] = useState<{ modId: number; name: string } | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);
  useEffect(() => {
    api.notes().then(setNotes, () => {});
  }, []);

  const load = () => {
    api.profiles().then(setProfiles, (e) => toast((e as Error).message));
    api.games().then(setGames, () => {});
    api.profileChecks().then(setChecks, () => {});
  };
  useEffect(load, []); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    api.library().then(setMods, () => {});
  }, [libraryVersion]);

  const gameList = useMemo(() => {
    const ids = new Set([...games.map((g) => g.domain), ...(profiles ?? []).map((p) => p.game)]);
    return [...ids].map((d) => games.find((g) => g.domain === d) ?? ({ domain: d, name: d, source: "added" } as Game));
  }, [games, profiles]);
  // Only the picked game's profiles.
  const shown = (profiles ?? []).filter((p) => p.game === game);
  const open = shown.find((p) => p.id === openId) ?? shown[0] ?? null;
  const openGame = open ? gameList.find((g) => g.domain === open.game) : undefined;
  const modName = (id: number) => mods.find((m) => m.game === open?.game && m.modId === id)?.name;
  const fileName = (mid: number, fid: number) => mods.find((m) => m.game === open?.game && m.modId === mid)?.files.find((f) => f.fileId === fid)?.name;

  // The conflict check runs by itself whenever a profile's mods or order change (and when one is
  // opened without a result yet). It only reads files on this computer, never Nexus Mods.
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const [checking, setChecking] = useState<string | null>(null);
  const autoCheck = (id: string) => {
    clearTimeout(timer.current);
    timer.current = setTimeout(async () => {
      setChecking(id);
      // First what the mods need, put in and sorted (unless the profile is set to manual)...
      const res = await api.resolveProfile(id).catch(() => null);
      if (res) {
        setProfiles((all) => (all ?? []).map((x) => (x.id === res.profile.id ? res.profile : x)));
        const said = [
          res.added.length ? `Added ${res.added.join(", ")}` : "",
          res.switchedOn.length ? `switched on ${res.switchedOn.join(", ")}` : "",
          res.moved ? `moved ${plural(res.moved, "mod")} above what needs ${res.moved === 1 ? "it" : "them"}` : "",
        ].filter(Boolean);
        if (said.length) toast(`${said.join("; ")}.`.replace(/^./, (c) => c.toUpperCase()));
      }
      // ...then the check.
      const r = await api.checkProfile(id).catch(() => null);
      setChecking(null);
      if (r) setChecks((c) => ({ ...c, [id]: r }));
    }, 400);
  };
  useEffect(() => () => clearTimeout(timer.current), []);

  const save = async (p: Profile) => {
    try {
      const saved = await api.saveProfile(p);
      setProfiles((all) => (all ?? []).map((x) => (x.id === saved.id ? saved : x)));
      autoCheck(saved.id);
    } catch (e) {
      toast((e as Error).message);
    }
  };
  const run = async <T,>(label: string, f: () => Promise<T>, done?: (r: T) => string) => {
    setBusy(label);
    try {
      const r = await f();
      if (done) toast(done(r));
      return r;
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setBusy("");
      api.games().then(setGames, () => {});
      reloadGames();
    }
  };
  const appliedWords = (a: Applied) =>
    `${plural(a.mods, "mod")} in place (${plural(a.files, "file")}${a.plugins ? `, ${plural(a.plugins, "plugin")} on` : ""}${a.backedUp ? `, ${plural(a.backedUp, "game file")} kept aside` : ""})`;

  const move = (i: number, d: number) => {
    if (!open) return;
    const mods2 = [...open.mods];
    const [m] = mods2.splice(i, 1);
    mods2.splice(i + d, 0, m);
    void save({ ...open, mods: mods2 });
  };

  const check = async () => {
    if (!open) return;
    const r = await run("check", () => api.checkProfile(open.id));
    if (r) setChecks((c) => ({ ...c, [open.id]: r }));
  };
  const openId2 = open?.id;
  // Check when a profile is opened without a result, and again whenever a download lands in the
  // library (a collection's profile fills in as its files finish).
  const firstLib = useRef(libraryVersion);
  useEffect(() => {
    if (openId2 && !checks[openId2]) autoCheck(openId2);
  }, [openId2]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (libraryVersion !== firstLib.current && openId2) autoCheck(openId2);
  }, [libraryVersion]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="page">
      <div className="page-head">
        <h1>Profiles</h1>
        {isDesktop && (
          <button
            className="btn"
            onClick={async () => {
              const path = await pickFile("Open a mod pack");
              if (!path) return;
              const r = await run("import", () => api.importPack(path));
              if (r) {
                load();
                setGame(r.profile.game);
                setOpenId(r.profile.id);
                toast(r.queued ? `Opened “${r.profile.name}”; ${plural(r.queued, "file")} to download` : `Opened “${r.profile.name}”`);
              }
            }}
          >
            <FileUp size={15} /> Open mod pack…
          </button>
        )}
        <button className="btn primary" onClick={() => setMaking(true)} disabled={gameList.length === 0}>
          <Plus size={15} /> New profile…
        </button>
      </div>
      <p className="muted" style={{ marginTop: -8 }}>
        A profile is a set of mods for one game, in order. <b>Play</b> puts its mods in the game's folder (and takes the last profile's out), then starts the game. Your game's own files are kept aside and put back.
      </p>

      {profiles === null ? null : shown.length === 0 ? (
        <div className="empty">
          <Layers size={32} />
          <p>Make a profile for each way you like to play: pick mods from your library, check them for conflicts, then play. Adding a collection makes one for you.</p>
          <button className="btn primary" onClick={() => (gameList.length ? setMaking(true) : go("games"))}>
            {gameList.length ? "New profile…" : "Add a game first"}
          </button>
        </div>
      ) : (
        <div className="profiles">
          <div className="profile-list">
            {shown.map((p) => {
              const g = gameList.find((x) => x.domain === p.game);
              const live = g?.activeProfile === p.name;
              const problems = checks[p.id] ? problemCount(checks[p.id]) : 0;
              return (
                <button key={p.id} className={`profile-item${open?.id === p.id ? " active" : ""}`} onClick={() => setOpenId(p.id)}>
                  <Cover domain={p.game} name={g?.name ?? p.game} className="mini" />
                  <span className="grow">
                    <b className="ellipsis">{p.name}</b>
                    <span className="sub">
                      {(() => {
                        const pr = profileProgress(p, mods);
                        return pr.have < pr.total ? `${pr.have} of ${pr.total} mods downloaded` : `${g?.name ?? p.game} · ${plural(pr.total, "mod")}`;
                      })()}
                    </span>
                  </span>
                  {problems > 0 && (
                    <span className="pill warn" title="The conflict check found problems">
                      {problems} to fix
                    </span>
                  )}
                  {p.newDownloads && (
                    <span className="pill" title="New downloads for this game go into this profile">
                      New
                    </span>
                  )}
                  {live && (
                    <span className="pill ok" title="This profile's mods are in the game's folder now, so they load whenever the game starts">
                      Active
                    </span>
                  )}
                </button>
              );
            })}
          </div>

          {open && (
            <div className="profile-detail">
              <Hero domain={open.game} name={openGame?.name ?? open.game}>
                <div className="row gap wrap hero-actions">
                  <input className="profile-name" value={open.name} aria-label="Profile name" onChange={(e) => setProfiles((all) => (all ?? []).map((x) => (x.id === open.id ? { ...x, name: e.target.value } : x)))} onBlur={() => save(open)} />
                  <span className="grow" />
                  <button className="btn" disabled={!!busy} onClick={check}>
                    <ShieldCheck size={15} /> {busy === "check" || checking === open.id ? "Checking…" : checks[open.id] ? "Check again" : "Check for conflicts"}
                  </button>
                  <button className="btn primary" disabled={!!busy} onClick={() => run("play", () => api.play(open.id), (a) => `${appliedWords(a)}. Starting the game.`)}>
                    <Play size={15} /> {busy === "play" ? "Putting mods in…" : "Play"}
                  </button>
                </div>
              </Hero>

              <div className="toolbar">
                <button className="btn" onClick={() => setAdding(true)}>
                  <Plus size={15} /> Add mods…
                </button>
                <label className="check" title="Puts in what the profile's mods need (from your library), switches it on, and loads it before the mods needing it. Your own order is kept otherwise.">
                  <input type="checkbox" checked={!open.manualRequirements} onChange={(e) => void save({ ...open, manualRequirements: !e.target.checked })} /> Handle requirements
                </label>
                <label className="check" title="Every finished download for this game goes into this profile, with what it needs. One profile per game can do this.">
                  <input
                    type="checkbox"
                    checked={!!open.newDownloads}
                    onChange={async (e) => {
                      const on = e.target.checked;
                      // Only one profile per game takes new downloads.
                      if (on) for (const o of (profiles ?? []).filter((x) => x.game === open.game && x.id !== open.id && x.newDownloads)) await save({ ...o, newDownloads: false });
                      await save({ ...open, newDownloads: on });
                    }}
                  />{" "}
                  New downloads go here
                </label>
                <button className="btn" disabled={!!busy} onClick={() => run("apply", () => api.applyProfile(open.id), appliedWords)} title="Put the mods in without starting the game">
                  <Upload size={15} /> {busy === "apply" ? "Putting mods in…" : "Put mods in game"}
                </button>
                {openGame?.activeProfile && (
                  <button className="btn" disabled={!!busy} onClick={() => run("clear", () => api.clearGame(open.game), () => `Took the mods out of ${openGame.name}; its own files are back`)}>
                    <RotateCcw size={15} /> Take mods out
                  </button>
                )}
                <button
                  className="btn"
                  disabled={!!busy}
                  title="Looks for newer versions of this profile's files (one request per mod)"
                  onClick={async () => {
                    const r = await run("updates", () => api.profileUpdates(open.id));
                    if (!r) return;
                    const newer = r.updates.filter((u) => u.newer);
                    if (!newer.length) return toast(`Everything in “${open.name}” is up to date`);
                    for (const u of newer) await api.add(u.game, u.modId, u.newer!.fileId).catch(() => {});
                    // When each finishes, it takes the old file's place in this profile.
                    rememberUpdates(open.id, newer.map((u) => ({ modId: u.modId, fileId: u.newer!.fileId })));
                    toast(`${plural(newer.length, "update")} added to downloads; each takes the old file's place here when it's done`);
                  }}
                >
                  <RefreshCw size={15} className={busy === "updates" ? "spin" : ""} /> {busy === "updates" ? "Checking…" : "Update this profile"}
                </button>
                <button
                  className="btn ghost"
                  title="A copy of this profile to try changes on"
                  onClick={async () => {
                    const copy = await api.saveProfile({ ...open, id: "", name: `${open.name} (copy)`, newDownloads: false }).catch((e) => toast((e as Error).message));
                    if (copy) {
                      setProfiles((all) => [...(all ?? []), copy]);
                      setOpenId(copy.id);
                      toast(`Made “${copy.name}”`);
                    }
                  }}
                >
                  <Copy size={15} /> Duplicate
                </button>
                <span className="grow" />
                {isDesktop && (
                  <button
                    className="btn ghost"
                    onClick={async () => {
                      const path = await pickSave("Save mod pack", `${open.name}.json`);
                      if (path) await run("export", () => api.exportPack(open.id, path), () => "Saved the mod pack. Share it so others can get the same mods.");
                    }}
                  >
                    <FileDown size={15} /> Save as mod pack…
                  </button>
                )}
                <button className="icon-btn danger" title="Delete profile" aria-label="Delete profile" onClick={() => setDeleting(open)}>
                  <Trash2 size={15} />
                </button>
              </div>

              {(() => {
                const pr = profileProgress(open, mods);
                if (pr.have >= pr.total) return null;
                return (
                  <div className="panel banner-panel">
                    <div className="grow disk">
                      <b>
                        {pr.have} of {pr.total} mods downloaded
                      </b>
                      <div className="disk-bar" role="meter" aria-valuemin={0} aria-valuemax={pr.total} aria-valuenow={pr.have} aria-label="Mods downloaded">
                        <span style={{ width: `${(pr.have / pr.total) * 100}%` }} />
                      </div>
                      <span className="muted small">The conflict check runs again as each one lands, and you'll get a message when they're all here. Missing mods are left out if you play before then.</span>
                    </div>
                    <button className="btn" onClick={() => go("downloads")}>
                      Go to Downloads
                    </button>
                  </div>
                );
              })()}
              {checks[open.id] && (
                <CheckPanel
                  report={checks[open.id]}
                  gameName={openGame?.name ?? open.game}
                  onGet={(modId) => openAdd(modUrl(open.game, modId))}
                  onAdd={(modId) => {
                    const m = mods.find((x) => x.game === open.game && x.modId === modId);
                    const f = m && [...m.files].sort((a, b) => b.downloadedAt - a.downloadedAt)[0];
                    if (m && f) void save({ ...open, mods: [...open.mods, { modId, fileId: f.fileId, enabled: true }] }).then(() => toast(`Added “${m.name}” to “${open.name}”`));
                  }}
                  onSwitchOn={(modId) => void save({ ...open, mods: open.mods.map((x) => (x.modId === modId ? { ...x, enabled: true } : x)) })}
                  onGetAll={(ids) =>
                    void api.queueRequirements(open.game, ids).then(
                      (r) => toast(`Added ${plural(r.queued.length, "file")} to downloads${r.skipped.length ? `; couldn't add ${r.skipped.join(", ")}` : ""}`),
                      (e) => toast((e as Error).message),
                    )
                  }
                />
              )}

              {open.mods.length === 0 ? (
                <div className="empty">
                  <p>No mods in this profile yet.</p>
                  <button className="btn primary" onClick={() => setAdding(true)}>
                    Add mods…
                  </button>
                </div>
              ) : (
                <div className="table-wrap">
                  <table className="table">
                    <thead>
                      <tr>
                        <th style={{ width: 34 }} className="r">
                          #
                        </th>
                        <th style={{ width: 30 }} />
                        <th>Mod</th>
                        <th />
                      </tr>
                    </thead>
                    <tbody>
                      {open.mods.map((m, i) => {
                        const name = modName(m.modId);
                        return (
                          <tr
                            key={`${m.modId}:${m.fileId}`}
                            className={`${dragFrom === i ? "dragging" : ""}${dragOver === i && dragFrom !== null && dragFrom !== i ? (dragFrom < i ? " drop-below" : " drop-above") : ""}`}
                            style={m.enabled ? undefined : { opacity: 0.55 }}
                            draggable
                            onDragStart={(e) => {
                              setDragFrom(i);
                              e.dataTransfer.effectAllowed = "move";
                            }}
                            onDragOver={(e) => {
                              e.preventDefault();
                              setDragOver(i);
                            }}
                            onDragEnd={() => {
                              setDragFrom(null);
                              setDragOver(null);
                            }}
                            onDrop={(e) => {
                              e.preventDefault();
                              if (dragFrom !== null && dragFrom !== i) move(dragFrom, i - dragFrom);
                              setDragFrom(null);
                              setDragOver(null);
                            }}
                          >
                            <td className="r muted drag-handle" title="Drag to move">
                              <GripVertical size={13} /> {i + 1}
                            </td>
                            <td>
                              <input type="checkbox" aria-label="Use this mod" checked={m.enabled} onChange={(e) => save({ ...open, mods: open.mods.map((x, j) => (j === i ? { ...x, enabled: e.target.checked } : x)) })} />
                            </td>
                            <td>
                              <div className="name">{name ?? `Mod ${m.modId}`}</div>
                              <div className="sub">{name ? fileName(m.modId, m.fileId) ?? `File ${m.fileId} (not downloaded)` : "Not downloaded yet"}</div>
                              {notes[`${open.game}:${m.modId}`] && <div className="note">“{notes[`${open.game}:${m.modId}`]}”</div>}
                            </td>
                            <td className="actions">
                              <button
                                className="icon-btn"
                                aria-label="Note"
                                title={notes[`${open.game}:${m.modId}`] ? "Change your note" : "Add a note about this mod"}
                                onClick={() => setNoting({ modId: m.modId, name: name ?? `Mod ${m.modId}` })}
                              >
                                <StickyNote size={15} />
                              </button>
                              <button className="icon-btn" aria-label="Load earlier" title="Load earlier" disabled={i === 0} onClick={() => move(i, -1)}>
                                <ArrowUp size={15} />
                              </button>
                              <button className="icon-btn" aria-label="Load later" title="Load later (wins conflicts)" disabled={i === open.mods.length - 1} onClick={() => move(i, 1)}>
                                <ArrowDown size={15} />
                              </button>
                              <button className="icon-btn danger" aria-label="Take out of profile" title="Take out of this profile" onClick={() => save({ ...open, mods: open.mods.filter((_, j) => j !== i) })}>
                                <Trash2 size={15} />
                              </button>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
              )}
              {openGame?.activeProfile && openGame.activeProfile !== open.name && (
                <p className="form-note">
                  <Check size={12} /> “{openGame.activeProfile}” is the active profile: its mods are in the game's folder now. Playing this profile swaps them.
                </p>
              )}
            </div>
          )}
        </div>
      )}

      {noting && open && (
        <NoteDialog
          game={open.game}
          modId={noting.modId}
          modName={noting.name}
          note={notes[`${open.game}:${noting.modId}`] ?? ""}
          onClose={() => setNoting(null)}
          onSaved={(t) => {
            const k = `${open.game}:${noting.modId}`;
            setNotes((n) => {
              const next = { ...n };
              if (t) next[k] = t;
              else delete next[k];
              return next;
            });
          }}
        />
      )}
      {making && (
        <NewProfile
          games={gameList}
          game={game}
          onClose={() => setMaking(false)}
          onMade={(p) => {
            setProfiles((all) => [...(all ?? []), p]);
            setGame(p.game);
            setOpenId(p.id);
          }}
        />
      )}
      {adding && open && <AddMods profile={open} mods={mods} onClose={() => setAdding(false)} onAdd={(picked) => save({ ...open, mods: [...open.mods, ...picked.map((p) => ({ ...p, enabled: true }))] })} />}
      {deleting && (
        <Confirm
          title="Delete this profile?"
          body={`Delete the profile “${deleting.name}”? Its mods stay in the library${games.find((g) => g.domain === deleting.game)?.activeProfile === deleting.name ? ", and stay in the game until you take them out or play another profile" : ""}.`}
          action="Delete"
          onConfirm={async () => {
            await api.deleteProfile(deleting.id);
            setOpenId(null);
            load();
          }}
          onClose={() => setDeleting(null)}
        />
      )}
    </div>
  );
}
