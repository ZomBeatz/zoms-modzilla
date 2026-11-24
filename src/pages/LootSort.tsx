// Sort a Bethesda profile's plugins with LOOT's community masterlist: see the order it would
// use, beside the one now, and LOOT's notes (incompatibilities, missing requirements, plugins
// that need cleaning). The order is kept for the profile; the mod order (who wins file
// clashes) doesn't change.

import { useEffect, useState } from "react";
import { ArrowDownUp, ExternalLink } from "lucide-react";
import { api, openUrl, type LootProposal, type Profile } from "../api";
import { plural } from "../format";
import { Dialog } from "../ui";

/** Games LOOT has a masterlist for. */
export const LOOT_GAMES = ["skyrimspecialedition", "skyrim", "fallout4", "newvegas", "fallout3", "oblivion", "starfield", "morrowind"];

export function LootSort({ profile, sorted, onClose, onApplied, toast }: { profile: Profile; sorted: boolean; onClose: () => void; onApplied: () => void; toast: (t: string) => void }) {
  const [p, setP] = useState<LootProposal | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    api.lootSort(profile.id).then(setP, (e) => setError((e as Error).message));
  }, [profile.id]);
  const moved = p ? new Set(p.sorted.order.filter((n, i) => p.now[i] !== n)) : new Set<string>();
  const notes = p?.sorted.notes ?? [];
  const apply = async (order: string[] | null, said: string) => {
    try {
      await api.lootApply(profile.id, order);
      toast(said);
      onApplied();
      onClose();
    } catch (e) {
      setError((e as Error).message);
    }
  };
  return (
    <Dialog
      wide
      title={`Sort plugins · ${profile.name}`}
      onClose={onClose}
      foot={
        <>
          {sorted && (
            <button className="btn ghost left" onClick={() => void apply(null, "Plugins load in mod order again")}>
              Back to mod order
            </button>
          )}
          {error && <span className="err-text small">{error}</span>}
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" disabled={!p || !p.sorted.changed} onClick={() => p && void apply(p.sorted.order, `Plugin order sorted (${plural(moved.size, "plugin")} moved). It's used the next time the mods go in.`)}>
            <ArrowDownUp size={15} /> {p && !p.sorted.changed ? "Already in order" : "Use this order"}
          </button>
        </>
      }
    >
      {!p && !error && <p className="muted">Reading LOOT's masterlist…</p>}
      {p && (
        <>
          <p style={{ marginTop: 0 }} className="small">
            LOOT's community masterlist puts masters first, each plugin after the ones it needs, and the rest in its groups; everything else keeps your order. The mod order (which mod wins a file) stays as it is.
            {p.sorted.unknown > 0 && <span className="muted"> {plural(p.sorted.unknown, "plugin")} LOOT doesn't know {p.sorted.unknown === 1 ? "keeps its" : "keep their"} place.</span>}{" "}
            <button className="btn ghost sm" onClick={() => void openUrl("https://loot.github.io/")}>
              <ExternalLink size={12} /> About LOOT
            </button>
          </p>
          {notes.length > 0 && (
            <div className="loot-notes">
              {notes.map((n, i) => (
                <div key={i} className={`check-row ${n.kind === "error" ? "err-text" : n.kind === "warn" ? "warn-text" : ""}`}>
                  <b>{n.plugin}</b>: {n.text.replace(/\[([^\]]+)\]\([^)]+\)/g, "$1").replace(/\*\*/g, "")}
                </div>
              ))}
            </div>
          )}
          <div className="loot-cols">
            <div>
              <div className="muted small">Now</div>
              <ol className="loot-list">
                {p.now.map((n) => (
                  <li key={n}>{n}</li>
                ))}
              </ol>
            </div>
            <div>
              <div className="muted small">Sorted</div>
              <ol className="loot-list">
                {p.sorted.order.map((n) => (
                  <li key={n} className={moved.has(n) ? "moved" : undefined}>
                    {n}
                  </li>
                ))}
              </ol>
            </div>
          </div>
        </>
      )}
    </Dialog>
  );
}
