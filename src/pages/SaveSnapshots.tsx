// Save snapshots for a game: a copy of its saves before trying risky mods, put back in a click.
// Restoring keeps the saves there now (snapshotted and moved aside), so nothing is ever lost.

import { useEffect, useState } from "react";
import { Camera, RotateCcw, Trash2 } from "lucide-react";
import { api, type Snapshot } from "../api";
import { bytes, plural, when } from "../format";
import { Dialog } from "../ui";

export function SaveSnapshots({ game, gameName, onClose, toast }: { game: string; gameName: string; onClose: () => void; toast: (t: string) => void }) {
  const [list, setList] = useState<Snapshot[] | null>(null);
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState("");
  const [sure, setSure] = useState<Snapshot | null>(null);
  const [auto, setAuto] = useState(false);
  const load = () => api.snapshots(game).then(setList, () => setList([]));
  useEffect(() => {
    void load();
    api.overview().then((o) => setAuto(!!o.settings.snapshotBeforePlay), () => {});
  }, [game]); // eslint-disable-line react-hooks/exhaustive-deps
  const run = async (what: string, f: () => Promise<unknown>, said: string) => {
    setBusy(what);
    try {
      await f();
      toast(said);
      await load();
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setBusy("");
    }
  };
  return (
    <Dialog wide title={`Save snapshots · ${gameName}`} onClose={onClose}>
      <p style={{ marginTop: 0 }}>A copy of the game's saves as they are now, to put back if a mod breaks them. Restoring keeps the saves there now too (they're snapshotted and moved aside), so nothing is lost.</p>
      <form
        className="row gap-sm"
        onSubmit={(e) => {
          e.preventDefault();
          void run("take", () => api.takeSnapshot(game, label), "Snapshot taken").then(() => setLabel(""));
        }}
      >
        <input className="grow" value={label} onChange={(e) => setLabel(e.target.value)} placeholder="What it's before (optional): trying Big Overhaul…" aria-label="Snapshot name" />
        <button className="btn primary" disabled={!!busy}>
          <Camera size={15} /> {busy === "take" ? "Taking…" : "Take a snapshot"}
        </button>
      </form>
      <label className="check" style={{ marginTop: 10 }}>
        <input
          type="checkbox"
          checked={auto}
          onChange={async (e) => {
            const on = e.target.checked;
            setAuto(on);
            try {
              const o = await api.overview();
              await api.saveSettings({ ...o.settings, snapshotBeforePlay: on });
            } catch (err) {
              toast((err as Error).message);
            }
          }}
        />{" "}
        Take one by itself before each Play (for every game; the last 10 are kept)
      </label>
      <div className="snap-list">
        {list === null ? (
          <p className="muted">Looking…</p>
        ) : list.length === 0 ? (
          <p className="muted">No snapshots yet.</p>
        ) : (
          list.map((s) => (
            <div key={s.id} className="snap-row">
              <div className="grow">
                <b>{s.label}</b> {s.auto && <span className="pill tiny-pill">by itself</span>}
                <div className="muted small">
                  {when(s.at)} · {plural(s.files, "file")} · {bytes(s.size)}
                  {s.owner !== "shared" ? " · a profile's own saves" : ""}
                </div>
              </div>
              <button className="btn sm" disabled={!!busy} onClick={() => setSure(s)}>
                <RotateCcw size={13} /> Restore
              </button>
              <button className="icon-btn" title="Delete this snapshot" aria-label="Delete this snapshot" disabled={!!busy} onClick={() => void run(`del${s.id}`, () => api.deleteSnapshot(game, s.id), "Snapshot deleted")}>
                <Trash2 size={14} />
              </button>
            </div>
          ))
        )}
      </div>
      {sure && (
        <Dialog
          title="Put these saves back?"
          onClose={() => setSure(null)}
          foot={
            <>
              <button className="btn" onClick={() => setSure(null)}>
                Cancel
              </button>
              <button
                className="btn primary"
                onClick={() => {
                  const s = sure;
                  setSure(null);
                  void run("restore", () => api.restoreSnapshot(game, s.id), `Saves from “${s.label}” are back. The ones before are kept as a snapshot too.`);
                }}
              >
                Restore
              </button>
            </>
          }
        >
          <p style={{ marginTop: 0 }}>
            The saves go back to “{sure.label}” ({when(sure.at)}). The saves there now are snapshotted first and moved aside, not deleted.
          </p>
        </Dialog>
      )}
    </Dialog>
  );
}
