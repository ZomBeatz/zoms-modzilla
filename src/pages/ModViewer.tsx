import { useEffect, useState } from "react";
import { FileText, Image as ImageIcon, File } from "lucide-react";
import { api, type ModFileEntry, type Profile, type StoredMod } from "../api";
import { bytes } from "../format";
import { Dialog } from "../ui";

/** Look inside a mod: its files, where each goes, which lose to a later mod in the profile, and
 * its pictures and text files (readmes, ini, json). */
export function ModViewer({ mod, profiles, onClose }: { mod: StoredMod; profiles: Profile[]; onClose: () => void }) {
  const mine = profiles.filter((p) => p.game === mod.game && p.mods.some((x) => x.modId === mod.modId));
  const [profile, setProfile] = useState(mine[0]?.id ?? "");
  const [files, setFiles] = useState<ModFileEntry[] | null>(null);
  const [error, setError] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [look, setLook] = useState<{ text?: string | null; image?: string | null; cut: boolean } | null>(null);
  const [filter, setFilter] = useState("");
  useEffect(() => {
    setFiles(null);
    setError("");
    void api.modContents(mod.game, mod.modId, profile || null).then(setFiles, (e) => setError((e as Error).message));
  }, [mod.game, mod.modId, profile]);
  useEffect(() => {
    if (!open) return setLook(null);
    void api.modFileRead(mod.game, mod.modId, open, profile || null).then(setLook, (e) => setLook({ text: (e as Error).message, cut: false }));
  }, [open, mod.game, mod.modId, profile]);
  // A readme first, when there is one.
  useEffect(() => {
    const readme = files?.find((f) => /readme|read me|description/i.test(f.path) && f.kind === "text") ?? files?.find((f) => f.kind === "image");
    if (readme && !open) setOpen(readme.path);
  }, [files]); // eslint-disable-line react-hooks/exhaustive-deps
  const shown = (files ?? []).filter((f) => !filter || f.path.toLowerCase().includes(filter.toLowerCase()));
  const lost = (files ?? []).filter((f) => f.lostTo).length;
  return (
    <Dialog title={`Inside ${mod.name}`} wide onClose={onClose} foot={<button className="btn" onClick={onClose}>Close</button>}>
      <div className="row gap wrap" style={{ marginBottom: 8 }}>
        <input placeholder="Find a file" value={filter} onChange={(e) => setFilter(e.target.value)} />
        {mine.length > 0 && (
          <label className="row gap-sm small">
            Clashes in
            <select value={profile} onChange={(e) => setProfile(e.target.value)}>
              {mine.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </label>
        )}
        {files && (
          <span className="muted small">
            {files.length} files{lost ? ` · ${lost} lose to a later mod` : ""}
          </span>
        )}
      </div>
      {error && <p className="banner warn small">{error}</p>}
      <div className="viewer">
        <div className="viewer-files">
          {!files && !error && <p className="muted">Reading…</p>}
          {shown.slice(0, 2000).map((f) => (
            <button key={f.path} className={`viewer-file${open === f.path ? " active" : ""}`} disabled={f.kind === "other"} onClick={() => setOpen(f.path)} title={f.lostTo ? `${f.path}: ${f.lostTo} (later in the profile) is used instead` : f.path}>
              {f.kind === "text" ? <FileText size={13} /> : f.kind === "image" ? <ImageIcon size={13} /> : <File size={13} />}
              <span className={`ellipsis grow${f.lostTo ? " lost" : ""}`}>{f.path}</span>
              {f.lostTo && <span className="pill warn">{f.lostTo} wins</span>}
              <span className="num muted small">{bytes(f.size)}</span>
            </button>
          ))}
        </div>
        <div className="viewer-look">
          {!look && <p className="muted small">Pick a picture or text file to look at it.</p>}
          {look?.image && <img src={look.image} alt={open ?? ""} />}
          {look?.text != null && (
            <pre>
              {look.text}
              {look.cut ? "\n…" : ""}
            </pre>
          )}
        </div>
      </div>
    </Dialog>
  );
}
