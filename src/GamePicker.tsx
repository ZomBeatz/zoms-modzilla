// The game picker at the top of the sidebar: one game is always picked, and every page shows
// just that game. The menu lists your most recent games (up to 5, fewer when the window is
// short); "All games…" opens a selector with every game.

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Check, ChevronDown, Gamepad2, LayoutGrid, Search } from "lucide-react";
import type { Game } from "./api";
import { Cover } from "./art";
import { Dialog } from "./ui";

const MOST = 5;
const ROW = 46;

/** Games in the order to offer them: recently picked first, then the rest by name. */
export function ordered(games: Game[], recent: string[]): Game[] {
  const byRecent = recent.map((d) => games.find((g) => g.domain === d)).filter((g): g is Game => !!g);
  return [...byRecent, ...games.filter((g) => !recent.includes(g.domain)).sort((a, b) => a.name.localeCompare(b.name))];
}

export function GamePicker({ games, game, recent, onPick, onManage }: { games: Game[]; game: string | null; recent: string[]; onPick: (g: string) => void; onManage: () => void }) {
  const [open, setOpen] = useState(false);
  const [more, setMore] = useState(false);
  const [fits, setFits] = useState(MOST);
  const [pos, setPos] = useState({ top: 0, left: 0 });
  const box = useRef<HTMLDivElement>(null);
  const current = games.find((g) => g.domain === game);

  // How many games fit under the button without the menu running off the window.
  useLayoutEffect(() => {
    if (!open || !box.current) return;
    const r = box.current.getBoundingClientRect();
    setPos({ top: r.bottom + 6, left: r.left });
    const below = window.innerHeight - r.bottom - 24;
    setFits(Math.max(1, Math.min(MOST, Math.floor((below - 2 * ROW - 12) / ROW))));
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const away = (e: MouseEvent) => !box.current?.contains(e.target as Node) && setOpen(false);
    const key = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("mousedown", away);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("mousedown", away);
      window.removeEventListener("keydown", key);
    };
  }, [open]);

  const list = ordered(games, recent);
  const shown = list.slice(0, fits);
  const pick = (g: string) => {
    onPick(g);
    setOpen(false);
    setMore(false);
  };

  return (
    <div className="game-picker" ref={box}>
      <button className={`picker-btn${open ? " open" : ""}`} onClick={() => (games.length ? setOpen(!open) : onManage())} aria-haspopup="menu" aria-expanded={open} title={games.length ? "Pick a game" : "Add a game"}>
        {current ? <Cover domain={current.domain} name={current.name} className="pick" /> : <span className="picker-all"><Gamepad2 size={16} /></span>}
        <span className="label grow picker-text">
          <span className="picker-small">Game</span>
          <b className="ellipsis">{current?.name ?? (games.length ? "Pick a game" : "Add a game")}</b>
        </span>
        <ChevronDown size={15} className="label muted" />
      </button>
      {open && (
        <div className="menu picker-menu" role="menu" style={{ top: pos.top, left: pos.left }}>
          {shown.map((g) => (
            <button key={g.domain} role="menuitem" className="menu-item" onClick={() => pick(g.domain)}>
              <Cover domain={g.domain} name={g.name} className="tiny" />
              <span className="grow ellipsis">{g.name}</span>
              {game === g.domain && <Check size={14} />}
            </button>
          ))}
          <div className="menu-sep" />
          <button role="menuitem" className="menu-item" onClick={() => setMore(true)}>
            <LayoutGrid size={14} />
            <span className="grow">All games…{list.length > shown.length ? ` (${list.length})` : ""}</span>
          </button>
          <button
            role="menuitem"
            className="menu-item"
            onClick={() => {
              setOpen(false);
              onManage();
            }}
          >
            <Gamepad2 size={14} />
            <span className="grow">{games.length ? "Add or change games…" : "Add a game…"}</span>
          </button>
        </div>
      )}
      {more && <MoreGames games={list} game={game} onPick={pick} onClose={() => setMore(false)} />}
    </div>
  );
}

function MoreGames({ games, game, onPick, onClose }: { games: Game[]; game: string | null; onPick: (g: string) => void; onClose: () => void }) {
  const [filter, setFilter] = useState("");
  const shown = games.filter((g) => !filter || g.name.toLowerCase().includes(filter.toLowerCase()));
  return (
    <Dialog wide title="All games" onClose={onClose}>
      <label className="search" style={{ display: "flex", marginBottom: 14 }}>
        <Search size={14} />
        <input autoFocus placeholder="Filter games" value={filter} onChange={(e) => setFilter(e.target.value)} style={{ width: "100%" }} aria-label="Filter games" />
      </label>
      {shown.length === 0 ? (
        <p className="muted">No games match “{filter}”.</p>
      ) : (
        <div className="pick-games">
          {shown.map((g) => (
            <button key={g.domain} className={`pick-game${g.domain === game ? " active" : ""}`} onClick={() => onPick(g.domain)}>
              <Cover domain={g.domain} name={g.name} />
              <span className="ellipsis">{g.name}</span>
            </button>
          ))}
        </div>
      )}
    </Dialog>
  );
}
