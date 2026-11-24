import { useEffect, useState } from "react";
import { Compass, Download, ExternalLink, Heart, Package } from "lucide-react";
import { api, modUrl, openUrl, type ModInfo, type TsGame } from "../api";
import { ThunderstoreBrowse } from "./ThunderstoreBrowse";
import { wantProfile } from "../nav";
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
export function Browse({ game: picked, games, overview, setOverview, openAdd, go, toast }: Ctx) {
  // Browsing is always for one game: the picked one, or the first one with All games.
  const game = picked ?? games[0]?.domain ?? null;
  const [list, setList] = useState<List>("trending");
  const [mods, setMods] = useState<ModInfo[] | null>(null);
  const [error, setError] = useState("");
  const saver = !!overview.settings.requestSaver;
  // In request saver mode a list loads only when asked for (each costs one request).
  const [asked, setAsked] = useState("");
  // Thunderstore, for games it has.
  const [ts, setTs] = useState<TsGame | null>(null);
  const [source, setSource] = useState<"nexus" | "ts">("nexus");
  useEffect(() => {
    setTs(null);
    if (!game) return;
    let live = true;
    api.tsGame(game).then((t) => live && setTs(t), () => {});
    return () => {
      live = false;
    };
  }, [game]);
  const onTs = source === "ts" && !!ts;
  const setSaver = async (on: boolean) => {
    try {
      setOverview(await api.saveSettings({ ...overview.settings, requestSaver: on }));
    } catch (e) {
      toast((e as Error).message);
    }
  };

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
    if (saver && asked !== key) {
      setMods(null);
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
  }, [game, list, overview.hasKey, saver, asked]); // eslint-disable-line react-hooks/exhaustive-deps
  const waiting = saver && mods === null && !lists.has(`${game}:${list}`);

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
      {game && ts && (
        <div className="toolbar">
          <div className="seg" role="tablist" aria-label="Where mods come from">
            <button className={!onTs ? "active" : ""} onClick={() => setSource("nexus")}>
              Nexus Mods
            </button>
            <button className={onTs ? "active" : ""} onClick={() => setSource("ts")}>
              Thunderstore
            </button>
          </div>
          {onTs && !ts.supported && <span className="warn-text small">{ts.name} uses {ts.loader} on Thunderstore, which Zom's Modzilla can't put in yet: you can look, not get.</span>}
        </div>
      )}
      {onTs && game ? (
        <>
          {g && <Hero domain={g.domain} name={g.name} />}
          <ThunderstoreBrowse
            game={game}
            adult={(overview.settings.adultContent ?? "blur") as "show" | "blur" | "hide"}
            toast={toast}
            onProfile={(p) => {
              wantProfile(p.id);
              go("profiles", p.game);
            }}
          />
        </>
      ) : !overview.hasKey ? (
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
            <span className="muted small grow">Nexus Mods' own lists. To search, use Search on Nexus Mods, then copy a mod's address into Add mod.</span>
            <label className="check" title="Browse waits for you to ask before loading a list, so your requests go to downloads. You can always look on the website instead, which costs nothing.">
              <input type="checkbox" checked={saver} onChange={(e) => void setSaver(e.target.checked)} /> Request saver
            </label>
          </div>
          {waiting && (
            <div className="empty">
              <Compass size={28} />
              <p>Request saver is on, so this list waits for you. Loading it uses 1 of your {overview.limits.dailyRemaining != null ? `${overview.limits.dailyRemaining.toLocaleString()} requests left today` : "requests"}; the website costs nothing.</p>
              <div className="row gap-sm">
                <button className="btn primary" onClick={() => setAsked(`${game}:${list}`)}>
                  <Download size={15} /> Load this list
                </button>
                <button className="btn" onClick={() => void openUrl(`https://www.nexusmods.com/games/${game}/mods`)}>
                  <ExternalLink size={15} /> Open on Nexus Mods
                </button>
              </div>
            </div>
          )}
          {error && <p className="err-text">{error}</p>}
          {waiting ? null : mods === null ? (
            <p className="muted">Loading…</p>
          ) : mods.length === 0 && !error ? (
            <div className="empty">
              <Package size={28} />
              <p>Nothing on this list right now.</p>
            </div>
          ) : (
            <div className="browse-grid">
              {mods.filter((m) => !(m.adult && overview.settings.adultContent === "hide")).map((m) => (
                <div key={m.modId} className="panel browse-card">
                  <button className="browse-pic" onClick={() => openAdd(modUrl(m.game, m.modId))} title="Pick files to download">
                    {m.pictureUrl ? <img src={m.pictureUrl} alt="" loading="lazy" className={m.adult && (overview.settings.adultContent ?? "blur") !== "show" ? "blurred" : undefined} /> : <Package size={28} />}
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
