// Games uninstalled while their mods are still in the library: a window that asks once (per run)
// whether to delete them, and a list on the Games page to answer later.

import { useCallback, useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import { api, type GoneGame } from "../api";
import { bytes, plural } from "../format";
import { Dialog } from "../ui";

const listeners = new Set<(g: GoneGame[]) => void>();
let last: GoneGame[] = [];
/** Look again (reads only this computer). */
export function lookForGone() {
  api.gamesGone().then(
    (g) => {
      last = g;
      listeners.forEach((l) => l(g));
    },
    () => {},
  );
}

export function useGone(): [GoneGame[], () => void] {
  const [gone, setGone] = useState<GoneGame[]>(last);
  useEffect(() => {
    listeners.add(setGone);
    return () => void listeners.delete(setGone);
  }, []);
  return [gone, lookForGone];
}

function Answer({ g, toast, compact, onDone }: { g: GoneGame; toast: (t: string) => void; compact?: boolean; onDone?: () => void }) {
  const [profilesToo, setProfilesToo] = useState(false);
  const [busy, setBusy] = useState(false);
  const answer = async (del: boolean) => {
    setBusy(true);
    try {
      const freed = await api.answerGone(g.domain, del, profilesToo);
      toast(del ? `Deleted ${g.name}'s mods${profilesToo ? " and profiles" : ""}: ${bytes(freed)} freed` : `Kept ${g.name}'s mods. You won't be asked again unless it's installed and removed again.`);
      lookForGone();
      onDone?.();
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className={compact ? "row gap wrap" : "gone-answer"}>
      {g.profiles > 0 && (
        <label className="check">
          <input type="checkbox" checked={profilesToo} onChange={(e) => setProfilesToo(e.target.checked)} /> Delete its {plural(g.profiles, "profile")} too
        </label>
      )}
      <span className="grow" />
      <button className="btn" disabled={busy} onClick={() => answer(false)} title="Keep the mods, for when you install it again">
        Keep the mods
      </button>
      <button className="btn danger solid" disabled={busy} onClick={() => answer(true)}>
        <Trash2 size={14} /> {busy ? "Deleting…" : `Delete ${bytes(g.bytes)}`}
      </button>
    </div>
  );
}

const why = (g: GoneGame) => `${g.name}'s folder is gone from ${g.path.replace(/\/[^/]+$/, "")}, but ${plural(g.mods, "mod")} for it (${bytes(g.bytes)}) are still in your library.`;

/** Asks about the first uninstalled game, once per run. Looks when the app starts and when the window comes back. */
export function GonePrompt({ toast }: { toast: (t: string) => void }) {
  const [gone, look] = useGone();
  const [asked, setAsked] = useState<string[]>([]);
  const lookSoon = useCallback(() => look(), [look]);
  useEffect(() => {
    const t = setTimeout(lookSoon, 4000);
    window.addEventListener("focus", lookSoon);
    return () => {
      clearTimeout(t);
      window.removeEventListener("focus", lookSoon);
    };
  }, [lookSoon]);
  const g = gone.find((x) => !asked.includes(x.domain));
  if (!g) return null;
  const later = () => setAsked((a) => [...a, g.domain]);
  return (
    <Dialog title={`${g.name} was uninstalled`} onClose={later} foot={<Answer g={g} toast={toast} onDone={later} />}>
      <p style={{ marginTop: 0 }}>{why(g)}</p>
      <p className="muted small">Delete them to free the space, or keep them for when you install it again. Closing this asks again next time; the Games page lists it meanwhile.</p>
    </Dialog>
  );
}

/** The Games page's list of uninstalled games with mods left. */
export function GoneList({ toast }: { toast: (t: string) => void }) {
  const [gone, look] = useGone();
  useEffect(look, [look]);
  if (!gone.length) return null;
  return (
    <div className="panel gone-list">
      <h2 style={{ marginTop: 0 }}>Uninstalled games with mods left</h2>
      {gone.map((g) => (
        <div key={g.domain} className="gone-row">
          <p className="small" style={{ margin: "0 0 6px" }}>
            <b>{g.name}</b>: {why(g)}
          </p>
          <Answer g={g} toast={toast} compact />
        </div>
      ))}
    </div>
  );
}
