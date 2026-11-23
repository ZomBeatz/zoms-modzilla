// Before Play, from wherever it's pressed (the profile, the sidebar, the SMAPI panel), in order:
// 1. the last run went wrong and nothing changed since: a mod crashed the game and is still on,
//    or the game closed right after starting (any game, compared with the last run that went fine);
// 2. Steam Cloud is on for a profile with its own saves (it copies the usual saves back in);
// 3. which saves the game will use, the first time a profile is played and whenever its saves
//    would switch (a profile with its own saves after the usual ones, or the other way).
// Each step can be passed on; nothing here stops Play for good.

import { useEffect, useRef, useState } from "react";
import { CloudOff, DoorOpen, Save, Siren } from "lucide-react";
import { api, playWithoutMods, type Game, type Job, type Profile, type StoredMod, type Trouble } from "./api";
import { knownFacts, needersOf } from "./facts";
import { unfixedCrash } from "./pages/SmapiPanel";
import { changes } from "./pages/RunPanel";
import { when } from "./format";
import { Dialog } from "./ui";

const SEEN = "zoms-nexus:saves-seen";
function seen(): string[] {
  try {
    return JSON.parse(localStorage.getItem(SEEN) ?? "[]") as string[];
  } catch {
    return [];
  }
}
function markSeen(id: string) {
  try {
    localStorage.setItem(SEEN, JSON.stringify([...new Set([...seen(), id])].slice(-500)));
  } catch {
    /* no storage: asked again next time */
  }
}

const CLOUD = "zoms-nexus:cloud-told";
function told(): string[] {
  try {
    return JSON.parse(localStorage.getItem(CLOUD) ?? "[]") as string[];
  } catch {
    return [];
  }
}
function markTold(id: string) {
  try {
    localStorage.setItem(CLOUD, JSON.stringify([...new Set([...told(), id])].slice(-500)));
  } catch {
    /* no storage: told again next time */
  }
}

type Ask = { id: string; play: () => void };
let ask: ((a: Ask) => void) | null = null;

/** Play a profile, asking about saves first when it's worth it. */
const changed = () => window.dispatchEvent(new Event("zoms:profiles-changed"));

export function requestPlay(id: string, play: () => void) {
  if (ask) ask({ id, play });
  else play();
}

/** Whether to ask before playing `p`, and why: "first" or "switch". */
export function whyAsk(p: Profile, game: Game | undefined, profiles: Profile[], seenIds: string[]): "first" | "switch" | null {
  if (!game?.saveFolder) return null;
  const live = profiles.find((x) => x.game === p.game && x.name === game.activeProfile);
  if (live && live.id !== p.id && !!live.ownSaves !== !!p.ownSaves) return "switch";
  if (live && live.id !== p.id && live.ownSaves && p.ownSaves) return "switch";
  return seenIds.includes(p.id) ? null : "first";
}

type Step =
  | { kind: "crash"; name: string; modId: number }
  | { kind: "quick"; t: Trouble }
  | { kind: "cloud" }
  | { kind: "saves"; why: "first" | "switch"; live?: Profile };

