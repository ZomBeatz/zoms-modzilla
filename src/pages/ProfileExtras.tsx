// Windows on the Profiles page: restore points (History), "load this after that" rules, and
// comparing two profiles side by side.

import { useEffect, useMemo, useState } from "react";
import { ChevronDown, ChevronRight, History as HistoryIcon, RotateCcw } from "lucide-react";
import { api, type Profile, type RestorePoint, type Update } from "../api";
import { plural, when } from "../format";
import { Dialog } from "../ui";

type Namer = (modId: number) => string;

/** How a restore point differs from the profile as it is now. */
function changes(now: Profile | undefined, p: RestorePoint): string {
  if (!now) return "the profile was deleted since";
  const a = new Map(now.mods.map((m) => [m.modId, m]));
  const b = new Map(p.mods.map((m) => [m.modId, m]));
  const added = p.mods.filter((m) => !a.has(m.modId)).length;
  const gone = now.mods.filter((m) => !b.has(m.modId)).length;
  const flipped = p.mods.filter((m) => a.has(m.modId) && (a.get(m.modId)!.enabled !== m.enabled || a.get(m.modId)!.fileId !== m.fileId)).length;
  const moved = p.mods.map((m) => m.modId).filter((id) => a.has(id)).join() !== now.mods.map((m) => m.modId).filter((id) => b.has(id)).join();
  const bits = [added && `${plural(added, "mod")} you took out since`, gone && `without ${plural(gone, "mod")} added since`, flipped && `${plural(flipped, "mod")} switched or on another file`, moved && "another order"].filter(Boolean);
  return bits.length ? bits.join(", ") : "the same as now";
}

/** What changed from the point before (same profile) to this one: "added A, B; took out C". */
export function changelog(p: RestorePoint, before: RestorePoint | undefined, name: Namer): string {
  if (!before) return "The first time noted";
  const was = new Map(before.mods.map((m) => [m.modId, m]));
  const now = new Map(p.mods.map((m) => [m.modId, m]));
  const added = p.mods.filter((m) => !was.has(m.modId)).map((m) => name(m.modId));
  const out = before.mods.filter((m) => !now.has(m.modId)).map((m) => name(m.modId));
  const on = p.mods.filter((m) => was.has(m.modId) && m.enabled && !was.get(m.modId)!.enabled).map((m) => name(m.modId));
  const off = p.mods.filter((m) => was.has(m.modId) && !m.enabled && was.get(m.modId)!.enabled).map((m) => name(m.modId));
  const files = p.mods.filter((m) => was.has(m.modId) && m.fileId !== was.get(m.modId)!.fileId).map((m) => name(m.modId));
  const list = (xs: string[]) => (xs.length > 3 ? `${xs.slice(0, 3).join(", ")} and ${xs.length - 3} more` : xs.join(", "));
  const bits = [added.length && `added ${list(added)}`, out.length && `took out ${list(out)}`, on.length && `switched on ${list(on)}`, off.length && `switched off ${list(off)}`, files.length && `another file for ${list(files)}`].filter(Boolean) as string[];
  if (!bits.length) return before.mods.map((m) => m.modId).join() !== p.mods.map((m) => m.modId).join() ? "Changed the order" : "No change to the mods";
  const t = bits.join("; ");
  return t[0].toUpperCase() + t.slice(1);
}

export function HistoryDialog({ game, gameName, profiles, running, name, onClose, onRestored }: { game: string; gameName: string; profiles: Profile[]; running: boolean; name: Namer; onClose: () => void; onRestored: (p: Profile) => Promise<void> }) {
  const [points, setPoints] = useState<RestorePoint[] | null>(null);
  const [busy, setBusy] = useState<number | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    api.history(game).then(setPoints, (e) => setError((e as Error).message));
  }, [game]);
  return (
    <Dialog title={`${gameName}: restore points`} onClose={onClose} wide foot={<button className="btn" onClick={onClose}>Close</button>}>
      <p className="muted small" style={{ marginTop: 0 }}>
        Each time mods go into the game with a different list, Zom's Modzilla notes it here (the newest 30). Going back puts that profile's list back the way it was and puts the mods in the game again. Mods since deleted from your library are left out.
      </p>
      {error && <p className="err-text">{error}</p>}
      {points?.length === 0 && (
        <div className="empty">
          <HistoryIcon size={28} />
          <p>Nothing yet. The first restore point is noted the next time you put mods in the game.</p>
        </div>
      )}
      <div className="history-list">
        {points?.map((p, i) => {
          const now = profiles.find((x) => x.id === p.profileId);
          const same = changes(now, p) === "the same as now";
          const before = points.slice(i + 1).find((x) => x.profileId === p.profileId);
          return (
            <div key={p.at} className="history-row">
              <div className="grow" style={{ minWidth: 0 }}>
                <b>{p.profileName}</b> <span className="muted small">· {when(p.at)}</span>
                {i === 0 && <span className="pill tiny-pill">Newest</span>}
                <div className="small">{changelog(p, before, name)}</div>
                <div className="muted small">
                  {plural(p.mods.filter((m) => m.enabled).length, "mod")} on · {changes(now, p)}
                </div>
                <input
                  className="history-note"
                  placeholder="Why? (a note for later)"
                  defaultValue={p.note ?? ""}
                  aria-label="Note on this change"
                  onBlur={(e) => {
                    const v = e.target.value.trim();
                    if (v !== (p.note ?? "")) void api.historyNote(game, p.at, v).then(() => setPoints((ps) => ps?.map((x) => (x.at === p.at ? { ...x, note: v } : x)) ?? null), (err) => setError((err as Error).message));
                  }}
                />
              </div>
              <button
                className="btn sm"
                disabled={busy !== null || same || running}
                title={running ? "Quit the game first: mods can't move while it's running." : same ? "This profile is already like this" : "Put this profile back the way it was, and put it in the game"}
                onClick={async () => {
                  setBusy(p.at);
                  setError("");
                  try {
                    await onRestored(await api.restore(game, p.at));
                    onClose();
                  } catch (e) {
                    setError((e as Error).message);
                  } finally {
                    setBusy(null);
                  }
                }}
              >
                <RotateCcw size={14} /> {busy === p.at ? "Going back…" : "Go back to this"}
              </button>
            </div>
          );
        })}
      </div>
    </Dialog>
  );
}

