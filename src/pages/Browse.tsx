import { useEffect, useState } from "react";
import { Compass, Download, ExternalLink, Heart, Package } from "lucide-react";
import { api, modUrl, openUrl, type ModInfo } from "../api";
import type { Ctx } from "../App";
import { Hero } from "../art";
import { when } from "../format";

type List = "trending" | "latest_added" | "latest_updated";
const LISTS: [List, string][] = [
  ["trending", "Trending"],
  ["latest_added", "New"],
  ["latest_updated", "Just updated"],
];

/** Lists already fetched this run, by game and list, kept for 10 minutes. */
const KEEP_MS = 10 * 60 * 1000;
const lists = new Map<string, { at: number; mods: ModInfo[] }>();

const count = (n?: number | null) => (n == null ? "" : n >= 1000 ? `${(n / 1000).toFixed(n >= 10000 ? 0 : 1)}k` : String(n));

/** Nexus Mods' own lists for a game, with a way into each mod's files. */
export function Browse({ game: picked, games, overview, openAdd, go, toast }: Ctx) {
  // Browsing is always for one game: the picked one, or the first one with All games.
  const game = picked ?? games[0]?.domain ?? null;
  const [list, setList] = useState<List>("trending");
  const [mods, setMods] = useState<ModInfo[] | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!game || !overview.hasKey) return;
    const key = `${game}:${list}`;
    const kept = lists.get(key);
    // Each list costs one request; keep it for a while so moving between pages is free.
    if (kept && Date.now() - kept.at < KEEP_MS) {
      setMods(kept.mods);
      setError("");
      return;
    }
    setMods(null);
    setError("");
    api.browse(game, list).then(
      (m) => {
        lists.set(key, { at: Date.now(), mods: m });
        setMods(m);
      },
      (e) => {
        setError((e as Error).message);
        setMods([]);
      },
    );
  }, [game, list, overview.hasKey]);

  const g = games.find((x) => x.domain === game);

  return (
    <div className="page">
      <div className="page-head">
        <h1>Browse</h1>
        {game && (
          <button className="btn" onClick={() => openUrl(`https://www.nexusmods.com/games/${game}/mods`)}>
            <ExternalLink size={15} /> Search on Nexus Mods
          </button>
        )}
      </div>
      {!overview.hasKey ? (
        <div className="empty">
          <Compass size={32} />
          <p>Add your Nexus Mods API key to browse mods.</p>
          <button className="btn primary" onClick={() => go("settings")}>
            Go to Settings
          </button>
        </div>
      ) : !game ? (
        <div className="empty">
          <Compass size={32} />
          <p>Add a game on the Games page to browse its mods.</p>
          <button className="btn primary" onClick={() => go("games")}>
            Go to Games
          </button>
        </div>
      ) : (
        <>
          {g && <Hero domain={g.domain} name={g.name} />}
          <div className="toolbar">
            <div className="seg">
              {LISTS.map(([id, label]) => (
                <button key={id} className={list === id ? "active" : ""} onClick={() => setList(id)}>
                  {label}
                </button>
              ))}
            </div>
            <span className="muted small">Nexus Mods' own lists. To search, use Search on Nexus Mods, then copy a mod's address into Add mod.</span>
          </div>
          {error && <p className="err-text">{error}</p>}
          {mods === null ? (
            <p className="muted">Loading…</p>
          ) : mods.length === 0 && !error ? (
            <div className="empty">
              <Package size={28} />
              <p>Nothing on this list right now.</p>
            </div>
          ) : (
            <div className="browse-grid">
              {mods.map((m) => (
                <div key={m.modId} className="panel browse-card">
                  <button className="browse-pic" onClick={() => openAdd(modUrl(m.game, m.modId))} title="Pick files to download">
                    {m.pictureUrl ? <img src={m.pictureUrl} alt="" loading="lazy" /> : <Package size={28} />}
                    {m.adult && <span className="pill warn browse-badge">Adult</span>}
                  </button>
                  <div className="browse-body">
                    <b className="ellipsis" title={m.name}>
                      {m.name}
                    </b>
                    <div className="meta">
                      {m.author && <span>by {m.author}</span>}
                      {m.updated && <span>{when(m.updated)}</span>}
                    </div>
                    {m.summary && <p className="browse-summary">{m.summary.replace(/<[^>]+>/g, "")}</p>}
                    <div className="row gap-sm">
                      {m.endorsements != null && (
                        <span className="muted small num row gap-sm">
                          <Heart size={12} /> {count(m.endorsements)}
                        </span>
                      )}
                      {m.downloads != null && (
                        <span className="muted small num row gap-sm">
                          <Download size={12} /> {count(m.downloads)}
                        </span>
                      )}
                      <span className="grow" />
                      <button className="icon-btn" title="Open on Nexus Mods" aria-label="Open on Nexus Mods" onClick={() => openUrl(modUrl(m.game, m.modId)).catch((e) => toast((e as Error).message))}>
                        <ExternalLink size={14} />
                      </button>
                      <button className="btn sm primary" onClick={() => openAdd(modUrl(m.game, m.modId))}>
                        Get…
                      </button>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
