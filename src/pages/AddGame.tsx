// Adding a game: Nexus Mods games Steam has installed here (one click), a search over every
// game on Nexus Mods, or typing one in by hand.

import { useEffect, useMemo, useState } from "react";
import { Gamepad2, Plus, Search } from "lucide-react";
import { api, type Game } from "../api";
import { Dialog } from "../ui";

type Catalog = Awaited<ReturnType<typeof api.gameCatalog>>;

/** Names compared without case, punctuation or ™ marks. */
const norm = (s: string) => s.toLowerCase().replace(/[™®©]/g, "").replace(/[^a-z0-9]+/g, "");

/** Steam's install that is this Nexus Mods game, if any ("The Elder Scrolls V: Skyrim Special
 * Edition" is "Skyrim Special Edition"). */
export function steamMatch(nexusName: string, installed: Catalog["installed"]) {
  const n = norm(nexusName);
  if (n.length < 3) return undefined;
  return installed.find((a) => norm(a.name) === n) ?? (n.length >= 8 ? installed.find((a) => norm(a.name).endsWith(n)) : undefined);
}

export function AddGame({ games, onClose, onAdded, onManual, toast }: { games: Game[]; onClose: () => void; onAdded: () => void; onManual: () => void; toast: (t: string) => void }) {
  const [cat, setCat] = useState<Catalog | null>(null);
  const [error, setError] = useState("");
  const [q, setQ] = useState("");
  const [busy, setBusy] = useState("");
  useEffect(() => {
    api.gameCatalog().then(setCat, (e) => setError((e as Error).message));
  }, []);
  const have = new Set(games.map((g) => g.domain));
  const ready = useMemo(
    () => (cat ? cat.games.filter((g) => !have.has(g.domain)).map((g) => ({ g, steam: steamMatch(g.name, cat.installed) })).filter((x) => x.steam) : []),
    [cat, games], // eslint-disable-line react-hooks/exhaustive-deps
  );
  const found = useMemo(() => {
    if (!cat || q.trim().length < 2) return [];
    const n = norm(q);
    return cat.games
      .filter((g) => norm(g.name).includes(n) || g.domain.includes(n))
      .sort((a, b) => b.mods - a.mods)
      .slice(0, 40);
  }, [cat, q]);

  const add = async (g: { domain: string; name: string }, steam?: { appId: number; path: string }) => {
    setBusy(g.domain);
    try {
      await api.addGame(g.domain, g.name, steam?.path ?? "", null, "", steam?.appId ?? null);
      toast(`Added ${g.name}${steam ? "" : ". Set its install folder on its card"}`);
      onAdded();
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setBusy("");
    }
  };

  const row = (g: { domain: string; name: string; mods: number }, steam?: { appId: number; path: string }) => (
    <div key={g.domain} className="pick-item" style={{ cursor: "default" }}>
      <div className="grow" style={{ minWidth: 0 }}>
        <div className="row gap-sm wrap">
          <b className="ellipsis">{g.name}</b>
          {steam && <span className="pill ok">Installed in Steam</span>}
          {have.has(g.domain) && <span className="pill">Already added</span>}
        </div>
        <div className="meta num">
          {g.mods.toLocaleString()} mods on Nexus Mods{steam ? ` · ${steam.path}` : ""}
        </div>
      </div>
      {!have.has(g.domain) && (
        <button className="btn sm primary" disabled={!!busy} onClick={() => void add(g, steam)}>
          <Plus size={13} /> {busy === g.domain ? "Adding…" : "Add"}
        </button>
      )}
    </div>
  );

  return (
    <Dialog
      wide
      title="Add a game"
      onClose={onClose}
      foot={
        <>
          <button className="btn left" onClick={onManual} title="Type its Nexus Mods name, folder and start command yourself">
            Another game…
          </button>
          <button className="btn" onClick={onClose}>
            Done
          </button>
        </>
      }
    >
      {error && <p className="err-text">{error}</p>}
      {!cat && !error && <p className="muted">Looking at Steam and Nexus Mods…</p>}
      {cat && (
        <>
          <h3 className="row gap-sm" style={{ marginTop: 0 }}>
            <Gamepad2 size={15} /> Ready to add from Steam
          </h3>
          {ready.length ? (
            <div className="pick-list">{ready.map((x) => row(x.g, x.steam))}</div>
          ) : (
            <p className="muted small">Every Nexus Mods game Steam has installed here is on your list already.</p>
          )}
          <h3 className="row gap-sm">
            <Search size={15} /> Every game on Nexus Mods
          </h3>
          <label className="search" style={{ display: "flex", marginBottom: 10 }}>
            <Search size={14} />
            <input autoFocus placeholder={`Search ${cat.games.length.toLocaleString()} games`} value={q} onChange={(e) => setQ(e.target.value)} style={{ width: "100%" }} aria-label="Search Nexus Mods games" />
          </label>
          {q.trim().length >= 2 && (found.length ? <div className="pick-list">{found.map((g) => row(g, steamMatch(g.name, cat.installed)))}</div> : <p className="muted small">No game on Nexus Mods matches “{q}”.</p>)}
        </>
      )}
    </Dialog>
  );
}