export function RuleDialog({ profile, modId, name, onClose, onSave }: { profile: Profile; modId: number; name: Namer; onClose: () => void; onSave: (p: Profile) => void }) {
  const others = profile.mods.filter((m) => m.modId !== modId);
  const [after, setAfter] = useState<number>(others[0]?.modId ?? 0);
  const mine = (profile.rules ?? []).filter((r) => r.modId === modId);
  return (
    <Dialog
      title={`Load “${name(modId)}” after…`}
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn primary"
            disabled={!after || mine.some((r) => r.after === after)}
            onClick={() => {
              onSave({ ...profile, rules: [...(profile.rules ?? []), { modId, after }] });
              onClose();
            }}
          >
            Add rule
          </button>
        </>
      }
    >
      <p className="muted small" style={{ marginTop: 0 }}>
        A rule keeps this mod below the other one in the list, so it wins when both have the same file. Zom's Modzilla keeps it whenever the order changes, moving as little as it can.
      </p>
      <label className="field">
        <span>After</span>
        <select value={after} onChange={(e) => setAfter(Number(e.target.value))}>
          {others.map((m) => (
            <option key={m.modId} value={m.modId}>
              {name(m.modId)}
            </option>
          ))}
        </select>
      </label>
      {mine.length > 0 && <p className="small">Already loads after: {mine.map((r) => name(r.after)).join(", ")}.</p>}
    </Dialog>
  );
}

