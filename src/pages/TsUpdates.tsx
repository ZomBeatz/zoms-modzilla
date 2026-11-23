// Updates for Thunderstore mods (and the mod packs profiles came from), with live progress.
// Free to check: it reads Thunderstore's list, fetched at most once an hour.

import { useEffect, useState } from "react";
import { RefreshCw } from "lucide-react";
import { api, on, type Profile } from "../api";
import { plural } from "../format";

type Updates = Awaited<ReturnType<typeof api.tsUpdates>>;

export function TsUpdates({ game, profile, toast, onProfile }: { game: string; profile?: Profile; toast: (t: string) => void; onProfile?: (p: Profile) => void }) {
  const [u, setU] = useState<Updates | null>(null);
  const [busy, setBusy] = useState("");
  const load = () => api.tsUpdates(game).then(setU, () => setU(null));
  useEffect(() => {
    void load();
    const off = on<{ name: string; at: number; of: number }>("thunderstore:progress", (p) => setBusy((b) => (b ? `${p.at} of ${p.of}: ${p.name}` : b)));
    return () => void off.then((f) => f());
  }, [game]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!u) return null;
  const mods = u.mods;
  const pack = profile ? u.packs.find((p) => p.profileId === profile.id) : null;
  const packs = profile ? [] : u.packs;
  if (!mods.length && !pack && !packs.length) return null;
  const run = async (what: string, f: () => Promise<unknown>) => {
    setBusy("Starting…");
    try {
      await f();
      toast(what);
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setBusy("");
      void load();
    }
  };
  return (
    <div className="panel collection-note accent ts-updates">
      <RefreshCw size={16} className={busy ? "spin" : undefined} />
      <div className="grow small">
        {pack && (
          <div>
            <b>{pack.pack}</b> has a new version: {pack.have} → {pack.newest}. Updating brings its mods' new versions and any it added; ones it dropped are switched off, and mods you added yourself stay.
          </div>
        )}
        {mods.length > 0 && (
          <div>
            <b>{plural(mods.length, "Thunderstore update")}</b>: {mods.slice(0, 4).map((m) => `${m.name} ${m.have} → ${m.newest}`).join(", ")}
            {mods.length > 4 ? "…" : ""}. Profiles that have them move to the new versions.
          </div>
        )}
        {packs.map((p) => (
          <div key={p.profileId}>
            Mod pack <b>{p.pack}</b> ({p.profileName}): {p.have} → {p.newest}.
          </div>
        ))}
        {busy && <div className="muted">{busy}</div>}
      </div>
      {pack && (
        <button className="btn sm primary" disabled={!!busy} onClick={() => void run(`Updated “${pack.pack}” to ${pack.newest}`, async () => onProfile?.(await api.tsUpdatePack(pack.profileId)))}>
          Update the pack
        </button>
      )}
      {packs.map((p) => (
        <button key={p.profileId} className="btn sm" disabled={!!busy} onClick={() => void run(`Updated “${p.pack}” to ${p.newest}`, async () => onProfile?.(await api.tsUpdatePack(p.profileId)))}>
          Update {p.profileName}
        </button>
      ))}
      {mods.length > 0 && (
        <button className="btn sm primary" disabled={!!busy} onClick={() => void run(`Updated ${plural(mods.length, "mod")}`, () => api.tsUpdate(game, mods.map((m) => m.fullName)))}>
          Update all
        </button>
      )}
    </div>
  );
}
