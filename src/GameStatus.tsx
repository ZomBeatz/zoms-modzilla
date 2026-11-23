// Where a game is after Play: putting mods in, starting, running (with a timer), or why it
// didn't. Shown in the sidebar, and used by the Play buttons.

import { useEffect, useState } from "react";
import { AlertTriangle, CheckCircle2, Loader2, Play, Square, X } from "lucide-react";
import { api, type GameState } from "./api";

/** States worth showing: anything in progress, and endings from the last 10 minutes. */
export const showing = (s: GameState, now = Date.now() / 1000) => ["applying", "starting", "running"].includes(s.state) || now - s.since < 600;

function useNow(every = 1000) {
  const [now, setNow] = useState(() => Date.now() / 1000);
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now() / 1000), every);
    return () => clearInterval(t);
  }, [every]);
  return now;
}

export const clock = (secs: number) => {
  const s = Math.max(0, Math.floor(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h ? `${h}:${String(m).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}` : `${m}:${String(s % 60).padStart(2, "0")}`;
};

/** The words for a state, short (buttons) and long (the sidebar). */
export function words(s: GameState, now: number): { short: string; long: string } {
  const t = clock(now - s.since);
  switch (s.state) {
    case "applying":
      return { short: "Putting mods in…", long: "Putting the profile's mods in" };
    case "starting":
      return { short: "Starting…", long: now - s.since > 60 ? `Waiting for the game to show up (${t}). Steam may be updating it or showing a window.` : "Asked to start. Waiting for the game to open." };
    case "running":
      return { short: `Playing · ${t}`, long: `Running for ${t}` };
    case "notStarted":
      return { short: "Didn't start", long: "Nothing from the game's folder ran within three minutes. Check Steam for an update, an error or a window waiting for you, then try Play again." };
    case "stopped":
      return { short: "Play", long: `Closed after ${s.detail ?? "a while"}` };
    case "failed":
      return { short: "Couldn't start", long: s.detail ?? "Something went wrong. Settings > Logs has the details." };
  }
}

export function GameStatus({ states, gameName, onOpen, onDismiss }: { states: GameState[]; gameName: (d: string) => string; onOpen: (s: GameState) => void; onDismiss: (game: string) => void }) {
  const now = useNow();
  const [asking, setAsking] = useState<string | null>(null);
  const shown = states.filter((s) => showing(s, now));
  if (!shown.length) return null;
  const stop = (game: string, end: boolean) => {
    setAsking(null);
    void api.stopGame(game, end).catch(() => {});
  };
  return (
    <>
      {shown.map((s) => {
        const w = words(s, now);
        const tone = s.state === "running" ? "ok" : s.state === "failed" || s.state === "notStarted" ? "bad" : s.state === "stopped" ? "done" : "busy";
        const Icon = tone === "ok" ? Play : tone === "bad" ? AlertTriangle : tone === "done" ? CheckCircle2 : Loader2;
        return (
          <div key={s.game} className={`game-status ${tone}`} role="status">
            <button className="game-status-body" onClick={() => onOpen(s)} title="Open its profile">
              <Icon size={14} className={tone === "busy" ? "spin" : undefined} />
              <span className="grow">
                <b className="ellipsis">{gameName(s.game)}</b>
                <span className="small">{w.long}</span>
                {s.profile && <span className="small muted ellipsis">{s.profile}</span>}
              </span>
            </button>
            {!["applying", "starting", "running"].includes(s.state) && (
              <button className="icon-btn" aria-label="Dismiss" title="Dismiss" onClick={() => onDismiss(s.game)}>
                <X size={13} />
              </button>
            )}
            {["starting", "running"].includes(s.state) && (
              <button className="icon-btn" aria-label="Stop" title="Stop waiting for it, or close the game" onClick={() => setAsking(asking === s.game ? null : s.game)}>
                <Square size={12} />
              </button>
            )}
            {asking === s.game && (
              <div className="game-status-ask">
                <button className="btn sm" onClick={() => stop(s.game, false)} title="Zom's Nexus stops waiting for it; the game is left alone">
                  Stop waiting
                </button>
                <button className="btn danger solid sm" onClick={() => stop(s.game, true)} title="Closes everything running from the game's folder (forced after 8 seconds). Unsaved progress is lost.">
                  Close the game
                </button>
              </div>
            )}
          </div>
        );
      })}
    </>
  );
}

/** A Play button's label and look while its game is busy. */
export function usePlayLabel(s: GameState | undefined): { label: string | null; busy: boolean; tone: string } {
  const now = useNow();
  if (!s || !showing(s, now) || s.state === "stopped") return { label: null, busy: false, tone: "" };
  const w = words(s, now);
  return { label: w.short, busy: ["applying", "starting", "running"].includes(s.state), tone: s.state === "running" ? "ok" : s.state === "failed" || s.state === "notStarted" ? "bad" : "busy" };
}

/**
 * Quick start, where the status card goes when nothing is starting or running: Play the picked
 * game's last profile (the one played most recently, or the one in the game now).
 */
export function QuickPlay({ game, onPlay }: { game: { domain: string; name: string; activeProfile?: string | null; steamAppId?: number | null; launch?: string | null } | undefined; onPlay: (id: string, name: string) => void }) {
  const [pick, setPick] = useState<{ id: string; name: string } | null>(null);
  const domain = game?.domain;
  const active = game?.activeProfile;
  useEffect(() => {
    let live = true;
    if (!domain) return setPick(null);
    void Promise.all([api.profiles(), api.playTimes()]).then(
      ([ps, times]) => {
        if (!live) return;
        const mine = ps.filter((p) => p.game === domain);
        const played = [...mine].filter((p) => times[p.id]?.lastPlayed).sort((a, b) => (times[b.id]?.lastPlayed ?? 0) - (times[a.id]?.lastPlayed ?? 0))[0];
        const p = played ?? mine.find((x) => x.name === active) ?? null;
        setPick(p ? { id: p.id, name: p.name } : null);
      },
      () => live && setPick(null),
    );
    return () => {
      live = false;
    };
  }, [domain, active]);
  if (!game || !pick || !(game.steamAppId || game.launch)) return null;
  return (
    <button className="game-status quick-play" onClick={() => onPlay(pick.id, pick.name)} title={`Put “${pick.name}” in ${game.name} and start it`}>
      <Play size={14} />
      <span className="grow">
        <b className="ellipsis">Play {game.name}</b>
        <span className="small muted ellipsis">{pick.name}</span>
      </span>
    </button>
  );
}
