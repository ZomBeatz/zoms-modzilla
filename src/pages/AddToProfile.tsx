import { useEffect, useState } from "react";
import { api, type Profile } from "../api";
import { Dialog } from "../ui";

/** Put one downloaded file into a profile of its game: an existing one, or a new one. */
export function AddToProfile({ game, gameName, modId, fileId, modName, onClose, onDone }: { game: string; gameName: string; modId: number; fileId: number; modName: string; onClose: () => void; onDone: (text: string) => void }) {
  const [profiles, setProfiles] = useState<Profile[] | null>(null);
  const [pick, setPick] = useState<string>("");
  const [name, setName] = useState("");
  const [error, setError] = useState("");

  useEffect(() => {
    api.profiles().then((all) => {
      const mine = all.filter((p) => p.game === game);
      setProfiles(mine);
      setPick(mine[0]?.id ?? "new");
    }, (e) => setError((e as Error).message));
  }, [game]);

  const target = profiles?.find((p) => p.id === pick);
  const already = target?.mods.some((m) => m.modId === modId && m.fileId === fileId);

  const save = async () => {
    try {
      const entry = { modId, fileId, enabled: true };
      if (pick === "new") {
        const p = await api.saveProfile({ id: "", name: name.trim(), game, mods: [entry] });
        const r = await api.resolveProfile(p.id).catch(() => null);
        onDone(`Made the profile “${p.name}” with “${modName}”${r?.added.length ? ` and what it needs (${r.added.join(", ")})` : ""}`);
      } else if (target) {
        await api.saveProfile({ ...target, mods: [...target.mods.filter((m) => !(m.modId === modId && m.fileId === fileId)), entry] });
        const r = await api.resolveProfile(target.id).catch(() => null);
        onDone(`Added “${modName}” to “${target.name}”${r?.added.length ? `, with what it needs (${r.added.join(", ")})` : ""}`);
      }
      onClose();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <Dialog
      title="Add to a profile"
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" disabled={profiles === null || (pick === "new" && !name.trim()) || !!already} onClick={save}>
            {pick === "new" ? "Make profile" : "Add"}
          </button>
        </>
      }
    >
      <p className="muted" style={{ marginTop: 0 }}>
        “{modName}” goes at the end of the list, so it wins when it has the same files as a mod above it.
      </p>
      <div className="form-grid">
        <label className="field full">
          <span>{gameName} profile</span>
          <select value={pick} onChange={(e) => setPick(e.target.value)} disabled={profiles === null}>
            {(profiles ?? []).map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
            <option value="new">New profile…</option>
          </select>
        </label>
        {pick === "new" && (
          <label className="field full">
            <span>Name</span>
            <input autoFocus placeholder="My mods" value={name} onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && name.trim() && void save()} />
          </label>
        )}
        {already && <span className="muted full">It's already in this profile.</span>}
        {error && <span className="err full">{error}</span>}
      </div>
    </Dialog>
  );
}
