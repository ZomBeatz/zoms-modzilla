import { useEffect, useMemo, useRef, useState } from "react";
import { AlertTriangle, Bot, Merge, CirclePlay, FolderInput, SlidersHorizontal, ArrowDown, ClipboardPaste, Share2, SearchCheck, ArrowDownToLine, Columns2, History as HistoryIcon, X, ArrowUp, Check, ChevronDown, ChevronRight, Copy, FileDown, FileUp, GripVertical, Layers, Play, Plus, RefreshCw, RotateCcw, Search, ShieldCheck, StickyNote, Trash2, Upload, Wand2 } from "lucide-react";
import { rememberUpdates } from "../progress";
import { NoteDialog } from "./NoteDialog";
import { api, isDesktop, modUrl, openUrl, pickFile, pickSave, type Applied, type CheckReport, type Game, type PlayTime, type Profile, type StoredMod, type Update, on, playWithoutMods } from "../api";
import type { Ctx } from "../App";
import { Cover, Hero } from "../art";
import { plural, when } from "../format";
import { autoFix, profileProgress } from "../progress";
import { factKey, isAdult, isGone, needersOf, softNote, useFacts } from "../facts";
import { onWantProfile, rememberProfile, rememberRecent, rememberedProfile, setTriedOff, takeWantedProfile, triedOff } from "../nav";
import { usePlayLabel } from "../GameStatus";
import { bytes } from "../format";
import { Confirm, Dialog } from "../ui";
import { CollectionCard } from "./CollectionCard";
import { FomodDialog } from "./FomodDialog";
import { SmapiPanel } from "./SmapiPanel";
import { RunPanel, protonTip, useLastRun } from "./RunPanel";
import { ModSettings } from "./ModSettings";
import { merge } from "../merge";
import { TsUpdates } from "./TsUpdates";
import { AiChat } from "./AiChat";
import { Mo2Import } from "./Mo2Import";
import { R2Import } from "./R2Import";
import { requestPlay } from "../SavesGate";
import { TagChips, tagsIn, useTags } from "../tags";
import { answer as halvingAnswer, halvingOf, roundMods, setHalving, start as startHalving, type Halving } from "../halving";
import { CompareDialog, HistoryDialog, RuleDialog, UpdatesDialog } from "./ProfileExtras";

function NewProfile({ games, game, onClose, onMade, onCollection }: { games: Game[]; game: string | null; onClose: () => void; onMade: (p: Profile) => void; onCollection: (address: string) => void }) {
  const [from, setFrom] = useState<"empty" | "collection">("empty");
  const [address, setAddress] = useState("");
  const [name, setName] = useState("");
  // Profiles are made for the game picked in the sidebar.
  const g = game ?? "";
  const gameName = games.find((x) => x.domain === g)?.name ?? g;
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
      title={`New ${gameName} profile`}
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          {from === "empty" ? (
            <button className="btn primary" disabled={!name.trim() || !g} onClick={save}>
              Make profile
            </button>
          ) : (
            <button className="btn primary" disabled={!address.includes("/collections/")} onClick={() => onCollection(address.trim())}>
              Look it up
            </button>
          )}
        </>
      }
    >
      <div className="seg" style={{ marginBottom: 14 }}>
        <button className={from === "empty" ? "active" : ""} onClick={() => setFrom("empty")}>
          Empty
        </button>
        <button className={from === "collection" ? "active" : ""} onClick={() => setFrom("collection")}>
          From a collection
        </button>
      </div>
      {from === "collection" ? (
        <div className="form-grid">
          <label className="field full">
            <span>Collection address</span>
            <input autoFocus placeholder="https://www.nexusmods.com/games/…/collections/…" value={address} onChange={(e) => setAddress(e.target.value)} onKeyDown={(e) => e.key === "Enter" && address.includes("/collections/") && onCollection(address.trim())} />
          </label>
          <p className="muted small full" style={{ margin: 0 }}>
            Copy it from the collection's page on Nexus Mods. You'll see what's in it before anything downloads. The profile keeps the collection apart from your own changes, so you can update it later or add add-ons on top. Got an add-on pack from someone? Use <b>Open mod pack…</b>.
          </p>
        </div>
      ) : (
      <div className="form-grid">
        <label className="field full">
          <span>Name</span>
          <input autoFocus placeholder="Winter playthrough" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && name.trim() && void save()} />
        </label>
        {error && <span className="err full">{error}</span>}
      </div>
      )}
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

/** Problems that stop mods working: missing plugin masters and missing requirements. (Mods
 * still downloading and other warnings aren't counted: they don't stop the game.) */
export function problemCount(r: CheckReport) {
  return r.plugins.length + (r.launchFix ? 1 : 0) + (r.loadersMissing?.length ?? 0) + (r.misplaced?.length ?? 0) + (r.gameLog && !r.gameLog.beforeChange ? r.gameLog.issues.length : 0) + (r.needs ?? []).filter((n) => ["notDownloaded", "notInProfile", "switchedOff"].includes(n.kind)).length;
}

