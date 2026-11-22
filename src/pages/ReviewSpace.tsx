// "Review space": what in the library takes room without being used, so it can be cleared.
//
// 1. Mods no profile uses. 2. Old files: other versions of a mod the profiles don't use.
// 3. Archives kept next to their unpacked folder (profiles use the folder).

import { useEffect, useMemo, useState } from "react";
import { Archive, FileClock, HardDrive, PackageX } from "lucide-react";
import { api, type FileUsage, type Profile, type StoredMod } from "../api";
import { bytes, plural } from "../format";
import { Dialog } from "../ui";

type Kind = "unused" | "old" | "archive";
interface Item {
  key: string;
  kind: Kind;
  mod: StoredMod;
  fileId?: number;
  label: string;
  detail: string;
  size: number;
}

export const usedKeys = (profiles: Profile[]) => ({
  mods: new Set(profiles.flatMap((p) => p.mods.map((m) => `${p.game}:${m.modId}`))),
  files: new Set(profiles.flatMap((p) => p.mods.map((m) => `${p.game}:${m.modId}:${m.fileId}`))),
});

/** The reclaimable space in these mods, grouped. */
export function findItems(mods: StoredMod[], profiles: Profile[], usage: FileUsage[]): Item[] {
  const used = usedKeys(profiles);
  const size = (m: StoredMod, fileId: number) => usage.find((u) => u.game === m.game && u.modId === m.modId && u.fileId === fileId);
  const out: Item[] = [];
  for (const m of mods) {
    if (!used.mods.has(`${m.game}:${m.modId}`)) {
      const total = m.files.reduce((n, f) => n + ((size(m, f.fileId)?.archive ?? 0) + (size(m, f.fileId)?.unpacked ?? 0) || f.size), 0);
      out.push({ key: `u:${m.game}:${m.modId}`, kind: "unused", mod: m, label: m.name, detail: `${plural(m.files.length, "file")}${m.local ? " · from your files" : ""}`, size: total });
      continue;
    }
    for (const f of m.files) {
      const u = size(m, f.fileId);
      if (!used.files.has(`${m.game}:${m.modId}:${f.fileId}`)) {
        out.push({ key: `o:${m.game}:${m.modId}:${f.fileId}`, kind: "old", mod: m, fileId: f.fileId, label: m.name, detail: `${f.name}${f.version ? ` · ${f.version}` : ""}`, size: (u?.archive ?? 0) + (u?.unpacked ?? 0) || f.size });
      } else if (f.unpacked && f.archive && (u?.archive ?? 0) > 0) {
        out.push({ key: `a:${m.game}:${m.modId}:${f.fileId}`, kind: "archive", mod: m, fileId: f.fileId, label: m.name, detail: f.fileName, size: u!.archive });
      }
    }
  }
  return out.sort((a, b) => b.size - a.size);
}

const GROUPS: [Kind, string, string, typeof Archive][] = [
  ["unused", "Mods not in any profile", "Downloaded, but no profile uses them. You can download them again from Nexus Mods.", PackageX],
  ["old", "Old files", "Other versions of mods your profiles use: the profiles use a different file.", FileClock],
  ["archive", "Archives kept after unpacking", "The original zip or 7z next to its unpacked folder. Profiles use the folder, so the mod keeps working.", Archive],
];

export function ReviewSpace({ mods, onClose, onDone }: { mods: StoredMod[]; onClose: () => void; onDone: (text: string) => void }) {
  const [profiles, setProfiles] = useState<Profile[] | null>(null);
  const [usage, setUsage] = useState<FileUsage[] | null>(null);
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  useEffect(() => {
    api.profiles().then(setProfiles, (e) => setError((e as Error).message));
    api.usage().then(setUsage, (e) => setError((e as Error).message));
  }, []);
  const items = useMemo(() => (profiles && usage ? findItems(mods, profiles, usage) : []), [mods, profiles, usage]);
  const chosen = items.filter((i) => picked.has(i.key));
  const total = chosen.reduce((n, i) => n + i.size, 0);
  const flip = (keys: string[], on: boolean) => {
    const n = new Set(picked);
    keys.forEach((k) => (on ? n.add(k) : n.delete(k)));
    setPicked(n);
  };

  const clear = async () => {
    setBusy(true);
    setError("");
    let freed = 0;
    const failed: string[] = [];
    for (const i of chosen) {
      try {
        if (i.kind === "unused") await api.removeMod(i.mod.game, i.mod.modId);
        else if (i.kind === "old") await api.removeFile(i.mod.game, i.mod.modId, i.fileId!);
        else await api.dropArchive(i.mod.game, i.mod.modId, i.fileId!);
        freed += i.size;
      } catch (e) {
        failed.push(`${i.label}: ${(e as Error).message}`);
      }
    }
    setBusy(false);
    onDone(`Freed ${bytes(freed)}${failed.length ? `. Couldn't clear ${failed.length}: ${failed[0]}` : ""}`);
    onClose();
  };

  return (
    <Dialog
      wide
      title="Review space"
      onClose={onClose}
      foot={
        <>
          <span className="left muted small num">{chosen.length ? `${plural(chosen.length, "item")} · frees ${bytes(total)}` : "Tick what to clear"}</span>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn danger" disabled={busy || chosen.length === 0} onClick={clear}>
            {busy ? "Clearing…" : `Delete ${chosen.length ? bytes(total) : ""}`}
          </button>
        </>
      }
    >
      {error && <p className="err-text">{error}</p>}
      {profiles === null || usage === null ? (
        <p className="muted">Measuring the library…</p>
      ) : items.length === 0 ? (
        <div className="empty">
          <HardDrive size={28} />
          <p>Nothing to clear: every mod here is in a profile, and there are no old files or spare archives.</p>
        </div>
      ) : (
        GROUPS.map(([kind, title, hint, Icon]) => {
          const group = items.filter((i) => i.kind === kind);
          if (!group.length) return null;
          const all = group.every((i) => picked.has(i.key));
          return (
            <div key={kind} className="review-group">
              <label className="review-head">
                <input type="checkbox" checked={all} onChange={(e) => flip(group.map((i) => i.key), e.target.checked)} />
                <Icon size={15} />
                <b className="grow">{title}</b>
                <span className="muted small num">
                  {plural(group.length, "item")} · {bytes(group.reduce((n, i) => n + i.size, 0))}
                </span>
              </label>
              <p className="muted small" style={{ margin: "2px 0 8px 26px" }}>
                {hint}
              </p>
              <div className="pick-list">
                {group.map((i) => (
                  <label key={i.key} className="pick-item">
                    <input type="checkbox" checked={picked.has(i.key)} onChange={(e) => flip([i.key], e.target.checked)} />
                    <div className="grow" style={{ minWidth: 0 }}>
                      <b className="ellipsis">{i.label}</b>
                      <div className="meta ellipsis">{i.detail}</div>
                    </div>
                    <span className="num small">{bytes(i.size)}</span>
                  </label>
                ))}
              </div>
            </div>
          );
        })
      )}
    </Dialog>
  );
}
