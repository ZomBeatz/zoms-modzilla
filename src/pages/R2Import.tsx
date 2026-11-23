// Bring an r2modman profile across: paste its profile code (r2modman › Export profile as code),
// or pick one of r2modman's profiles on this computer. Its mods come at the same versions, and
// its config files become the new profile's own settings.

import { useEffect, useState } from "react";
import { api, on, type Profile } from "../api";
import { plural } from "../format";
import { Dialog } from "../ui";

export function R2Import({ game, gameName, onClose, onDone }: { game: string; gameName: string; onClose: () => void; onDone: (p: Profile, said: string) => void }) {
  const [code, setCode] = useState("");
  const [local, setLocal] = useState<{ name: string; path: string }[]>([]);
  const [busy, setBusy] = useState("");
  const [error, setError] = useState("");
  useEffect(() => {
    api.r2Profiles(game).then(setLocal, () => {});
    const off = on<{ name: string; at: number; of: number }>("thunderstore:progress", (p) => setBusy(`${p.at} of ${p.of}: ${p.name}`));
    return () => void off.then((f) => f());
  }, [game]);
  const bring = async (what: { code?: string; folder?: string }) => {
    setBusy("Starting…");
    setError("");
    try {
      const r = await api.r2Import(game, what.code ?? null, what.folder ?? null);
      const said = [`Made “${r.profile?.name}”`, r.added.length ? `${plural(r.added.length, "mod")} downloaded` : "", r.had ? `${r.had} you had` : "", r.missing.length ? `Thunderstore doesn't have ${r.missing.join(", ")}` : ""].filter(Boolean).join(" · ");
      if (r.profile) onDone(r.profile, said);
      onClose();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy("");
    }
  };
  return (
    <Dialog
      title="Bring across from r2modman"
      onClose={onClose}
      foot={
        <>
          {error && <span className="err-text small left">{error}</span>}
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" disabled={!code.trim() || !!busy} onClick={() => void bring({ code: code.trim() })}>
            {busy ? "Bringing it across…" : "Use this code"}
          </button>
        </>
      }
    >
      <p style={{ marginTop: 0 }}>
        An r2modman (or Thunderstore Mod Manager) profile becomes a profile for <b>{gameName}</b>: the same mods at the same versions, switched on and off the same way, with its config files as the profile's own settings.
      </p>
      <label className="setting">
        <span className="what">
          <b>Profile code</b>
          <span>In r2modman: Settings › Profile › Export profile as code. Codes expire after a while.</span>
        </span>
        <input value={code} onChange={(e) => setCode(e.target.value)} placeholder="0193a1b2-…" aria-label="Profile code" />
      </label>
      {local.length > 0 && (
        <>
          <p className="small muted">Or one of r2modman's profiles on this computer:</p>
          <div className="pick-list">
            {local.map((l) => (
              <button key={l.path} className="btn" disabled={!!busy} onClick={() => void bring({ folder: l.path })}>
                {l.name}
              </button>
            ))}
          </div>
        </>
      )}
      {busy && <p className="muted small">{busy}</p>}
    </Dialog>
  );
}
