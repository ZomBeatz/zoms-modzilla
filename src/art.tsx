// Game pictures: the cover (tall), hero (wide banner) and logo, asked for once per game and
// kept for as long as the app is open. A game without a picture gets a colored tile with its
// initials instead.

import { useEffect, useState } from "react";
import { api, type Game } from "./api";

type Kind = "cover" | "hero" | "logo";
const cache = new Map<string, Promise<string | null>>();

export function useArt(domain: string | undefined, kind: Kind): string | null | undefined {
  const [url, setUrl] = useState<string | null | undefined>(undefined);
  useEffect(() => {
    if (!domain) return;
    const key = `${domain}:${kind}`;
    if (!cache.has(key)) cache.set(key, api.art(domain, kind).catch(() => null));
    let live = true;
    void cache.get(key)!.then((u) => live && setUrl(u));
    return () => {
      live = false;
    };
  }, [domain, kind]);
  return url;
}

const CATS = ["var(--zb-cat-1)", "var(--zb-cat-2)", "var(--zb-cat-3)", "var(--zb-cat-4)", "var(--zb-cat-5)", "var(--zb-cat-6)"];

function initials(name: string) {
  const words = name.replace(/[^\p{L}\p{N} ]/gu, " ").split(/\s+/).filter(Boolean);
  return (words.length > 1 ? words[0][0] + words[1][0] : (words[0] ?? "?").slice(0, 2)).toUpperCase();
}

export function tint(domain: string) {
  let h = 0;
  for (const c of domain) h = (h * 31 + c.charCodeAt(0)) >>> 0;
  return CATS[h % CATS.length];
}

/** The game's cover, filling its box. */
export function Cover({ domain, name, className = "" }: { domain: string; name: string; className?: string }) {
  const url = useArt(domain, "cover");
  if (url) return <img className={`cover ${className}`} src={url} alt="" draggable={false} />;
  return (
    <div className={`cover cover-blank ${className}`} style={{ background: tint(domain) }} aria-hidden="true">
      {url === undefined ? "" : initials(name)}
    </div>
  );
}

/** A wide banner with the game's logo (or name) over it. */
export function Hero({ domain, name, children }: { domain: string; name: string; children?: React.ReactNode }) {
  const hero = useArt(domain, "hero");
  const logo = useArt(domain, "logo");
  return (
    <div className="hero" style={hero ? { backgroundImage: `url("${hero}")` } : { background: `linear-gradient(135deg, ${tint(domain)}, var(--zb-surface-2))` }}>
      <div className="hero-shade" />
      <div className="hero-body">
        {logo ? <img className="hero-logo" src={logo} alt={name} /> : <h2 className="hero-name">{name}</h2>}
        {children}
      </div>
    </div>
  );
}

// Game names by domain, asked for once.
let names: Promise<Map<string, string>> | null = null;
export function useGameName(domain: string): string {
  const [name, setName] = useState(domain);
  useEffect(() => {
    names ??= api.games().then((g: Game[]) => new Map(g.map((x) => [x.domain, x.name])), () => new Map());
    let live = true;
    void names.then((m) => live && setName(m.get(domain) ?? domain));
    return () => {
      live = false;
    };
  }, [domain]);
  return name;
}

/** A tiny cover and the game's name, for lists. */
export function GameTag({ domain }: { domain: string }) {
  const name = useGameName(domain);
  return (
    <span className="game-tag">
      <Cover domain={domain} name={name} className="tiny" />
      <span className="ellipsis">{name}</span>
    </span>
  );
}