export function SavesGate({ games, toast, jobs }: { games: Game[]; toast: (t: string) => void; jobs: Job[] }) {
  const [pending, setPending] = useState<{ a: Ask; p: Profile; step: Step } | null>(null);
  const [own, setOwn] = useState(false);
  const [quiet, setQuiet] = useState(true);
  const jobsNow = useRef(jobs);
  jobsNow.current = jobs;
  const gamesNow = useRef(games);
  gamesNow.current = games;

  /** The first step from `from` on that has something to say, or Play. */
  const go = async (a: Ask, from: number, given?: Profile) => {
    let all: Profile[];
    try {
      all = await api.profiles();
    } catch {
      return a.play();
    }
    const p = all.find((x) => x.id === a.id);
    if (!p) return a.play();
    const p2 = given ?? p;
    const game = gamesNow.current.find((g) => g.domain === p2.game);
    const why = whyAsk(p2, game, all, seen());
    if (from <= 0) {
      const [checks, library] = await Promise.all([api.profileChecks().catch(() => ({}) as Record<string, never>), api.library().catch(() => [] as StoredMod[])]);
      const c = unfixedCrash((checks as Record<string, import("./api").CheckReport>)[p2.id]?.gameLog?.smapi, p2, library, jobsNow.current);
      if (c?.modId) return setPending({ a, p: p2, step: { kind: "crash", name: c.name, modId: c.modId } });
    }
    if (from <= 1) {
      const t = await api.lastRun(p2.id).catch(() => null);
      if (t && !t.changedSince) return setPending({ a, p: p2, step: { kind: "quick", t } });
    }
    // The saves question mentions Steam Cloud itself; only say it on its own when that isn't asked.
    if (from <= 2 && !why && p2.ownSaves && game?.steamCloud && !told().includes(p2.id)) {
      setQuiet(true);
      return setPending({ a, p: p2, step: { kind: "cloud" } });
    }
    if (from <= 3 && why) {
      setOwn(!!p2.ownSaves);
      setQuiet(why === "first");
      return setPending({ a, p: p2, step: { kind: "saves", why, live: all.find((x) => x.game === p2.game && x.name === game?.activeProfile) } });
    }
    a.play();
  };
  const order: Step["kind"][] = ["crash", "quick", "cloud", "saves"];
  const next = (p?: Profile) => {
    if (!pending) return;
    const at = order.indexOf(pending.step.kind) + 1;
    const a = pending.a;
    setPending(null);
    void go(a, at, p);
  };
  const switchOff = async (ids: number[], what: string) => {
    if (!pending) return;
    const p = pending.p;
    const off = new Set(ids);
    if (!p.manualRequirements) for (const id of ids) needersOf(id, p.game, p.mods, knownFacts()).forEach((x) => off.add(x));
    const fewer = { ...p, mods: p.mods.map((m) => (off.has(m.modId) ? { ...m, enabled: false } : m)) };
    try {
      await api.saveProfile(fewer);
      changed();
    } catch (e) {
      return toast((e as Error).message);
    }
    toast(`Switched off ${what}${off.size > ids.length ? ` and what needs it (${off.size - ids.length})` : ""}. Switch it back on from the profile.`);
    next(fewer);
  };

  useEffect(() => {
    ask = (a) => void go(a, 0);
    return () => {
      ask = null;
    };
  }, []); // eslint-disable-line react-hooks/exhaustive-deps
  if (!pending) return null;
  const { p, step } = pending;
  const game = games.find((g) => g.domain === p.game);
  const gameName = game?.name ?? p.game;
  const close = () => setPending(null);

  if (step.kind === "crash")
    return (
      <Dialog
        title={`${step.name} crashed the game last time`}
        onClose={close}
        foot={
          <>
            <button className="btn ghost left" onClick={() => next()}>
              Play anyway
            </button>
            <button className="btn" onClick={close}>
              Cancel
            </button>
            <button className="btn primary" onClick={() => void switchOff([step.modId], `“${step.name}”`)}>
              Switch it off and play
            </button>
          </>
        }
      >
        <div className="saves-gate">
          <Siren size={20} color="var(--zb-danger)" />
          <div className="grow">
            <p style={{ marginTop: 0 }}>It kept failing right up to the crash, and it's still switched on in “{p.name}”. Playing now will most likely crash the same way.</p>
            <p className="muted small">Switching it off keeps everything else. Switch it back on once it's updated (the profile offers the update).</p>
          </div>
        </div>
      </Dialog>
    );

  if (step.kind === "quick") {
    const t = step.t;
    const added = t.added.filter((a) => p.mods.some((m) => m.modId === a.modId && m.enabled));
    const what = changes(t);
    return (
      <Dialog
        title={`${gameName} closed right after starting last time`}
        onClose={close}
        foot={
          <>
            <button className="btn ghost left" onClick={() => next()}>
              Play anyway
            </button>
            <button className="btn" onClick={close}>
              Cancel
            </button>
            <button
              className="btn"
              title="Start the game as it came: if it still closes, it isn't the mods"
              onClick={() => {
                close();
                void playWithoutMods(p.game).then(
                  () => toast(`Starting ${gameName} without mods. If it still closes, it isn't the mods.`),
                  (e) => toast((e as Error).message),
                );
              }}
            >
              Try without mods
            </button>
            {added.length > 0 && (
              <button className="btn primary" onClick={() => void switchOff(added.map((x) => x.modId), added.length === 1 ? `“${added[0].name}”` : `${added.length} new mods`)}>
                Switch off what's new and play
              </button>
            )}
          </>
        }
      >
        <div className="saves-gate">
          <DoorOpen size={20} color="var(--zb-warn)" />
          <div className="grow">
            <p style={{ marginTop: 0 }}>
              With “{p.name}”, {gameName} closed {t.secs} seconds after starting ({when(t.at)}){t.times > 1 ? `, ${t.times} times in a row` : ""}, and the profile hasn't changed since. A mod is almost always why.
            </p>
            {t.goodAt && what ? (
              <p>
                It last ran fine {when(t.goodAt)}. Since then you {what}.
              </p>
            ) : t.goodAt ? (
              <p>It ran fine {when(t.goodAt)} with the same mods, so something else changed (a game update, say): mods that run code are the first to break.</p>
            ) : (
              <p>It hasn't run fine with this profile yet. The profile's Find the problem mod switches half the mods off at a time to find which.</p>
            )}
          </div>
        </div>
      </Dialog>
    );
  }

  if (step.kind === "cloud")
    return (
      <Dialog
        title={`Steam Cloud is on for ${gameName}`}
        onClose={close}
        foot={
          <>
            <label className="check left">
              <input type="checkbox" checked={quiet} onChange={(e) => setQuiet(e.target.checked)} /> Don't remind me for this profile
            </label>
            <button className="btn" onClick={close}>
              Cancel
            </button>
            <button
              className="btn"
              onClick={async () => {
                const shared = { ...p, ownSaves: false };
                try {
                  await api.saveProfile(shared);
      changed();
                } catch (e) {
                  return toast((e as Error).message);
                }
                next(shared);
              }}
            >
              Use the usual saves
            </button>
            <button
              className="btn primary"
              onClick={() => {
                if (quiet) markTold(p.id);
                next();
              }}
            >
              Play
            </button>
          </>
        }
      >
        <div className="saves-gate">
          <CloudOff size={20} color="var(--zb-warn)" />
          <div className="grow">
            <p style={{ marginTop: 0 }}>
              “{p.name}” keeps its own saves, but Steam Cloud can copy your usual {gameName} saves into them while the game runs. Zom's Nexus notices afterwards and offers to move them out, but it can't stop Steam doing it.
            </p>
            <p className="small">
              To stop it: in Steam, right-click {gameName} › <b>Properties</b> › <b>General</b>, and turn off <b>Keep games saves in the Steam Cloud</b>. Your saves stay on this computer either way.
            </p>
          </div>
        </div>
      </Dialog>
    );

  const { why, live } = step;
  return (
    <Dialog
      title={`Saves for “${p.name}”`}
      onClose={close}
      foot={
        <>
          <label className="check left">
            <input type="checkbox" checked={quiet} onChange={(e) => setQuiet(e.target.checked)} /> Don't ask again for this profile
          </label>
          <button className="btn" onClick={close}>
            Cancel
          </button>
          <button
            className="btn primary"
            onClick={async () => {
              if (quiet) markSeen(p.id);
              let used = p;
              if (own !== !!p.ownSaves) {
                used = { ...p, ownSaves: own };
                try {
                  await api.saveProfile(used);
      changed();
                } catch (e) {
                  return toast((e as Error).message);
                }
              }
              // Told about Steam Cloud here, so not again before this Play.
              if (own && game?.steamCloud) markTold(p.id);
              close();
              pending.a.play();
            }}
          >
            Play
          </button>
        </>
      }
    >
      <div className="saves-gate">
        <Save size={20} color="var(--zb-accent)" />
        <div className="grow">
          {why === "switch" && live && (
            <p style={{ marginTop: 0 }}>
              “{live.name}” is in the game now{live.ownSaves ? ", with its own saves" : ", with the usual saves"}. Playing “{p.name}” puts {own ? "its own saves" : "the usual saves"} in place instead. Nothing is deleted: each set waits in {game?.saveFolder ? "the save folder's .zoms-nexus-saves" : "its own folder"} until it's used again.
            </p>
          )}
          {why === "first" && <p style={{ marginTop: 0 }}>The first time playing “{p.name}”. Which saves should the game see?</p>}
          <label className="setting" style={{ cursor: "pointer" }}>
            <input type="radio" name="saves" checked={!own} onChange={() => setOwn(false)} style={{ marginTop: 3 }} />
            <span className="what">
              <b>The usual saves</b>
              <span>The same saves as playing without mods, and as other profiles that share them.</span>
            </span>
          </label>
          <label className="setting" style={{ cursor: "pointer" }}>
            <input type="radio" name="saves" checked={own} onChange={() => setOwn(true)} style={{ marginTop: 3 }} />
            <span className="what">
              <b>Its own saves</b>
              <span>Kept apart, so a heavily modded game never touches your usual saves. {p.ownSaves ? "" : "It starts with none: start a new game, or copy a save in."}</span>
            </span>
          </label>
          {own && game?.steamCloud && (
            <p className="warn-text small">
              Steam Cloud is on for {game.name}. It may copy your usual saves into this profile's folder (sometimes as empty folders). Zom's Nexus notices and offers to move them out; to stop it, turn Steam Cloud off in Steam › {game.name} › Properties › General.
            </p>
          )}
        </div>
      </div>
    </Dialog>
  );
}