export function CompareDialog({ a, profiles, name, onClose }: { a: Profile; profiles: Profile[]; name: Namer; onClose: () => void }) {
  const others = profiles.filter((p) => p.game === a.game && p.id !== a.id);
  const [bId, setBId] = useState(others[0]?.id ?? "");
  const b = others.find((p) => p.id === bId);
  const rows = useMemo(() => {
    if (!b) return null;
    const inA = new Map(a.mods.map((m) => [m.modId, m]));
    const inB = new Map(b.mods.map((m) => [m.modId, m]));
    const onlyA = a.mods.filter((m) => m.enabled && !inB.get(m.modId)?.enabled).map((m) => m.modId);
    const onlyB = b.mods.filter((m) => m.enabled && !inA.get(m.modId)?.enabled).map((m) => m.modId);
    const files = a.mods.filter((m) => inB.has(m.modId) && inB.get(m.modId)!.fileId !== m.fileId).map((m) => m.modId);
    const both = a.mods.filter((m) => m.enabled && inB.get(m.modId)?.enabled).map((m) => m.modId);
    const orderB = new Map(b.mods.filter((m) => both.includes(m.modId)).map((m, i) => [m.modId, i]));
    const moved = both.filter((id, i) => orderB.get(id) !== i);
    return { onlyA, onlyB, files, both: both.length, moved };
  }, [a, b]);
  const list = (ids: number[]) => (ids.length ? <ul className="compare-list">{ids.map((id) => <li key={id}>{name(id)}</li>)}</ul> : <p className="muted small">None</p>);
  return (
    <Dialog title="Compare profiles" wide onClose={onClose} foot={<button className="btn" onClick={onClose}>Close</button>}>
      {others.length === 0 ? (
        <p className="muted">This game has no other profile to compare with.</p>
      ) : (
        <>
          <div className="row gap wrap" style={{ marginBottom: 12 }}>
            <b>{a.name}</b>
            <span className="muted">and</span>
            <select value={bId} onChange={(e) => setBId(e.target.value)} aria-label="Profile to compare with">
              {others.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>
          {rows && b && (
            <>
              <p className="small muted">
                {plural(rows.both, "mod")} on in both{rows.moved.length ? `, ${plural(rows.moved.length, "mod")} in another order` : ", in the same order"}.
              </p>
              <div className="compare">
                <div>
                  <h3>Only on in “{a.name}”</h3>
                  {list(rows.onlyA)}
                </div>
                <div>
                  <h3>Only on in “{b.name}”</h3>
                  {list(rows.onlyB)}
                </div>
              </div>
              {rows.files.length > 0 && (
                <>
                  <h3>Another file of the same mod</h3>
                  {list(rows.files)}
                </>
              )}
            </>
          )}
        </>
      )}
    </Dialog>
  );
}

/** The notes for one update: the new file's own description, and on request the mod's changelog
 * (versions newer than yours first). */
function WhatsNew({ u, have }: { u: Update; have?: string | null }) {
  const [log, setLog] = useState<{ version: string; notes: string[] }[] | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const desc = (u.newer?.description ?? "").replace(/<[^>]+>/g, " ").replace(/\s+/g, " ").trim();
  const newer = log?.filter((c) => !have || c.version.replace(/^v/i, "") !== have.replace(/^v/i, "")) ?? [];
  const cut = have ? newer.findIndex((c) => c.version.replace(/^v/i, "") === have.replace(/^v/i, "")) : -1;
  return (
    <div className="whats-new small">
      {desc && <p className="muted" style={{ margin: "4px 0" }}>{desc.length > 400 ? `${desc.slice(0, 400)}…` : desc}</p>}
      {log === null ? (
        <button
          className="btn ghost sm"
          disabled={busy}
          title="Reads the mod's changelog on Nexus Mods (one request)"
          onClick={async () => {
            setBusy(true);
            try {
              setLog(await api.changelog(u.game, u.modId));
            } catch (e) {
              setError((e as Error).message);
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy ? "Reading…" : "What's new?"}
        </button>
      ) : log.length === 0 ? (
        <p className="muted">The author didn't write a changelog.</p>
      ) : (
        <div className="changelog">
          {(cut > 0 ? newer.slice(0, cut) : newer.slice(0, 5)).map((c) => (
            <div key={c.version}>
              <b>{c.version}</b>
              <ul>
                {c.notes.map((n, i) => (
                  <li key={i}>{n}</li>
                ))}
              </ul>
            </div>
          ))}
        </div>
      )}
      {error && <p className="err-text">{error}</p>}
    </div>
  );
}

/** Updates for a profile, to read and pick before any download starts. */
export function UpdatesDialog({ profile, updates, version, name, onClose, onUpdate }: { profile: Profile; updates: Update[]; version: (modId: number, fileId: number) => string | null | undefined; name: (id: number) => string; onClose: () => void; onUpdate: (picked: Update[]) => Promise<void> }) {
  const [picked, setPicked] = useState<Set<number>>(new Set(updates.map((u) => u.modId)));
  const [open, setOpen] = useState<Record<number, boolean>>({});
  const [busy, setBusy] = useState(false);
  return (
    <Dialog
      wide
      title={`Updates for “${profile.name}”`}
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Not now
          </button>
          <button
            className="btn primary"
            disabled={!picked.size || busy}
            onClick={async () => {
              setBusy(true);
              try {
                await onUpdate(updates.filter((u) => picked.has(u.modId)));
                onClose();
              } finally {
                setBusy(false);
              }
            }}
          >
            {busy ? "Adding…" : `Update ${plural(picked.size, "mod")}`}
          </button>
        </>
      }
    >
      <p className="muted small" style={{ marginTop: 0 }}>
        Untick any you'd rather keep as they are. Each picked update downloads, then takes the old file's place in this profile. Read what's new first with each mod's arrow.
      </p>
      <div className="update-list">
        {updates.map((u) => (
          <div key={u.modId} className="update-row">
            <div className="row gap">
              <input type="checkbox" aria-label={`Update ${name(u.modId)}`} checked={picked.has(u.modId)} onChange={(e) => setPicked((p) => { const n = new Set(p); if (e.target.checked) n.add(u.modId); else n.delete(u.modId); return n; })} />
              <button className="icon-btn" aria-label={open[u.modId] ? "Hide what's new" : "Show what's new"} onClick={() => setOpen((o) => ({ ...o, [u.modId]: !o[u.modId] }))}>
                {open[u.modId] ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
              </button>
              <b className="grow ellipsis">{u.modName || name(u.modId)}</b>
              <span className="muted small num">
                {version(u.modId, u.fileId) ?? "yours"} → {u.newer?.version ?? u.newer?.name}
              </span>
            </div>
            {open[u.modId] && <WhatsNew u={u} have={version(u.modId, u.fileId)} />}
          </div>
        ))}
      </div>
    </Dialog>
  );
}
