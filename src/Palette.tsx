// Ctrl+K: find a page, game, profile, mod or setting from anywhere. Reads only what the app
// already has on this computer (never asks Nexus Mods).

import { useEffect, useMemo, useRef, useState } from "react";
import { Gamepad2, Layers, Package, Search, Settings as Cog, Compass } from "lucide-react";
import { api, type Game, type Profile, type StoredMod } from "./api";
import type { Page } from "./App";
import { wantProfile } from "./nav";

type Item = { key: string; label: string; hint: string; icon: React.ReactNode; run: () => void; words: string };

const PAGES: [Page, string, string][] = [
  ["downloads", "Downloads", "files downloading and done"],
  ["library", "Library", "every mod you have"],
  ["browse", "Browse", "find mods on Nexus Mods"],
  ["profiles", "Profiles", "sets of mods to play with"],
  ["games", "Games", "your games and their folders"],
  ["activity", "Activity", "what needs fixing, what happened"],
  ["settings", "Settings", "downloads, account, this computer"],
];
const SETTINGS = ["Nexus Mods account", "API key", "Download mode", "Speed limit", "Library folder", "Keep archives", "Adult content", "Request saver", "Start with computer", "nxm links", "Logs", "Theme", "Schedule", "Startup"];

/** Words that all appear somewhere in `hay`, best first: name starts, then name contains. */
export function score(q: string, label: string, words: string): number {
  const parts = q.toLowerCase().split(/\s+/).filter(Boolean);
  const l = label.toLowerCase();
  const all = `${l} ${words.toLowerCase()}`;
  if (!parts.every((p) => all.includes(p))) return -1;
  return (l.startsWith(parts[0]) ? 3 : 0) + (parts.every((p) => l.includes(p)) ? 2 : 0) - l.length / 1000;
}

export function Palette({ games, go, setGame, openAdd, onClose }: { games: Game[]; go: (p: Page, g?: string) => void; setGame: (g: string) => void; openAdd: () => void; onClose: () => void }) {
  const [q, setQ] = useState("");
  const [at, setAt] = useState(0);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [mods, setMods] = useState<StoredMod[]>([]);
  const list = useRef<HTMLDivElement>(null);
  useEffect(() => {
    api.profiles().then(setProfiles, () => {});
    api.library().then(setMods, () => {});
  }, []);
  const gameName = (d: string) => games.find((g) => g.domain === d)?.name ?? d;

  const items = useMemo<Item[]>(() => {
    const done = (f: () => void) => () => {
      f();
      onClose();
    };
    return [
      { key: "add", label: "Add mod or collection…", hint: "N", icon: <Package size={15} />, run: done(openAdd), words: "download new paste address" },
      ...PAGES.map(([p, label, hint]) => ({ key: `page:${p}`, label, hint, icon: <Compass size={15} />, run: done(() => go(p)), words: `page ${hint}` })),
      ...games.map((g) => ({ key: `game:${g.domain}`, label: g.name, hint: "Game", icon: <Gamepad2 size={15} />, run: done(() => setGame(g.domain)), words: `game ${g.domain}` })),
      ...profiles.map((p) => ({
        key: `profile:${p.id}`,
        label: p.name,
        hint: `Profile · ${gameName(p.game)}`,
        icon: <Layers size={15} />,
        run: done(() => {
          wantProfile(p.id);
          go("profiles", p.game);
        }),
        words: `profile ${gameName(p.game)}`,
      })),
      ...mods.map((m) => ({
        key: `mod:${m.game}:${m.modId}`,
        label: m.name,
        hint: `Mod · ${gameName(m.game)}`,
        icon: <Package size={15} />,
        run: done(() => {
          go("library", m.game);
          setTimeout(() => window.dispatchEvent(new CustomEvent("zoms:library-filter", { detail: m.name })), 50);
        }),
        words: `mod ${m.author ?? ""} ${m.modId} ${gameName(m.game)}`,
      })),
      ...SETTINGS.map((s) => ({ key: `set:${s}`, label: s, hint: "Setting", icon: <Cog size={15} />, run: done(() => go("settings")), words: "setting settings option" })),
    ];
  }, [games, profiles, mods]); // eslint-disable-line react-hooks/exhaustive-deps

  const shown = useMemo(() => {
    if (!q.trim()) return items.filter((i) => i.key === "add" || i.key.startsWith("page:") || i.key.startsWith("profile:")).slice(0, 14);
    return items
      .map((i) => ({ i, s: score(q, i.label, i.words) }))
      .filter((x) => x.s >= 0)
      .sort((a, b) => b.s - a.s)
      .slice(0, 30)
      .map((x) => x.i);
  }, [q, items]);
  useEffect(() => {
    setAt(0);
  }, [q]);
  useEffect(() => {
    list.current?.querySelector(".pal-item.on")?.scrollIntoView({ block: "nearest" });
  }, [at]);

  return (
    <div className="backdrop pal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="dialog palette" role="dialog" aria-modal="true" aria-label="Find anything">
        <label className="pal-search">
          <Search size={16} />
          <input
            autoFocus
            placeholder="Find a page, game, profile, mod or setting"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape") onClose();
              else if (e.key === "ArrowDown") {
                e.preventDefault();
                setAt((a) => Math.min(a + 1, shown.length - 1));
              } else if (e.key === "ArrowUp") {
                e.preventDefault();
                setAt((a) => Math.max(a - 1, 0));
              } else if (e.key === "Enter") shown[at]?.run();
            }}
          />
          <kbd>Esc</kbd>
        </label>
        <div className="pal-list" ref={list}>
          {shown.length === 0 && <p className="muted small" style={{ padding: "10px 14px" }}>Nothing matches “{q}”.</p>}
          {shown.map((i, n) => (
            <button key={i.key} className={`pal-item${n === at ? " on" : ""}`} onMouseMove={() => setAt(n)} onClick={i.run}>
              {i.icon}
              <span className="grow ellipsis">{i.label}</span>
              <span className="muted small ellipsis">{i.hint}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
