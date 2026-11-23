// Bring a Mod Organizer 2 setup across: pick its folder, pick one of its profiles, and it becomes a
// profile here, in the same order, with the mods as MO2 installed them. MO2's folder isn't changed.

import { useState } from "react";
import { FolderOpen } from "lucide-react";
import { api, pickFolder, type Mo2Found, type Profile } from "../api";
import { bytes, plural } from "../format";
import { Dialog } from "../ui";

export function Mo2Import({ game, gameName, onClose, onDone }: { game: string; gameName: string; onClose: () => void; onDone: (p: Profile, said: string) => void }) {
  const [found, setFound] = useState<Mo2Found | null>(null);
  const [pick, setPick] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const choose = async () => {
    const path = await pickFolder("Mod Organizer 2's folder (where ModOrganizer.ini is)");
    if (!path) return;
    setError("");
    try {
      const f = await api.mo2Look(path);
      setFound(f);
      setPick(f.profiles.includes("Default") ? "Default" : f.profiles[0]);
    } catch (e) {
      setError((e as Error).message);
    }
  };
  const bring = async () => {
    if (!found) return;
    setBusy(true);
    try {
      const r = await api.mo2Import(game, found.folder, pick);
      const said = [`Brought “${pick}” across as “${r.profile.name}”`, r.added ? `${plural(r.added, "mod")} new in the library${r.bytesCopied ? ` (${bytes(r.bytesCopied)} copied; the rest share MO2's files)` : ", sharing MO2's files (no extra space)"}` : "", r.skipped.length ? `${plural(r.skipped.length, "mod")} MO2 lists aren't in its mods folder: ${r.skipped.slice(0, 3).join(", ")}` : ""].filter(Boolean).join(". ");
      onDone(r.profile, said);
      onClose();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Dialog
      title="Bring across from Mod Organizer 2"
      onClose={onClose}
      foot={
        <>
          {error && <span className="err-text small left">{error}</span>}
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" disabled={!found || !pick || busy} onClick={bring}>
            {busy ? "Bringing it across…" : "Bring it across"}
          </button>
        </>
      }
    >
      <p style={{ marginTop: 0 }}>
        A Mod Organizer 2 profile becomes a profile for <b>{gameName}</b> here: the same mods in the same order, switched on and off the same way, with the files MO2 installed (your installer choices included). Nothing is downloaded again and MO2's folder isn't changed.
      </p>
      <button className="btn" onClick={choose}>
        <FolderOpen size={15} /> {found ? "Pick another folder…" : "Pick MO2's folder…"}
      </button>
      {found && (
        <div style={{ marginTop: 12 }}>
          <p className="muted small">
            {found.folder} · {plural(found.mods, "mod")}
            {found.gameName ? ` · for ${found.gameName}` : ""}
          </p>
          <label className="setting">
            <span className="what">
              <b>Which profile</b>
              <span>Its mod list and order come across.</span>
            </span>
            <select value={pick} onChange={(e) => setPick(e.target.value)}>
              {found.profiles.map((p) => (
                <option key={p}>{p}</option>
              ))}
            </select>
          </label>
          <p className="muted small">Mods from Nexus Mods keep their numbers, so updates and requirements work as usual. Mods MO2 doesn't know a number for come in as your own mods. The Bethesda plugin order follows the mod order; check it after.</p>
        </div>
      )}
    </Dialog>
  );
}