const day = (t?: number | null) => (t ? new Date(t * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" }) : "");

/** Play, showing where the game is: putting mods in, starting, playing (with a timer), or what went wrong. */
function PlayButton({ state, disabled, putting, onPlay }: { state?: import("../api").GameState; disabled: boolean; putting: boolean; onPlay: () => void }) {
  const p = usePlayLabel(state);
  const label = putting ? "Putting mods in…" : p.label ?? "Play";
  const cls = p.tone === "ok" ? "btn playing" : p.tone === "bad" ? "btn play-bad" : "btn primary";
  return (
    <button className={cls} disabled={disabled || p.busy} onClick={onPlay} title={p.tone === "bad" ? `${state?.detail ?? label}. Click to try again.` : p.busy ? "The game is starting or running" : undefined}>
      {p.tone === "busy" || putting ? <RefreshCw size={15} className="spin" /> : <Play size={15} />} {label}
    </button>
  );
}

const hours = (s: number) => (s >= 3600 ? `${(s / 3600).toFixed(s >= 36000 ? 0 : 1)} h` : `${Math.max(1, Math.round(s / 60))} min`);

/** A profile at a glance: what's in it, how big, and how much it's been played. */
function ProfileStats({ profile, mods, facts, play, report }: { profile: Profile; mods: StoredMod[]; facts: Record<string, import("../api").ModFacts>; play?: PlayTime; report?: CheckReport }) {
  const lib = (id: number) => mods.find((m) => m.game === profile.game && m.modId === id);
  const on = profile.mods.filter((m) => m.enabled);
  const size = on.reduce((n, m) => n + (lib(m.modId)?.files.find((f) => f.fileId === m.fileId)?.size ?? 0), 0);
  const adult = on.filter((m) => isAdult(facts, profile.game, m.modId, lib(m.modId)?.adult)).length;
  const cats = new Map<string, number>();
  for (const m of on) {
    const c = facts[factKey(profile.game, m.modId)]?.category;
    if (c) cats.set(c, (cats.get(c) ?? 0) + 1);
  }
  const top = [...cats].sort((a, b) => b[1] - a[1]).slice(0, 3);
  const problems = report ? problemCount(report) : null;
  return (
    <div className="stats">
      <div className="panel stat" title={`${on.length} switched on, ${profile.mods.length - on.length} off`}>
        <b>{on.length}</b>
        <span>mods on{profile.mods.length > on.length ? ` · ${profile.mods.length - on.length} off` : ""}</span>
      </div>
      <div className="panel stat" title="The mods' downloaded files (switched on)">
        <b>{bytes(size)}</b>
        <span>of mod files</span>
      </div>
      <div className="panel stat" title={play?.lastPlayed ? `Last played ${when(play.lastPlayed)}` : "Counted when you start the game with Play, while it runs"}>
        <b>{play?.seconds ? hours(play.seconds) : "—"}</b>
        <span>{play?.sessions ? `played · ${play.sessions} ${play.sessions === 1 ? "time" : "times"}` : "not played from here yet"}</span>
      </div>
      <div className="panel stat" title={play?.lastPlayed ? new Date(play.lastPlayed * 1000).toLocaleString() : undefined}>
        <b>{play?.lastPlayed ? when(play.lastPlayed) : "—"}</b>
        <span>last played</span>
      </div>
      <div className="panel stat">
        <b style={{ color: problems ? "var(--zb-danger)" : problems === 0 ? "var(--zb-ok)" : undefined }}>{problems === null ? "—" : problems === 0 ? "Ready" : problems}</b>
        <span>{problems ? (problems === 1 ? "problem to fix" : "problems to fix") : problems === 0 ? "nothing to fix" : "not checked yet"}</span>
      </div>
      {(top.length > 0 || adult > 0) && (
        <div className="panel stat" title={[...cats].map(([c, n]) => `${c}: ${n}`).join("\n")}>
          <b className="ellipsis" style={{ fontSize: 14, paddingTop: 3 }}>{top.map(([c]) => c).join(", ") || "—"}</b>
          <span>top categories{adult ? ` · ${adult} adult` : ""}</span>
        </div>
      )}
    </div>
  );
}

/** A section of the check: a one-line summary that opens to the details. */
function CheckGroup({ tone, count, title, open, onToggle, children }: { tone: "err" | "warn" | "info"; count: number; title: string; open: boolean; onToggle: () => void; children: React.ReactNode }) {
  if (!count) return null;
  return (
    <div className={`check-group ${tone}`}>
      <button className="check-sum" aria-expanded={open} onClick={onToggle}>
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
        <span className={`pill ${tone === "err" ? "danger" : tone === "warn" ? "warn" : ""}`}>{count}</span>
        <span className="grow">{title}</span>
        <span className="muted small">{open ? "Hide" : "Show"}</span>
      </button>
      {open && <ul className="check-list">{children}</ul>}
    </div>
  );
}

/** The conflict check as a summary card: a line per kind of problem, each opening to the details. */
function CheckPanel({ report, gameName, gameDomain, onCopied, onSwitchOff, onUpdate, onApply, onTry, tried, onUndoTry, canTry, checking, fixable, onFix, onGet, onAdd, onSwitchOn, onGetAll }: { report: CheckReport; gameName: string; gameDomain: string; onCopied: () => void; onSwitchOff: (modId: number) => void; onUpdate: (modId: number, name: string) => Promise<string>; onApply: () => void; onTry: (modIds: number[]) => Promise<void>; tried: { names: string[]; at: number } | null; onUndoTry: () => void; canTry: (modId: number) => boolean; checking: boolean; fixable: number; onFix: () => void; onGet: (modId: number) => void; onAdd: (modId: number) => void; onSwitchOn: (modId: number) => void; onGetAll: (modIds: number[]) => void }) {
  const ok = problemCount(report) === 0;
  const needs = report.needs ?? [];
  const compat = report.compat ?? [];
  const by = (n: { neededBy: string[] }) => (n.neededBy.length > 2 ? `${n.neededBy.slice(0, 2).join(", ")} and ${n.neededBy.length - 2} more` : n.neededBy.join(" and "));
  const fixCount = needs.filter((n) => ["notDownloaded", "notInProfile", "switchedOff"].includes(n.kind)).length + report.plugins.length + (report.launchFix ? 1 : 0) + (report.loadersMissing?.length ?? 0) + (report.misplaced?.length ?? 0) + (report.gameLog && !report.gameLog.beforeChange ? report.gameLog.issues.length : 0);
  const oldLog = report.gameLog?.beforeChange ? report.gameLog.issues.length : 0;
  const warnCount = needs.filter((n) => !["notDownloaded", "notInProfile", "switchedOff"].includes(n.kind)).length + compat.length + (report.programs ?? []).length + report.missing.length + report.installers.length + oldLog + (report.gameLog?.assets?.length ?? 0);
  // Problems open by themselves when there are few; warnings and conflicts start closed.
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const isOpen = (k: string, byDefault: boolean) => open[k] ?? byDefault;
  const toggle = (k: string, byDefault: boolean) => setOpen((o) => ({ ...o, [k]: !(o[k] ?? byDefault) }));
  const missingReqs = needs.filter((n) => n.kind === "notDownloaded");
  const [trying, setTrying] = useState(false);
  // Crash suspects: mods with problems during play just before it. At most three are offered to
  // switch off, and never one other mods need (that would take a lot more with it).
  const near = (report.gameLog?.assets ?? []).filter((a) => a.nearCrash);
  const suspects = near.filter((a) => canTry(a.modId)).slice(0, 3);
  const heldBack = near.filter((a) => !canTry(a.modId)).slice(0, 3);
  // The update button's progress and answer, kept in the card.
  const [updating, setUpdating] = useState<Record<number, string>>({});
  const logRows = report.gameLog?.issues.map((i) => (
          <li key={`log${i.modId}${i.kind}${i.otherId ?? ""}`} className={`check-row ${report.gameLog?.beforeChange ? "warn-text" : "err-text"}`}>
            <span className="grow">
              <b>{i.modName}</b>{" "}
              {i.kind === "oldPlugin" ? "was made for another version of the game, so it doesn't load." : i.kind === "needsNewer" ? <>doesn't work with your <b>{i.otherName}</b>.</> : i.kind === "loaderErrors" ? "had errors while the game ran." : "didn't load."} <span className="muted">{i.detail} (The game said so {when(report.gameLog!.at)}.)</span>
              {(i.kind === "oldPlugin" || i.kind === "loaderRefused") && <span className="muted"> Switch it off (and what needs it) until its author updates it.</span>}
              {i.otherId && updating[i.otherId] && updating[i.otherId] !== "…" && <span className="inline-answer">{updating[i.otherId]}</span>}
            </span>
            {i.kind === "needsNewer" && i.otherId && (
              <button
                className="btn sm primary"
                disabled={updating[i.otherId] === "…"}
                onClick={async () => {
                  setUpdating((u) => ({ ...u, [i.otherId!]: "…" }));
                  const said = await onUpdate(i.otherId!, i.otherName ?? "");
                  setUpdating((u) => ({ ...u, [i.otherId!]: said }));
                }}
                title="Looks for its newest main file on Nexus Mods; if yours is older, downloads it, and it takes the old file's place in this profile"
              >
                {updating[i.otherId] === "…" ? <><RefreshCw size={13} className="spin" /> Checking…</> : `Update ${i.otherName}`}
              </button>
            )}
            <button className="btn sm" onClick={() => onSwitchOff(i.modId)}>
              Switch off
            </button>
            <button className="btn sm ghost" onClick={() => void openUrl(modUrl(gameDomain, i.modId))}>
              Open page
            </button>
          </li>
        ));

  return (
    <div className="panel check-panel">
      <div className="check-head">
        {ok ? <ShieldCheck size={18} color="var(--zb-ok)" /> : <AlertTriangle size={18} color="var(--zb-warn)" />}
        <div className="grow">
          <b>{ok ? (warnCount ? "Ready to play, with warnings" : "Ready to play") : `${plural(fixCount, "problem")} to fix before playing`}</b>
          <div className="muted small">
            {[warnCount && plural(warnCount, "warning"), report.conflicts.length && `${plural(report.conflicts.length, "overlap")} between mods`].filter(Boolean).join(" · ") || "No conflicts: no two mods have the same file."}
            {" · "}
            {checking ? "checking again…" : `checked ${when(report.at)}`}
          </div>
        </div>
        {fixable > 0 && (
          <button className="btn sm primary" onClick={onFix} title="Switches on and adds requirements from your library, points mods at files you have, takes out doubles, and downloads missing requirements">
            <Wand2 size={13} /> Fix for me
          </button>
        )}
      </div>
      {report.outOfDate && (
        <div className="out-of-date">
          <RefreshCw size={15} />
          <span className="grow">
            <b>The game has an older version of this profile.</b> You changed it after its mods last went into the game{report.gameLog ? ", so what the game said below may already be fixed" : ""}. Put the mods in again (or press Play) to bring the game up to date.
          </span>
          <button className="btn sm primary" onClick={onApply}>
            Put mods in game
          </button>
        </div>
      )}
      {report.gameLog?.crash && (
        <div className="crash-box">
          <AlertTriangle size={16} />
          <div className="grow">
            <b>
              {gameName} crashed {when(report.gameLog.crash.at)}, {report.gameLog.crash.minutes} min in{report.gameLog.beforeChange ? " (before your last change)" : ""}.
            </b>
            <div className="small">
              {report.gameLog.crash.outOfMemory
                ? "It ran out of memory: too many or too large texture and outfit mods, or a memory leak in one."
                : report.gameLog.crash.inGameCode
                  ? "It happened in the game's own code, not a mod's program: usually something a mod loaded (an outfit, a mesh, an appearance)."
                  : "A crash reporter recorded where it happened."}
              {report.gameLog.crash.place && <span className="muted"> ({report.gameLog.crash.place})</span>}{" "}
              {suspects.length + heldBack.length > 0
                ? <>Mods that had problems while you played, in the five minutes before: <b>{[...suspects, ...heldBack].map((a) => a.modName).join(", ")}</b>.{heldBack.length > 0 && <> Other mods need {heldBack.map((a) => a.modName).join(", ")}, so {heldBack.length === 1 ? "it isn't" : "they aren't"} switched off for you.</>}</>
                : "No mod reported a problem while you played before it, so the logs can't name one. Think about what you were doing then, and test by switching off the mods for that (outfits, character creator, vehicles), half at a time."}
            </div>
            {tried && tried.at && report.gameLog.crash.at > tried.at ? (
              <div className="crash-actions">
                <span className="small">
                  <b>It crashed again without {tried.names.join(", ")}</b>, so {tried.names.length === 1 ? "it wasn't" : "they weren't"} the cause. Switch {tried.names.length === 1 ? "it" : "them"} back on.
                </span>
                <button className="btn sm primary" onClick={onUndoTry}>
                  Switch {tried.names.length === 1 ? "it" : "them"} back on
                </button>
              </div>
            ) : tried ? (
              <div className="crash-actions">
                <span className="small">
                  Testing without <b>{tried.names.join(", ")}</b>. Play and do the same thing again: if it doesn't crash, one of them was the cause.
                </span>
                <button className="btn sm" onClick={onUndoTry}>
                  Switch them back on
                </button>
              </div>
            ) : (
              !report.gameLog.crash.outOfMemory &&
              suspects.length > 0 && (
                <div className="crash-actions">
                  <button
                    className="btn sm primary"
                    disabled={trying}
                    onClick={async () => {
                      setTrying(true);
                      await onTry(suspects.map((a) => a.modId));
                      setTrying(false);
                    }}
                  >
                    <Wand2 size={13} /> {trying ? "Working…" : `Test without ${suspects.length === 1 ? suspects[0].modName : `these ${suspects.length}`}`}
                  </button>
                  <span className="small muted">Switches {suspects.length === 1 ? "it" : "them"} off and puts the mods in, ready to test. Nothing else changes; switch {suspects.length === 1 ? "it" : "them"} back on here anytime.</span>
                </div>
              )
            )}
          </div>
        </div>
      )}
      <CheckGroup tone="err" count={fixCount} title={fixCount === 1 ? "Problem that stops a mod working" : "Problems that stop mods working"} open={isOpen("fix", fixCount <= 4)} onToggle={() => toggle("fix", fixCount <= 4)}>
        {report.gameLog && !report.gameLog.beforeChange && logRows}
        {(report.loadersMissing ?? []).map((l) => (
          <li key={`loader${l.loader}`} className="check-row err-text">
            <span className="grow">
              <b>{l.mods.length > 2 ? `${l.mods.slice(0, 2).join(", ")} and ${l.mods.length - 2} more` : l.mods.join(" and ")}</b> {l.mods.length === 1 ? "needs" : "need"} <b>{l.loader}</b>, which isn't in {gameName} or this profile, so {l.mods.length === 1 ? "it won't" : "they won't"} load.{" "}
              <span className="muted">Add {l.loader} for {gameName} to this profile (from Nexus Mods, or its own site when Nexus Mods doesn't have it).</span>
            </span>
            <button className="btn sm" onClick={() => void openUrl(`https://www.nexusmods.com/games/${encodeURIComponent(gameDomain)}/mods?keyword=${encodeURIComponent(l.loader.replace(/^an? /, ""))}`)}>
              Look for it
            </button>
          </li>
        ))}
        {(report.misplaced ?? []).map((m) => (
          <li key={`placed${m.modId}`} className="check-row err-text">
            <span className="grow">
              <b>{m.modName}</b> has its files inside an extra folder, so they'd land in <code>{m.lands}</code> instead of <code>{m.should}</code>, where the game looks.{" "}
              <span className="muted">Its page usually says which file to use, or whether it has an installer. Switch it off until then.</span>
            </span>
            <button className="btn sm" onClick={() => onSwitchOff(m.modId)}>
              Switch off
            </button>
            <button className="btn sm ghost" onClick={() => void openUrl(modUrl(gameDomain, m.modId))}>
              Open page
            </button>
          </li>
        ))}
        {report.launchFix && (
          <li className="check-row err-text">
            <span className="grow">
              Mod loaders here won't run: Proton uses its own <b>{report.launchFix.missing.map((d) => `${d}.dll`).join(", ")}</b> unless {gameName}'s Steam launch options say otherwise, so the mods that need {report.launchFix.missing.length === 1 ? "it" : "them"} fail (for Cyberpunk, RED4ext's winmm.dll loads Codeware, TweakXL and ArchiveXL). In Steam: right-click {gameName} › Properties › Launch Options, and paste:
              <code className="launch-line">{report.launchFix.suggested}</code>
            </span>
            <button className="btn sm" onClick={() => void navigator.clipboard.writeText(report.launchFix!.suggested).then(() => onCopied())}>
              Copy
            </button>
          </li>
        )}
        {needs.filter((n) => ["notDownloaded", "notInProfile", "switchedOff"].includes(n.kind)).map((n) => (
          <li key={n.kind + n.modId + n.name} className={`check-row ${["notDownloaded", "notInProfile", "switchedOff"].includes(n.kind) ? "err-text" : "warn-text"}`}>
            <span className="grow">
              {n.kind === "dlc" ? (
                <>
                  {by(n)} {n.neededBy.length > 1 ? "need" : "needs"} the <b>{n.name}</b> expansion.
                </>
              ) : (
                <>
                  {by(n)} {n.kind === "optional" ? (n.neededBy.length > 1 ? "suggest" : "suggests") : n.neededBy.length > 1 ? "need" : "needs"} <b>{n.name}</b>
                  {n.kind === "optional" && " (its author says it's optional), which you don't have on."}
                  {n.kind === "notDownloaded" && ", which isn't downloaded."}
                  {n.kind === "notInProfile" && ", which is in your library but not in this profile."}
                  {n.kind === "switchedOff" && ", which is switched off in this profile."}
                  {n.kind === "external" && " from outside Nexus Mods. Zom's Nexus can't check this one; get it from its page."}
                  {n.kind === "otherGame" && " (a tool on Nexus Mods). Zom's Nexus can't check this one; see its page."}
                  {n.kind === "unavailable" && ", which its author took down on Nexus Mods (or it was removed), so it can't be downloaded. Check its page for a replacement; without it, the mod may not work."}
                </>
              )}
              {n.notes && <span className="muted"> Note from the author: “{n.notes}”</span>}
            </span>
            {(n.kind === "notDownloaded" || (n.kind === "optional" && n.modId > 0)) && (
              <button className="btn sm" onClick={() => onGet(n.modId)}>
                Get…
              </button>
            )}
            {n.kind === "unavailable" && (
              <button className="btn sm ghost" onClick={() => void openUrl(modUrl(gameDomain, n.modId))}>
                Open page
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
        {missingReqs.length > 1 && (
          <li className="check-row">
            <button className="btn sm primary" onClick={() => onGetAll(missingReqs.map((n) => n.modId))}>
              Download all {missingReqs.length} missing
            </button>
            <span className="muted small">Their main files go into Downloads; when they're done, they're added to this profile in the right place.</span>
          </li>
        )}
      </CheckGroup>
      <CheckGroup tone="warn" count={warnCount} title={warnCount === 1 ? "Warning" : "Warnings: worth a look, won't stop the game"} open={isOpen("warn", oldLog > 0)} onToggle={() => toggle("warn", oldLog > 0)}>
        {oldLog > 0 && (
          <li className="check-row muted">
            <span className="grow">
              From the game's last start ({when(report.gameLog!.at)}), <b>before you last put the mods in</b>. Some of this may be fixed now; play to see. Mods you've switched off since aren't listed.
            </span>
          </li>
        )}
        {oldLog > 0 && logRows}
        {(report.gameLog?.assets ?? []).map((a) => (
          <li key={`asset${a.modId}`} className="check-row warn-text">
            <span className="grow">
              {a.nearCrash && <span className="pill danger tiny-pill" style={{ marginLeft: 0, marginRight: 6 }}>Before the crash</span>}
              <b>{a.modName}</b>: the game reported {a.count === 1 ? "a problem" : `${a.count} problems`} with its files <span className="muted">(e.g. {a.sample})</span>
            </span>
            <button className="btn sm" onClick={() => onSwitchOff(a.modId)}>
              Switch off
            </button>
            <button className="btn sm ghost" onClick={() => void openUrl(modUrl(gameDomain, a.modId))}>
              Open page
            </button>
          </li>
        ))}
        {needs.filter((n) => !["notDownloaded", "notInProfile", "switchedOff"].includes(n.kind)).map((n) => (
          <li key={n.kind + n.modId + n.name} className={`check-row ${["notDownloaded", "notInProfile", "switchedOff"].includes(n.kind) ? "err-text" : "warn-text"}`}>
            <span className="grow">
              {n.kind === "dlc" ? (
                <>
                  {by(n)} {n.neededBy.length > 1 ? "need" : "needs"} the <b>{n.name}</b> expansion.
                </>
              ) : (
                <>
                  {by(n)} {n.kind === "optional" ? (n.neededBy.length > 1 ? "suggest" : "suggests") : n.neededBy.length > 1 ? "need" : "needs"} <b>{n.name}</b>
                  {n.kind === "optional" && " (its author says it's optional), which you don't have on."}
                  {n.kind === "notDownloaded" && ", which isn't downloaded."}
                  {n.kind === "notInProfile" && ", which is in your library but not in this profile."}
                  {n.kind === "switchedOff" && ", which is switched off in this profile."}
                  {n.kind === "external" && " from outside Nexus Mods. Zom's Nexus can't check this one; get it from its page."}
                  {n.kind === "otherGame" && " (a tool on Nexus Mods). Zom's Nexus can't check this one; see its page."}
                  {n.kind === "unavailable" && ", which its author took down on Nexus Mods (or it was removed), so it can't be downloaded. Check its page for a replacement; without it, the mod may not work."}
                </>
              )}
              {n.notes && <span className="muted"> Note from the author: “{n.notes}”</span>}
            </span>
            {(n.kind === "notDownloaded" || (n.kind === "optional" && n.modId > 0)) && (
              <button className="btn sm" onClick={() => onGet(n.modId)}>
                Get…
              </button>
            )}
            {n.kind === "unavailable" && (
              <button className="btn sm ghost" onClick={() => void openUrl(modUrl(gameDomain, n.modId))}>
                Open page
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
        {(report.programs ?? []).map((p) => (
          <li key={"prog" + p.modName} className="check-row warn-text">
            <span className="grow">
              <b>{p.modName}</b> has {p.count === 1 ? "a program" : `${p.count} programs`} in it ({p.files.join(", ")}
              {p.count > p.files.length ? ", …" : ""}). Mods rarely need these. Only run them if the mod's page says to, and you trust its author.
            </span>
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
        {report.installers.map((m) => (
          <li key={m} className="warn-text">
            “{m}” has an installer with choices that haven't been made, so its files go in as they are. Click <b>Choose options</b> on its row in the list.
          </li>
        ))}
      </CheckGroup>
      <CheckGroup tone="info" count={report.conflicts.length} title="Mods with the same files (the lower one in the list wins)" open={isOpen("conf", false)} onToggle={() => toggle("conf", false)}>
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
      </CheckGroup>
      {report.loadOrder.length > 0 && (
        <details className="check-group">
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

function PasteCode({ onClose, onOpened }: { onClose: () => void; onOpened: (r: { profile: Profile; queued: number; note?: string | null }) => void }) {
  const [code, setCode] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  return (
    <Dialog
      title="Paste a share code"
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn primary"
            disabled={!code.trim() || busy}
            onClick={async () => {
              setBusy(true);
              setError("");
              try {
                onOpened(await api.importCode(code));
                onClose();
              } catch (e) {
                setError((e as Error).message);
              } finally {
                setBusy(false);
              }
            }}
          >
            {busy ? "Opening…" : "Open"}
          </button>
        </>
      }
    >
      <p className="muted small" style={{ marginTop: 0 }}>
        A share code is a profile someone copied with <b>Copy share code</b>. It lists Nexus Mods numbers only: you'll get a profile with the same mods, and the ones you don't have are added to downloads (free accounts click each file, as usual).
      </p>
      <textarea className="mono" rows={5} autoFocus placeholder="ZN1-…" value={code} onChange={(e) => setCode(e.target.value)} style={{ width: "100%" }} />
      {error && <p className="err-text small">{error}</p>}
    </Dialog>
  );
}

export function Profiles({ game, setGame, toast, libraryVersion, go, reloadGames, openAdd, gameStates, jobs }: Ctx) {
  const [profiles, setProfiles] = useState<Profile[] | null>(null);
  const [games, setGames] = useState<Game[]>([]);
  const [mods, setMods] = useState<StoredMod[]>([]);
  const [openId, setOpenId] = useState<string | null>(takeWantedProfile);
  useEffect(
    () =>
      onWantProfile(() => {
        const id = takeWantedProfile();
        if (id) setOpenId(id);
      }),
    [],
  );
  const [play, setPlay] = useState<Record<string, PlayTime>>({});
  const [checks, setChecks] = useState<Record<string, CheckReport>>({});
  const [making, setMaking] = useState(false);
  const [adding, setAdding] = useState(false);
  const [deleting, setDeleting] = useState<Profile | null>(null);
  const [busy, setBusy] = useState("");
  const [notes, setNotes] = useState<Record<string, string>>({});
  const [dragFrom, setDragFrom] = useState<number | null>(null);
  const [noting, setNoting] = useState<{ modId: number; name: string } | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);
  const [profDrag, setProfDrag] = useState<number | null>(null);
  const [profOver, setProfOver] = useState<number | null>(null);
  const [installing, setInstalling] = useState<{ mod: StoredMod; fileId: number } | null>(null);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [comparing, setComparing] = useState(false);
  const [ruling, setRuling] = useState<number | null>(null);
  const [halving, setHalvingState] = useState<Halving | null>(null);
  const [pasting, setPasting] = useState(false);
  const [fromMo2, setFromMo2] = useState(false);
  const [fromR2, setFromR2] = useState(false);
  const [hasTs, setHasTs] = useState(false);
  useEffect(() => {
    setHasTs(false);
    if (game) api.tsGame(game).then((t) => setHasTs(!!t?.supported), () => {});
  }, [game]);
  const [updating, setUpdating] = useState<Update[] | null>(null);
  useEffect(() => {
    api.notes().then(setNotes, () => {});
  }, []);

  const load = () => {
    api.profiles().then(setProfiles, (e) => toast((e as Error).message));
    api.games().then(setGames, () => {});
    api.profileChecks().then(setChecks, () => {});
  };
  useEffect(load, []); // eslint-disable-line react-hooks/exhaustive-deps
  // The check before Play changed a profile (switched a mod off, changed its saves).
  useEffect(() => {
    const again = () => void api.profiles().then(setProfiles, () => {});
    window.addEventListener("zoms:profiles-changed", again);
    return () => window.removeEventListener("zoms:profiles-changed", again);
  }, []);
  // Back from Steam (a setting there may have changed): read the games again.
  useEffect(() => {
    const back = () => void api.games().then(setGames, () => {});
    window.addEventListener("focus", back);
    return () => window.removeEventListener("focus", back);
  }, []);
  useEffect(() => {
    api.playTimes().then(setPlay, () => {});
    const off = on<number>("activity:changed", () => void api.playTimes().then(setPlay, () => {}));
    return () => void off.then((f) => f());
  }, []);
  useEffect(() => {
    api.library().then(setMods, () => {});
  }, [libraryVersion]);

  const gameList = useMemo(() => {
    const ids = new Set([...games.map((g) => g.domain), ...(profiles ?? []).map((p) => p.game)]);
    return [...ids].map((d) => games.find((g) => g.domain === d) ?? ({ domain: d, name: d, source: "added" } as Game));
  }, [games, profiles]);
  // Only the picked game's profiles.
  const shown = (profiles ?? []).filter((p) => p.game === game);
  // The one picked; otherwise the one last opened for this game, the one in the game now, or the first.
  const live = gameList.find((g) => g.domain === game)?.activeProfile;
  const open = shown.find((p) => p.id === openId) ?? shown.find((p) => p.id === (game ? rememberedProfile(game) : null)) ?? shown.find((p) => p.name === live) ?? shown[0] ?? null;
  const openGame = open ? gameList.find((g) => g.domain === open.game) : undefined;
  useEffect(() => {
    if (open) {
      rememberProfile(open.game, open.id);
      rememberRecent({ id: open.id, name: open.name, game: open.game });
    }
  }, [open?.id]); // eslint-disable-line react-hooks/exhaustive-deps
  const modName = (id: number) => mods.find((m) => m.game === open?.game && m.modId === id)?.name;
  const lastRun = useLastRun(open?.id, open?.mods);
  const [editing, setEditing] = useState(false);
  const [merging, setMerging] = useState(false);
  const [asking, setAsking] = useState(false);
  const [protons, setProtons] = useState<string[]>([]);
  useEffect(() => {
    api.customProtons().then(setProtons, () => {});
  }, []);
  const facts = useFacts(open ? [...new Set(open.mods.map((m) => factKey(open.game, m.modId)))].sort() : []);
  const [show, setShow] = useState("all");
  const tags = useTags();
  useEffect(() => setHalvingState(open ? halvingOf(open.id) : null), [open?.id]); // eslint-disable-line react-hooks/exhaustive-deps
  /** What a mod needs, from what Nexus Mods says (suggestions left out). */
  const needsOf = (id: number) => (open ? (facts[factKey(open.game, id)]?.requires ?? []).filter((q) => !softNote(q.notes)).map((q) => q.modId) : []);
  const halvingStep = async (h: Halving | null, message: string) => {
    if (!open) return;
    setHalving(open.id, h);
    setHalvingState(h);
    await save({ ...open, mods: h ? roundMods(h) : (halving?.original ?? open.mods) });
    toast(message);
  };
  const [query, setQuery] = useState("");
  // Another profile (picked, or shown because the game changed) starts with no search or filter.
  useEffect(() => {
    setQuery("");
    setShow("all");
  }, [open?.id]);
  const report = open ? checks[open.id] : undefined;
  /** Mods the check names in a problem or warning. */
  const flagged = useMemo(() => {
    const names = new Set<string>();
    const ids = new Set<number>();
    if (report) {
      for (const n of report.needs ?? []) n.neededBy.forEach((x) => names.add(x));
      for (const c of report.compat ?? []) ids.add(c.modId);
      for (const p of report.plugins) names.add(p.modName);
      for (const p of report.programs ?? []) names.add(p.modName);
      report.installers.forEach((x) => names.add(x));
    }
    return { names, ids };
  }, [report]);
  const has = (m: { modId: number; fileId: number }) => mods.some((l) => l.game === open?.game && l.modId === m.modId && l.files.some((f) => f.fileId === m.fileId));
  const categories = open ? [...new Set(open.mods.map((m) => facts[factKey(open.game, m.modId)]?.category).filter((c): c is string => !!c))].sort() : [];
  const fits = (m: { modId: number; fileId: number; enabled: boolean }) => {
    if (!open) return true;
    const f = facts[factKey(open.game, m.modId)];
    const lib = mods.find((l) => l.game === open.game && l.modId === m.modId);
    switch (show) {
      case "all": return true;
      case "on": return m.enabled;
      case "off": return !m.enabled;
      case "missing": return !has(m);
      case "flagged": return flagged.ids.has(m.modId) || flagged.names.has(lib?.name ?? "");
      case "gone": return isGone(f);
      case "adult": return isAdult(facts, open.game, m.modId, lib?.adult);
      case "notAdult": return !isAdult(facts, open.game, m.modId, lib?.adult);
      default: if (show.startsWith("tag:")) return !!tags[`${open.game}:${m.modId}`]?.includes(show.slice(4));
        return show.startsWith("cat:") && f?.category === show.slice(4);
    }
  };
  // The search box: mod name, file name or author (and the mod's number).
  const matches = (m: { modId: number; fileId: number }) => {
    const q = query.trim().toLowerCase();
    if (!q || !open) return true;
    const lib = mods.find((l) => l.game === open.game && l.modId === m.modId);
    const f = lib?.files.find((x) => x.fileId === m.fileId);
    return `${lib?.name ?? facts[factKey(open.game, m.modId)]?.name ?? ""} ${f?.name ?? ""} ${lib?.author ?? ""} ${m.modId}`.toLowerCase().includes(q);
  };
  const picked = open ? open.mods.filter((m) => fits(m) && matches(m)) : [];
  const bulk = (change: "on" | "off" | "out") => {
    if (!open) return;
    const hit = new Set(picked.map((m) => `${m.modId}:${m.fileId}`));
    const k = (m: { modId: number; fileId: number }) => `${m.modId}:${m.fileId}`;
    const next = change === "out" ? open.mods.filter((m) => !hit.has(k(m))) : open.mods.map((m) => (hit.has(k(m)) ? { ...m, enabled: change === "on" } : m));
    void save({ ...open, mods: next }, `${change === "out" ? "Took" : change === "on" ? "Switched on" : "Switched off"} ${plural(hit.size, "mod")}${change === "out" ? ` out of “${open.name}”. They stay in your library.` : ""}`);
    if (change === "out") setShow("all");
  };
  const fixAll = async () => {
    if (!open) return;
    const f = autoFix(open, mods, checks[open.id]);
    if (f.done.length) await save(f.profile);
    let queued = 0;
    if (f.download.length) {
      const r = await api.queueRequirements(open.game, f.download).catch((e) => {
        toast((e as Error).message);
        return null;
      });
      queued = r?.queued.length ?? 0;
    }
    const said = [...f.done, queued ? `added ${plural(queued, "missing requirement")} to downloads` : ""].filter(Boolean);
    toast(said.length ? `${said.join("; ")}.`.replace(/^./, (c) => c.toUpperCase()) : "Nothing left that Zom's Nexus can fix by itself. The rest needs a look.");
  };
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

  /** Save a profile. With `what`, the toast says what changed and offers Undo. */
  const save = async (p: Profile, what?: string) => {
    const before = profiles?.find((x) => x.id === p.id);
    try {
      const saved = await api.saveProfile(p);
      setProfiles((all) => (all ?? []).map((x) => (x.id === saved.id ? saved : x)));
      autoCheck(saved.id);
      // A rule kept a mod where it was.
      const order = (q: Profile) => q.mods.map((m) => m.modId).join();
      const kept = order(saved) !== order(p);
      if (kept && !what) toast("Kept your load order rules: a mod with a rule stays after the one it follows.");
      else if (what && before)
        toast(kept ? `${what}. Mods with a rule moved to keep it.` : what, {
          label: "Undo",
          run: () =>
            void api.saveProfile(before).then((b) => {
              setProfiles((all) => (all ?? []).map((x) => (x.id === b.id ? b : x)));
              autoCheck(b.id);
            }),
        });
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

  /** Move a profile in this game's list (dragged), keeping other games' profiles where they are. */
  const moveProfile = (from: number, to: number) => {
    const list = [...shown];
    const [p] = list.splice(from, 1);
    list.splice(to, 0, p);
    let k = 0;
    const ids = (profiles ?? []).map((x) => (x.game === game ? list[k++].id : x.id));
    setProfiles((all) => ids.map((id) => (all ?? []).find((x) => x.id === id)!));
    api.orderProfiles(ids).then(setProfiles, (e) => toast((e as Error).message));
  };

  const playNow = (p: Profile) => requestPlay(p.id, () => void run("play", () => api.play(p.id), (a) => `${appliedWords(a)}. Starting the game.`).then(() => load()));

  const move = (i: number, d: number) => {
    if (!open) return;
    const mods2 = [...open.mods];
    const [m] = mods2.splice(i, 1);
    mods2.splice(i + d, 0, m);
    void save({ ...open, mods: mods2 }, `Moved “${modName(m.modId) ?? `Mod ${m.modId}`}” ${d < 0 ? "up" : "down"}`);
  };

  const check = async () => {
    if (!open) return;
    const r = await run("check", () => api.checkProfile(open.id));
    if (r) setChecks((c) => ({ ...c, [open.id]: r }));
  };
  const openId2 = open?.id;
  // Check each time a profile is opened, and again whenever a download lands in the
  // library (a collection's profile fills in as its files finish).
  const firstLib = useRef(libraryVersion);
  useEffect(() => {
    // Each time a profile is opened (the last result shows meanwhile). It only reads this
    // computer, plus Nexus Mods for mods not asked about today.
    if (openId2) autoCheck(openId2);
  }, [openId2]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (libraryVersion !== firstLib.current && openId2) autoCheck(openId2);
  }, [libraryVersion]); // eslint-disable-line react-hooks/exhaustive-deps
  // Saves in this profile's own folder that are copies of the shared ones (Steam Cloud).
  const [copies, setCopies] = useState(0);
  const ownIn = !!open?.ownSaves && !!openGame?.activeProfile && openGame.activeProfile === open?.name;
  useEffect(() => {
    if (!ownIn || !open) return setCopies(0);
    const look = () => void api.savesCopies(open.game).then(setCopies, () => setCopies(0));
    look();
    window.addEventListener("focus", look);
    return () => window.removeEventListener("focus", look);
  }, [ownIn, open?.id, gameStates[open?.game ?? ""]?.state]); // eslint-disable-line react-hooks/exhaustive-deps
  // When the game closes (or didn't start), check again: its logs may say what went wrong.
  const gameNow = open ? gameStates[open.game]?.state : undefined;
  useEffect(() => {
    if (openId2 && (gameNow === "stopped" || gameNow === "notStarted" || gameNow === "running")) autoCheck(openId2);
  }, [gameNow]); // eslint-disable-line react-hooks/exhaustive-deps

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
                toast([r.queued ? `Opened “${r.profile.name}”; ${plural(r.queued, "file")} to download` : `Opened “${r.profile.name}”`, r.note].filter(Boolean).join(". "));
              }
            }}
          >
            <FileUp size={15} /> Open mod pack…
          </button>
        )}
        {game && hasTs && (
          <button className="btn" onClick={() => setFromR2(true)} title="Make a profile from an r2modman profile or profile code">
            <FolderInput size={15} /> From r2modman…
          </button>
        )}
        {game && (
          <button className="btn" onClick={() => setFromMo2(true)} title="Make a profile from a Mod Organizer 2 setup, without downloading anything again">
            <FolderInput size={15} /> From MO2…
          </button>
        )}
        <button className="btn" onClick={() => setPasting(true)} title="Open a profile someone shared as a code">
          <ClipboardPaste size={15} /> Paste share code…
        </button>
        <button className="btn primary" onClick={() => setMaking(true)} disabled={!game} title={game ? undefined : "Pick a game first"}>
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
            {shown.map((p, i) => {
              const g = gameList.find((x) => x.domain === p.game);
              const live = g?.activeProfile === p.name;
              const problems = checks[p.id] ? problemCount(checks[p.id]) : 0;
              return (
                <button
                  key={p.id}
                  className={`profile-item${open?.id === p.id ? " active" : ""}${profDrag !== null && profOver === i && profDrag !== i ? (profDrag < i ? " drop-after" : " drop-before") : ""}${profDrag === i ? " dragging" : ""}`}
                  onClick={() => setOpenId(p.id)}
                  title={`${p.name}\nDrag to move it in the list`}
                  draggable
                  onDragStart={(e) => {
                    setProfDrag(i);
                    e.dataTransfer.effectAllowed = "move";
                    e.dataTransfer.setData("text/plain", p.id);
                  }}
                  onDragOver={(e) => {
                    if (profDrag === null) return;
                    e.preventDefault();
                    setProfOver(i);
                  }}
                  onDragEnd={() => {
                    setProfDrag(null);
                    setProfOver(null);
                  }}
                  onDrop={(e) => {
                    e.preventDefault();
                    if (profDrag !== null && profDrag !== i) moveProfile(profDrag, i);
                    setProfDrag(null);
                    setProfOver(null);
                  }}
                >
                  <Cover domain={p.game} name={g?.name ?? p.game} className="mini" />
                  <span className="profile-text">
                    <b className="ellipsis">
                      {checks[p.id] && (
                        <span
                          className={`health-dot ${problems > 0 ? "bad" : (checks[p.id].compat?.length ?? 0) + (checks[p.id].needs?.length ?? 0) > 0 ? "warn" : "ok"}`}
                          title={problems > 0 ? "Its last check found problems" : (checks[p.id].compat?.length ?? 0) + (checks[p.id].needs?.length ?? 0) > 0 ? "Its last check found warnings" : "Its last check found nothing to fix"}
                        />
                      )}
                      {p.name}
                    </b>
                    <span className="sub">
                      {(() => {
                        const pr = profileProgress(p, mods);
                        return pr.have < pr.total ? `${pr.have} of ${pr.total} mods downloaded` : plural(pr.total, "mod");
                      })()}
                    </span>
                    {(problems > 0 || p.ownSaves || p.newDownloads || live || p.collection) && (
                      <span className="profile-tags">
                        {live && (
                          <span className="pill ok" title="This profile's mods are in the game's folder now, so they load whenever the game starts">
                            Active
                          </span>
                        )}
                        {problems > 0 && (
                          <span className="pill danger" title={`The conflict check found ${plural(problems, "problem")} that stop mods working. Open the profile to see them.`}>
                            {plural(problems, "problem")}
                          </span>
                        )}
                        {p.collection && (
                          <span className="pill" title={`Made from a collection${p.addons?.length ? `, with ${plural(p.addons.length, "add-on")}` : ""}`}>
                            Collection
                          </span>
                        )}
                        {p.ownSaves && (
                          <span className="pill" title="Keeps its own saves">
                            Saves
                          </span>
                        )}
                        {p.newDownloads && (
                          <span className="pill" title="New downloads for this game go into this profile">
                            New
                          </span>
                        )}
                      </span>
                    )}
                  </span>
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
                  <PlayButton
                    state={gameStates[open.game]}
                    disabled={!!busy}
                    putting={busy === "play"}
                    onPlay={() => playNow(open)}
                  />
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
                <label
                  className="check"
                  title={
                    openGame?.saveFolder
                      ? `This profile keeps its own saves. When it goes into the game, the saves there now are moved aside (nothing is deleted) and this profile's come in: ${openGame.saveFolder}`
                      : "Set where this game keeps its saves on the Games page first"
                  }
                  style={openGame?.saveFolder ? undefined : { opacity: 0.5 }}
                >
                  <input
                    type="checkbox"
                    disabled={!openGame?.saveFolder}
                    checked={!!open.ownSaves}
                    onChange={(e) => {
                      void save({ ...open, ownSaves: e.target.checked });
                      if (e.target.checked && openGame?.steamCloud) toast(`Steam Cloud keeps ${openGame.name}'s saves and puts them back into this profile's folder. Turn it off for this game in Steam (Properties › General) to keep the saves apart.`);
                    }}
                  />{" "}
                  Own saves
                </label>

                <button className="btn" disabled={!!busy} onClick={() => run("apply", () => api.applyProfile(open.id), appliedWords)} title="Put the mods in without starting the game">
                  <Upload size={15} /> {busy === "apply" ? "Putting mods in…" : "Put mods in game"}
                </button>
                {openGame?.activeProfile && (
                  <button className="btn" disabled={!!busy} onClick={() => run("clear", () => api.clearGame(open.game), () => `Took the mods out of ${openGame.name}; its own files are back`)}>
                    <RotateCcw size={15} /> Take mods out
                  </button>
                )}
                {(openGame?.steamAppId || openGame?.launch) && (
                  <button className="btn" disabled={!!busy} title="Take the mods out and start the game as it came: is it the mods, or the game?" onClick={() => run("clear", () => playWithoutMods(open.game), () => `Starting ${openGame?.name ?? "the game"} without mods. Play puts this profile back in.`)}>
                    <CirclePlay size={15} /> Play without mods
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
                    setUpdating(newer);
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
                {(profiles ?? []).some((x) => x.game === open.game && x.id !== open.id) && (
                  <button className="btn ghost" title="Add another profile's mods to this one" onClick={() => setMerging(true)}>
                    <Merge size={15} /> Add a profile's mods…
                  </button>
                )}
                <button className="btn ghost" title="Ask an AI about this profile: crashes, logs, which mod to blame (optional, set up in Settings)" onClick={() => setAsking(true)}>
                  <Bot size={15} /> Ask AI…
                </button>
                <button className="btn ghost" title="Change what the mods' settings files say, for this profile only" onClick={() => setEditing(true)}>
                  <SlidersHorizontal size={15} /> Mod settings
                </button>
                {!halving && open.mods.filter((m) => m.enabled).length > 1 && (
                  <button
                    className="btn ghost"
                    title="Something's wrong in the game and you don't know which mod: switch half the mods off, play, say if it's still there, and repeat"
                    onClick={() => {
                      const h = startHalving(open.mods, needsOf);
                      void halvingStep(h, `Round 1: ${plural(h.suspects.length - h.on.length, "mod")} switched off. Play, then say whether the problem is still there.`);
                    }}
                  >
                    <SearchCheck size={15} /> Find a problem mod
                  </button>
                )}
                <button className="btn ghost" title="What went into the game before, to go back to" onClick={() => setHistoryOpen(true)}>
                  <HistoryIcon size={15} /> History
                </button>
                {shown.length > 1 && (
                  <button className="btn ghost" title="See how this profile differs from another" onClick={() => setComparing(true)}>
                    <Columns2 size={15} /> Compare
                  </button>
                )}
                <span className="grow" />
                <button
                  className="btn ghost"
                  title="Copy this profile as a code to paste in a message: Nexus Mods numbers only, never the mods' files"
                  onClick={async () => {
                    const code = await run("share", () => api.shareCode(open.id));
                    if (code) await navigator.clipboard.writeText(code).then(() => toast(`Copied a share code for “${open.name}” (${Math.ceil(code.length / 1000)} KB of text). Whoever pastes it in Zom's Nexus gets the same mods.`), () => toast("Couldn't copy the code."));
                  }}
                >
                  <Share2 size={15} /> Copy share code
                </button>
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

              <details className="profile-note" open={!!open.note}>
                <summary className="small muted">{open.note ? "Your notes" : "Add notes about this profile"}</summary>
                <textarea
                  key={open.id}
                  rows={3}
                  placeholder="What it's for, what to remember, what to try next…"
                  defaultValue={open.note ?? ""}
                  onBlur={(e) => {
                    const v = e.target.value.trim();
                    if (v !== (open.note ?? "")) void save({ ...open, note: v || null });
                  }}
                />
              </details>
              {open.tsPack && <TsUpdates game={open.game} profile={open} toast={toast} onProfile={() => load()} />}
              {lastRun && !halving && (
                <RunPanel
                  t={lastRun}
                  gameName={openGame?.name ?? open.game}
                  profile={open}
                  tip={protonTip(openGame?.proton, protons, openGame?.name ?? open.game, openGame?.source === "steam")}
                  onSwitchOff={(ids) => void save({ ...open, mods: open.mods.map((x) => (ids.includes(x.modId) ? { ...x, enabled: false } : x)) }, `Switched off ${ids.length === 1 ? `“${modName(ids[0]) ?? `Mod ${ids[0]}`}”` : `${ids.length} new mods`}. Play to check.`)}
                  onFind={() => {
                    const h = startHalving(open.mods, needsOf);
                    void halvingStep(h, `Round 1: ${plural(h.suspects.length - h.on.length, "mod")} switched off. Play, then say whether it still closes.`);
                  }}
                />
              )}
              {checks[open.id]?.gameLog?.smapi && <SmapiPanel s={checks[open.id].gameLog!.smapi!} profile={open} library={mods} jobs={jobs} toast={toast} onUpdates={setUpdating} onPlay={() => playNow(open)} onSwitchOff={(modId) => {
                const also = open.manualRequirements ? [] : needersOf(modId, open.game, open.mods, facts);
                void save({ ...open, mods: open.mods.map((x) => (x.modId === modId || also.includes(x.modId) ? { ...x, enabled: false } : x)) }, `Switched off “${modName(modId) ?? `Mod ${modId}`}”${also.length ? ` and what needs it (${also.length})` : ""}. Put the mods in and play to check.`);
              }} />}
              {halving && (
                <div className="panel collection-note accent halving">
                  {halving.suspects.length === 1 ? (
                    <>
                      <div className="grow small">
                        <b>Found it: {modName(halving.suspects[0]) ?? `Mod ${halving.suspects[0]}`}</b> is the one, after {plural(halving.round - 1, "round")}. Put everything back as it was, then switch it off or look for an update.
                      </div>
                      <button className="btn primary sm" onClick={() => void halvingStep(null, "Put the mods back as they were. Put them in the game again to play with them.")}>
                        Put everything back
                      </button>
                      <button
                        className="btn sm"
                        onClick={async () => {
                          const bad = halving.suspects[0];
                          setHalving(open.id, null);
                          setHalvingState(null);
                          await save({ ...open, mods: halving.original.map((m) => (m.modId === bad ? { ...m, enabled: false } : m)) });
                          toast(`Put the mods back, with “${modName(bad) ?? `Mod ${bad}`}” switched off`);
                        }}
                      >
                        Put back, with it off
                      </button>
                    </>
                  ) : (
                    <>
                      <div className="grow small">
                        <b>Finding the problem mod, round {halving.round}:</b> {plural(halving.suspects.length, "suspect")} left, {plural(halving.suspects.length - halving.on.length, "mod")} switched off. Play, then say whether the problem is still there.
                      </div>
                      <button className="btn sm" onClick={() => void halvingStep(halvingAnswer(halving, true, needsOf), "Narrowed it down to the mods that were on")}>
                        Still there
                      </button>
                      <button className="btn sm" onClick={() => void halvingStep(halvingAnswer(halving, false, needsOf), "Narrowed it down to the mods that were off")}>
                        It's gone
                      </button>
                      <button className="btn ghost sm" onClick={() => void halvingStep(null, "Stopped, and put the mods back as they were")}>
                        Stop
                      </button>
                    </>
                  )}
                </div>
              )}

              <CollectionCard
                profile={open}
                toast={toast}
                onChanged={(p) => {
                  setProfiles((all) => (all ?? []).map((x) => (x.id === p.id ? p : x)));
                  autoCheck(p.id);
                }}
              />

              {copies > 0 && (
                <div className="panel banner-panel warn-banner" role="alert">
                  <AlertTriangle size={18} color="var(--zb-warn)" />
                  <div className="grow">
                    <b>{copies === 1 ? "1 save here is a copy" : `${copies} saves here are copies`} of your shared saves</b>
                    <p className="small" style={{ margin: "4px 0 0" }}>
                      Steam Cloud put them into this profile's own saves, so the game still shows them. Move them out and this profile starts with only its own saves. Nothing is deleted: they go into a “copies” folder beside your saves. Quit the game first.
                    </p>
                  </div>
                  <button
                    className="btn primary"
                    onClick={() =>
                      void api.savesMoveCopies(open.game).then(
                        (n) => {
                          setCopies(0);
                          toast(n ? `Moved ${plural(n, "copied save")} out. This profile now has only its own saves.` : "No copies left to move.");
                        },
                        (e) => toast((e as Error).message),
                      )
                    }
                  >
                    Move them out
                  </button>
                </div>
              )}
              {open.ownSaves && openGame?.steamCloud && (
                <div className="panel banner-panel warn-banner" role="alert">
                  <AlertTriangle size={18} color="var(--zb-warn)" />
                  <div className="grow">
                    <b>Steam Cloud is putting your other saves back into this profile</b>
                    <p className="small" style={{ margin: "4px 0 6px" }}>
                      This profile keeps its own saves, but Steam Cloud keeps {openGame.name}'s saves too, so it copies them back into this profile's folder each time the game starts. Your saves are safe; they just aren't kept apart. To keep them apart, turn Steam Cloud off for this game:
                    </p>
                    <ol className="small steps">
                      <li>In Steam, right-click <b>{openGame.name}</b> in your library and pick <b>Properties</b>.</li>
                      <li>On <b>General</b>, switch off <b>Keep game saves in the Steam Cloud</b>.</li>
                      <li>Come back here: this message goes away by itself.</li>
                    </ol>
                    <p className="small muted" style={{ margin: "6px 0 0" }}>Or switch off Own saves above to keep using Steam Cloud.</p>
                  </div>
                </div>
              )}
              <ProfileStats profile={open} mods={mods} facts={facts} play={play[open.id]} report={checks[open.id]} />
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
                  gameDomain={open.game}
                  onCopied={() => toast("Copied. Paste it into the game's Launch Options in Steam, then start the game again.")}
                  checking={checking === open.id || busy === "check"}
                  fixable={(() => {
                    const f = autoFix(open, mods, checks[open.id]);
                    return f.done.length + f.download.length;
                  })()}
                  onFix={() => void fixAll()}
                  onGet={(modId) => openAdd(modUrl(open.game, modId))}
                  onAdd={(modId) => {
                    const m = mods.find((x) => x.game === open.game && x.modId === modId);
                    const f = m && [...m.files].sort((a, b) => b.downloadedAt - a.downloadedAt)[0];
                    if (m && f) void save({ ...open, mods: [...open.mods, { modId, fileId: f.fileId, enabled: true }] }).then(() => toast(`Added “${m.name}” to “${open.name}”`));
                  }}
                  onSwitchOn={(modId) => void save({ ...open, mods: open.mods.map((x) => (x.modId === modId ? { ...x, enabled: true } : x)) })}
                  onUpdate={async (modId, name) => {
                    try {
                      const { files } = await api.modFiles(open.game, modId);
                      const main = files.filter((f) => (f.category ?? "").toUpperCase() === "MAIN");
                      const newest = [...(main.length ? main : files)].sort((a, b) => (b.uploaded ?? 0) - (a.uploaded ?? 0))[0];
                      if (!newest) return `Nexus Mods lists no files for ${name}.`;
                      if (open.mods.some((m) => m.modId === modId && m.fileId === newest.fileId))
                        return checks[open.id]?.outOfDate
                          ? `This profile already has ${name}'s newest main file (${newest.version ?? newest.name}), but the game still has the old one. Put the mods in again (button above).`
                          : `This profile already has ${name}'s newest main file (${newest.version ?? newest.name}). Check ${name}'s page for an extra file it may need.`;
                      await api.add(open.game, modId, newest.fileId);
                      rememberUpdates(open.id, [{ modId, fileId: newest.fileId }]);
                      return `Downloading ${name} ${newest.version ?? ""}. When it's done it takes the old file's place here; then put the mods in again.`;
                    } catch (e) {
                      return (e as Error).message;
                    }
                  }}
                  tried={(() => {
                    const t = triedOff(open.id);
                    return t && t.ids.length ? { names: t.ids.map((id) => modName(id) ?? `Mod ${id}`), at: t.at } : null;
                  })()}
                  canTry={(id) => needersOf(id, open.game, open.mods, facts).length === 0}
                  onTry={async (ids) => {
                    const off = ids.filter((id) => open.mods.some((m) => m.modId === id && m.enabled));
                    if (!off.length) return toast("Those mods are already switched off.");
                    await save({ ...open, mods: open.mods.map((x) => (off.includes(x.modId) ? { ...x, enabled: false } : x)) });
                    setTriedOff(open.id, off);
                    const a = await run("apply", () => api.applyProfile(open.id), appliedWords);
                    if (a) toast(`Switched off ${off.map((id) => modName(id) ?? `Mod ${id}`).join(", ")} and put the mods in. Play and do the same thing again.`);
                  }}
                  onUndoTry={() => {
                    const ids = triedOff(open.id)?.ids ?? [];
                    setTriedOff(open.id, []);
                    void save({ ...open, mods: open.mods.map((x) => (ids.includes(x.modId) ? { ...x, enabled: true } : x)) }).then(() => toast("Switched them back on. Put the mods in (or Play) for the game to have them again."));
                  }}
                  onApply={() => void run("apply", () => api.applyProfile(open.id), appliedWords).then(() => autoCheck(open.id))}
                  onSwitchOff={(modId) => {
                    // What needs it goes off too; otherwise Handle requirements would switch it back on.
                    const also = needersOf(modId, open.game, open.mods, facts);
                    const off = new Set([modId, ...also]);
                    const names = (ids: number[]) => ids.map((id) => modName(id) ?? `Mod ${id}`);
                    void save({ ...open, mods: open.mods.map((x) => (off.has(x.modId) ? { ...x, enabled: false } : x)) }).then(() =>
                      toast(`Switched off “${names([modId])[0]}”${also.length ? ` and what needs it: ${names(also).join(", ")}` : ""}. Put the mods in again (or Play) for the game to have the change.`),
                    );
                  }}
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
                <>
                <div className="toolbar">
                  <label className="row gap-sm">
                    <span className="muted small">Show</span>
                    <select value={show} onChange={(e) => setShow(e.target.value)} aria-label="Show">
                      <option value="all">All mods</option>
                      <option value="on">Switched on</option>
                      <option value="off">Switched off</option>
                      <option value="missing">Not downloaded</option>
                      {report && <option value="flagged">Named in the check</option>}
                      <option value="gone">Gone from Nexus Mods</option>
                      <option value="adult">Adult</option>
                      <option value="notAdult">Not adult</option>
                      {categories.length > 0 && (
                        <optgroup label="Category">
                          {categories.map((c) => (
                            <option key={c} value={`cat:${c}`}>
                              {c}
                            </option>
                          ))}
                        </optgroup>
                      )}
                      {tagsIn(tags, open.mods.map((m) => `${open.game}:${m.modId}`)).length > 0 && (
                        <optgroup label="Your tags">
                          {tagsIn(tags, open.mods.map((m) => `${open.game}:${m.modId}`)).map((t) => (
                            <option key={t} value={`tag:${t}`}>
                              {t}
                            </option>
                          ))}
                        </optgroup>
                      )}
                    </select>
                  </label>
                  <label className="search">
                    <Search size={14} />
                    <input placeholder="Search this profile" value={query} onChange={(e) => setQuery(e.target.value)} aria-label="Search this profile's mods" />
                  </label>
                  {(show !== "all" || query.trim()) && (
                    <>
                      <span className="muted small num">
                        {picked.length} of {open.mods.length}
                      </span>
                      {picked.length > 0 && (
                        <>
                          {picked.some((m) => !m.enabled) && (
                            <button className="btn sm" onClick={() => bulk("on")}>
                              Switch these on
                            </button>
                          )}
                          {picked.some((m) => m.enabled) && (
                            <button className="btn sm" onClick={() => bulk("off")}>
                              Switch these off
                            </button>
                          )}
                          <button className="btn sm ghost danger" onClick={() => bulk("out")} title="Takes them out of this profile; they stay in your library">
                            Take these out
                          </button>
                        </>
                      )}
                    </>
                  )}
                </div>
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
                      {picked.length === 0 && (
                        <tr>
                          <td colSpan={4} className="muted">
                            {query.trim() ? `No mods in this profile match “${query.trim()}”.` : "No mods in this profile are like that."}{" "}
                            <button
                              className="link-name"
                              onClick={() => {
                                setShow("all");
                                setQuery("");
                              }}
                            >
                              Show all
                            </button>
                          </td>
                        </tr>
                      )}
                      {open.mods.map((m, i) => {
                        if (!fits(m) || !matches(m)) return null;
                        const name = modName(m.modId);
                        const f = facts[factKey(open.game, m.modId)];
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
                              <input
                                type="checkbox"
                                aria-label="Use this mod"
                                checked={m.enabled}
                                onChange={(e) => {
                                  const on = e.target.checked;
                                  // Switching off something others need: they go off too, or Handle requirements switches it back on.
                                  const also = on || open.manualRequirements ? [] : needersOf(m.modId, open.game, open.mods, facts);
                                  void save(
                                    { ...open, mods: open.mods.map((x, j) => (j === i ? { ...x, enabled: on } : also.includes(x.modId) ? { ...x, enabled: false } : x)) },
                                    `Switched “${name ?? `Mod ${m.modId}`}” ${on ? "on" : "off"}${also.length ? `, and what needs it: ${also.map((id) => modName(id) ?? `Mod ${id}`).join(", ")}` : ""}`,
                                  );
                                }}
                              />
                            </td>
                            <td>
                              <div className="name">{name ?? `Mod ${m.modId}`}</div>
                              <div className="sub">
                                {name ? fileName(m.modId, m.fileId) ?? `File ${m.fileId} (not downloaded)` : "Not downloaded yet"}
                                {isAdult(facts, open.game, m.modId, mods.find((l) => l.game === open.game && l.modId === m.modId)?.adult) && <span className="pill warn tiny-pill">Adult</span>}
                                {isGone(f) && (
                                  <span className="pill warn tiny-pill" title="Nexus Mods doesn't show this mod any more">
                                    Gone from Nexus Mods
                                  </span>
                                )}
                                {f?.category && <span className="muted"> · {f.category}</span>}{" "}
                                <TagChips tags={tags[`${open.game}:${m.modId}`]} />
                                {(() => {
                                  const lm = mods.find((l) => l.game === open.game && l.modId === m.modId);
                                  const lf = lm?.files.find((x) => x.fileId === m.fileId);
                                  if (!lm || !lf || !(lf.hasInstaller || lf.installed)) return null;
                                  return (
                                    <button className={`pill tiny-pill pill-btn${lf.installed ? " ok" : " warn"}`} title={lf.installed ? "Installed with your installer choices. Click to change them." : "This mod has an installer: pick its options"} onClick={() => setInstalling({ mod: lm, fileId: lf.fileId })}>
                                      {lf.installed ? "Options chosen" : "Choose options"}
                                    </button>
                                  );
                                })()}
                              </div>
                              {notes[`${open.game}:${m.modId}`] && <div className="note">“{notes[`${open.game}:${m.modId}`]}”</div>}
                              {(open.rules ?? [])
                                .filter((r) => r.modId === m.modId)
                                .map((r) => (
                                  <span key={r.after} className="pill tiny-pill rule-pill" title="Your rule: this mod always loads after that one">
                                    After {modName(r.after) ?? `Mod ${r.after}`}
                                    <button aria-label="Remove rule" onClick={() => void save({ ...open, rules: (open.rules ?? []).filter((x) => x !== r) }, "Removed the rule")}>
                                      <X size={11} />
                                    </button>
                                  </span>
                                ))}
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
                              <button className="icon-btn" aria-label="Load after another mod" title="Always load this after another mod (a rule)" onClick={() => setRuling(m.modId)}>
                                <ArrowDownToLine size={15} />
                              </button>
                              <button className="icon-btn" aria-label="Load earlier" title="Load earlier" disabled={i === 0} onClick={() => move(i, -1)}>
                                <ArrowUp size={15} />
                              </button>
                              <button className="icon-btn" aria-label="Load later" title="Load later (wins conflicts)" disabled={i === open.mods.length - 1} onClick={() => move(i, 1)}>
                                <ArrowDown size={15} />
                              </button>
                              <button className="icon-btn danger" aria-label="Take out of profile" title="Take out of this profile" onClick={() => save({ ...open, mods: open.mods.filter((_, j) => j !== i), rules: (open.rules ?? []).filter((r) => r.modId !== m.modId && r.after !== m.modId) }, `Took “${name ?? `Mod ${m.modId}`}” out of “${open.name}”`)}>
                                <Trash2 size={15} />
                              </button>
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>
                </>
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

      {historyOpen && open && (
        <HistoryDialog
          game={open.game}
          gameName={openGame?.name ?? open.game}
          profiles={profiles ?? []}
          running={gameStates[open.game]?.state === "running"}
          name={(id) => modName(id) ?? facts[factKey(open.game, id)]?.name ?? `Mod ${id}`}
          onClose={() => setHistoryOpen(false)}
          onRestored={async (p) => {
            setProfiles((all) => (all ?? []).some((x) => x.id === p.id) ? (all ?? []).map((x) => (x.id === p.id ? p : x)) : [...(all ?? []), p]);
            setOpenId(p.id);
            const a = await api.applyProfile(p.id);
            toast(`Went back: “${p.name}” is as it was, and ${appliedWords(a)}.`);
            reloadGames();
          }}
        />
      )}
      {comparing && open && <CompareDialog a={open} profiles={profiles ?? []} name={(id) => modName(id) ?? facts[factKey(open.game, id)]?.name ?? `Mod ${id}`} onClose={() => setComparing(false)} />}
      {ruling !== null && open && <RuleDialog profile={open} modId={ruling} name={(id) => modName(id) ?? `Mod ${id}`} onClose={() => setRuling(null)} onSave={(p) => void save(p, "Added the rule")} />}
      {asking && open && (
        <AiChat
          profile={open}
          onClose={() => setAsking(false)}
          onSetup={() => {
            setAsking(false);
            go("settings");
          }}
        />
      )}
      {merging && open && (
        <Dialog title={`Add another profile's mods to “${open.name}”`} onClose={() => setMerging(false)}>
          <p style={{ marginTop: 0 }}>Its mods go after this profile's. Mods this one has already stay where they are; if the other profile has them on, they're switched on here too, with its file.</p>
          <div className="pick-list">
            {(profiles ?? [])
              .filter((x) => x.game === open.game && x.id !== open.id)
              .map((x) => (
                <button
                  key={x.id}
                  className="btn"
                  onClick={async () => {
                    const r = merge(open.mods, x.mods);
                    setMerging(false);
                    if (!r.added && !r.changed) return toast(`“${open.name}” already has everything in “${x.name}”`);
                    await save({ ...open, mods: r.mods }, `Added ${plural(r.added, "mod")} from “${x.name}”${r.changed ? ` and switched on or updated ${r.changed}` : ""}`);
                  }}
                >
                  {x.name} <span className="muted small">· {plural(x.mods.filter((m) => m.enabled).length, "mod")}</span>
                </button>
              ))}
          </div>
        </Dialog>
      )}
      {editing && open && <ModSettings profile={open} toast={toast} onClose={() => setEditing(false)} />}
      {updating && open && (
        <UpdatesDialog
          profile={open}
          updates={updating}
          version={(modId, fileId) => mods.find((l) => l.game === open.game && l.modId === modId)?.files.find((f) => f.fileId === fileId)?.version}
          name={(id) => modName(id) ?? `Mod ${id}`}
          onClose={() => setUpdating(null)}
          onUpdate={async (picked) => {
            for (const u of picked) await api.add(u.game, u.modId, u.newer!.fileId).catch(() => {});
            // When each finishes, it takes the old file's place in this profile.
            rememberUpdates(open.id, picked.map((u) => ({ modId: u.modId, fileId: u.newer!.fileId })));
            toast(`${plural(picked.length, "update")} added to downloads; each takes the old file's place here when it's done`);
          }}
        />
      )}
      {fromR2 && game && (
        <R2Import
          game={game}
          gameName={games.find((g) => g.domain === game)?.name ?? game}
          onClose={() => setFromR2(false)}
          onDone={(p, said) => {
            load();
            setOpenId(p.id);
            toast(said);
          }}
        />
      )}
      {fromMo2 && game && (
        <Mo2Import
          game={game}
          gameName={games.find((g) => g.domain === game)?.name ?? game}
          onClose={() => setFromMo2(false)}
          onDone={(p, said) => {
            load();
            setOpenId(p.id);
            toast(said);
          }}
        />
      )}
      {pasting && (
        <PasteCode
          onClose={() => setPasting(false)}
          onOpened={(r) => {
            load();
            setGame(r.profile.game);
            setOpenId(r.profile.id);
            toast([r.queued ? `Opened “${r.profile.name}”; ${plural(r.queued, "file")} to download` : `Opened “${r.profile.name}”`, r.note].filter(Boolean).join(". "));
          }}
        />
      )}
      {installing && (
        <FomodDialog
          mod={installing.mod}
          fileId={installing.fileId}
          onClose={() => setInstalling(null)}
          onDone={(m) => {
            setMods((ms) => ms.map((x) => (x.game === m.game && x.modId === m.modId ? { ...m, files: m.files.map((f) => ({ ...f, hasInstaller: x.files.find((y) => y.fileId === f.fileId)?.hasInstaller ?? f.hasInstaller })) } : x)));
            toast(`“${m.name}” will go in with the options you picked. Put the mods in the game again to use them.`);
          }}
        />
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
          onCollection={(address) => {
            setMaking(false);
            openAdd(address);
          }}
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
