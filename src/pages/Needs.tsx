// What a download's mod needs (as listed on Nexus Mods), and a way to get what's missing.

import { useState } from "react";
import { Check, Download, ExternalLink, Puzzle } from "lucide-react";
import { api, modUrl, openUrl, type Job, type ModFacts, type StoredMod } from "../api";
import { plural } from "../format";
import { Dialog } from "../ui";

export type Have = "library" | "downloads" | "missing" | "elsewhere";

/** Where each requirement stands: in the library, in Downloads, missing, or not checkable. */
export function standing(f: ModFacts, library: StoredMod[], jobs: Job[]) {
  return f.requires.map((r) => {
    let have: Have = "missing";
    if (r.modId === 0 || r.otherGame) have = "elsewhere";
    else if (library.some((m) => m.game === f.game && m.modId === r.modId)) have = "library";
    else if (jobs.some((j) => j.game === f.game && j.modId === r.modId && j.status !== "done")) have = "downloads";
    return { ...r, have };
  });
}

/** The line under a download's name: "Needs 5 mods · 3 not downloaded". */
export function NeedsLine({ facts, library, jobs, onOpen }: { facts: ModFacts; library: StoredMod[]; jobs: Job[]; onOpen: () => void }) {
  if (!facts.requires.length && !facts.dlc.length) return null;
  const s = standing(facts, library, jobs);
  const missing = s.filter((r) => r.have === "missing").length;
  const coming = s.filter((r) => r.have === "downloads").length;
  return (
    <button className={`needs-line${missing ? " warn-text" : ""}`} onClick={onOpen} title="What this mod needs, from its page on Nexus Mods">
      <Puzzle size={12} />
      {facts.requires.length ? `Needs ${plural(facts.requires.length, "mod")}` : ""}
      {facts.dlc.length ? `${facts.requires.length ? " and " : "Needs "}${facts.dlc.join(", ")}` : ""}
      {missing ? ` · ${missing} not downloaded` : ""}
      {coming ? ` · ${coming} downloading` : ""}
      {!missing && !coming ? " · all in your library" : ""}
    </button>
  );
}

export function NeedsDialog({ facts, library, jobs, onClose, toast }: { facts: ModFacts; library: StoredMod[]; jobs: Job[]; onClose: () => void; toast: (t: string) => void }) {
  const [busy, setBusy] = useState(false);
  const s = standing(facts, library, jobs);
  const missing = s.filter((r) => r.have === "missing");
  const get = async () => {
    setBusy(true);
    try {
      const r = await api.queueRequirements(facts.game, missing.map((m) => m.modId));
      toast(`Added ${plural(r.queued.length, "file")} to downloads${r.skipped.length ? `; couldn't add ${r.skipped.join(", ")}` : ""}`);
      onClose();
    } catch (e) {
      toast((e as Error).message);
      setBusy(false);
    }
  };
  return (
    <Dialog
      title={`What “${facts.name}” needs`}
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Close
          </button>
          {missing.length > 0 && (
            <button className="btn primary" disabled={busy} onClick={get} title="Queues the main, newest file of each. Check the notes for versions the mod asks for.">
              <Download size={15} /> {busy ? "Adding…" : `Download the ${plural(missing.length, "missing mod")}`}
            </button>
          )}
        </>
      }
    >
      <p className="muted" style={{ marginTop: 0 }}>
        From the mod's page on Nexus Mods. Put what it needs in the same profile; the conflict check tells you if something's missing.
      </p>
      <div className="pick-list">
        {s.map((r) => (
          <div key={r.modId + r.name} className="pick-item" style={{ cursor: "default" }}>
            <div className="grow">
              <div className="row gap-sm wrap">
                <b>{r.name}</b>
                {r.have === "library" && (
                  <span className="pill ok">
                    <Check size={11} /> In your library
                  </span>
                )}
                {r.have === "downloads" && <span className="pill accent">In downloads</span>}
                {r.have === "missing" && <span className="pill warn">Not downloaded</span>}
                {r.have === "elsewhere" && <span className="pill">{r.modId === 0 ? "From another site" : "A tool on Nexus Mods"}</span>}
              </div>
              {r.notes && <div className="meta">{r.notes}</div>}
            </div>
            {(r.url || r.modId > 0) && (
              <button className="btn sm ghost" onClick={() => void openUrl(r.url || modUrl(facts.game, r.modId))}>
                <ExternalLink size={13} /> Open page
              </button>
            )}
          </div>
        ))}
        {facts.dlc.map((d) => (
          <div key={d} className="pick-item" style={{ cursor: "default" }}>
            <div className="grow">
              <b>{d}</b> <span className="pill">Expansion</span>
              <div className="meta">An expansion for the game. Zom's Nexus can't check whether you own it.</div>
            </div>
          </div>
        ))}
      </div>
    </Dialog>
  );
}
