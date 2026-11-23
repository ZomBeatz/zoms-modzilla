// Thunderstore's mods for a game, searched on this computer (the list comes once an hour, as a
// few files from Thunderstore's CDN, no account needed). Get downloads a mod and everything it
// needs into the library, and into a profile when one is picked; a mod pack becomes a profile.

import { useEffect, useRef, useState } from "react";
import { Download, ExternalLink, Package, Search, Star } from "lucide-react";
import { api, on, openUrl, type Profile, type TsFound } from "../api";
import { bytes, plural, when } from "../format";
import { TsUpdates } from "./TsUpdates";

const count = (n: number) => (n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n >= 1000 ? `${(n / 1000).toFixed(n >= 10000 ? 0 : 1)}k` : String(n));
const SORTS: [string, string][] = [
  ["popular", "Popular"],
  ["rating", "Top rated"],
  ["new", "Just updated"],
];

export function ThunderstoreBrowse({ game, adult, toast, onProfile }: { game: string; adult: "show" | "blur" | "hide"; toast: (t: string) => void; onProfile: (p: Profile) => void }) {
  const [q, setQ] = useState("");
  const [typed, setTyped] = useState("");
  const [sort, setSort] = useState("popular");
  const [found, setFound] = useState<{ results: TsFound[]; total: number } | null>(null);
  const [more, setMore] = useState(0);
  const [error, setError] = useState("");
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [into, setInto] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [progress, setProgress] = useState("");

  useEffect(() => {
    const t = setTimeout(() => setQ(typed), 250);
    return () => clearTimeout(t);
  }, [typed]);
  useEffect(() => {
    api.profiles().then((ps) => setProfiles(ps.filter((p) => p.game === game)), () => {});
    const off = on<{ name: string; at: number; of: number }>("thunderstore:progress", (p) => setProgress(`${p.at} of ${p.of}: ${p.name}`));
    return () => void off.then((f) => f());
  }, [game]);
  // Each search knows what it was for: an answer to an older one is dropped.
  const asked = useRef("");
  useEffect(() => {
    setMore(0);
  }, [game, q, sort]);
  useEffect(() => {
    const key = `${game}\n${q}\n${sort}`;
    const first = asked.current !== key;
    asked.current = key;
    const offset = first ? 0 : more;
    setError("");
    if (!offset) setFound(null);
    api.tsSearch(game, q, sort, offset).then(
      (r) => {
        if (asked.current !== key) return;
        setFound((f) => (offset && f ? { results: [...f.results, ...r.results], total: r.total } : r));
      },
      (e) => {
        if (asked.current !== key) return;
        setError((e as Error).message);
        setFound({ results: [], total: 0 });
      },
    );
  }, [game, q, sort, more]);

  const get = async (m: TsFound) => {
    setBusy(m.fullName);
    setProgress("");
    try {
      const r = await api.tsAdd(game, m.fullName, into || null);
      const parts = [r.added.length ? `Downloaded ${plural(r.added.length, "mod")} (${r.added.slice(0, 3).join(", ")}${r.added.length > 3 ? "…" : ""})` : "Already in your library", r.had && r.added.length ? `${r.had} you had` : "", r.profile ? `in “${r.profile.name}”` : "", r.missing.length ? `Thunderstore doesn't have ${r.missing.join(", ")}` : ""];
      toast(parts.filter(Boolean).join(" · "));
      if (r.profile) onProfile(r.profile);
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setBusy(null);
      setProgress("");
    }
  };

  return (
    <>
      <TsUpdates game={game} toast={toast} onProfile={onProfile} />
      <div className="toolbar">
        <label className="pal-search small-search grow">
          <Search size={14} />
          <input placeholder="Search Thunderstore's mods for this game" value={typed} onChange={(e) => setTyped(e.target.value)} />
        </label>
        <div className="seg">
          {SORTS.map(([id, label]) => (
            <button key={id} className={sort === id ? "active" : ""} onClick={() => setSort(id)}>
              {label}
            </button>
          ))}
        </div>
        <label className="row gap-sm small" title="Get also puts the mod and what it needs into this profile">
          Put into
          <select value={into} onChange={(e) => setInto(e.target.value)}>
            <option value="">just the library</option>
            {profiles.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name}
              </option>
            ))}
          </select>
        </label>
      </div>
      {busy && <p className="muted small">Getting {busy}{progress ? ` · ${progress}` : "…"}</p>}
      {error && <p className="err-text">{error}</p>}
      {found === null ? (
        <p className="muted">Getting Thunderstore's list for this game (the first time takes a moment)…</p>
      ) : found.results.length === 0 && !error ? (
        <div className="empty">
          <Package size={28} />
          <p>Nothing matches “{q}”.</p>
        </div>
      ) : (
        <>
          <p className="muted small">{plural(found.total, "mod")}{q ? ` matching “${q}”` : ""} on Thunderstore. Mod packs become a profile of their own.</p>
          <div className="browse-grid">
            {found.results.map((m) => (
              <div key={m.fullName} className={`panel browse-card${m.deprecated ? " faded" : ""}`}>
                <button className="browse-pic" onClick={() => void get(m)} title="Download it and what it needs" disabled={!!busy}>
                  {m.icon ? <img src={m.icon} alt="" loading="lazy" className={m.nsfw && adult !== "show" ? "blurred" : undefined} /> : <Package size={28} />}
                  {m.modpack && <span className="pill accent browse-badge">Mod pack</span>}
                  {m.deprecated && <span className="pill warn browse-badge">Deprecated</span>}
                </button>
                <div className="browse-body">
                  <b className="ellipsis" title={m.name}>
                    {m.name.replaceAll("_", " ")}
                  </b>
                  <div className="meta">
                    <span>by {m.owner}</span>
                    <span>v{m.version}</span>
                    {m.updated && <span>{when(Date.parse(m.updated) / 1000)}</span>}
                  </div>
                  <p className="browse-summary">{m.description}</p>
                  <div className="row gap-sm">
                    <span className="muted small num row gap-sm" title="Downloads, all versions">
                      <Download size={12} /> {count(m.downloads)}
                    </span>
                    <span className="muted small num row gap-sm" title="Rating">
                      <Star size={12} /> {count(m.rating)}
                    </span>
                    {m.needs > 0 && <span className="muted small">+{plural(m.needs, "requirement")}</span>}
                    {m.size > 0 && <span className="muted small">{bytes(m.size)}</span>}
                    <span className="grow" />
                    <button className="icon-btn" title="Open on Thunderstore" aria-label="Open on Thunderstore" onClick={() => openUrl(m.url).catch((e) => toast((e as Error).message))}>
                      <ExternalLink size={14} />
                    </button>
                    <button className="btn sm primary" disabled={!!busy} onClick={() => void get(m)}>
                      {busy === m.fullName ? "Getting…" : "Get"}
                    </button>
                  </div>
                </div>
              </div>
            ))}
          </div>
          {found.results.length < found.total && (
            <button className="btn" style={{ marginTop: 12 }} onClick={() => setMore(found.results.length)}>
              Show more
            </button>
          )}
        </>
      )}
    </>
  );
}
