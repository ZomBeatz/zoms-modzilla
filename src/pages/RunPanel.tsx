// The last time this profile was played, the game closed right after starting. For any game,
// with no logs needed: what changed since it last ran fine, and the quickest ways to find which.

import { useEffect, useState } from "react";
import { DoorOpen } from "lucide-react";
import { api, on, type Profile, type Trouble } from "../api";
import { when } from "../format";

const names = (list: { name: string }[]) => (list.length > 3 ? `${list.slice(0, 3).map((c) => c.name).join(", ")} and ${list.length - 3} more` : list.map((c) => c.name).join(", "));

/** "2 h 10 min", "40 min". */
export const runLength = (secs: number) => {
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  return h ? `${h} h ${m} min` : `${Math.max(1, m)} min`;
};

/** What changed between the run that went fine and the one that didn't, in words. */
export function changes(t: Trouble): string {
  const parts = [t.added.length && `added ${names(t.added)}`, t.updated.length && `updated ${names(t.updated)}`, t.removed.length && `took out ${names(t.removed)}`].filter(Boolean);
  return parts.join("; ");
}

/** The last run's quick exit, read again when a game stops. */
export function useLastRun(profileId: string | undefined, mods: unknown) {
  const [t, setT] = useState<Trouble | null>(null);
  useEffect(() => {
    if (!profileId) return setT(null);
    const load = () => void api.lastRun(profileId).then(setT, () => setT(null));
    load();
    const off = on<number>("activity:changed", load);
    return () => void off.then((f) => f());
  }, [profileId, mods]);
  return t;
}

/** After repeated quick exits on stock Proton: GE-Proton fixes many mod crashes. */
export function protonTip(proton: string | null | undefined, custom: string[], gameName: string, steam: boolean): string | null {
  if (!proton || /ge-proton|proton-ge|tkg|cachyos/i.test(proton)) return null;
  const ge = custom.find((c) => /ge-proton/i.test(c));
  const how = steam ? `in Steam, ${gameName} › Properties › Compatibility` : "in its launcher's settings for the game";
  return ge ? `${gameName} runs with ${proton}. ${ge} is installed and fixes many mod crashes: pick it ${how}.` : `${gameName} runs with ${proton}. GE-Proton fixes many mod crashes Valve's Proton doesn't: add it with ProtonUp-Qt (in Discover on a Steam Deck), then pick it ${how}.`;
}

export function RunPanel({ t, gameName, profile, onSwitchOff, onFind, tip }: { t: Trouble; gameName: string; profile: Profile; onSwitchOff: (modIds: number[]) => void; onFind: () => void; tip?: string | null }) {
  const what = changes(t);
  const added = t.added.filter((a) => profile.mods.some((m) => m.modId === a.modId && m.enabled));
  return (
    <div className="panel collection-note warn run-panel">
      <DoorOpen size={18} color="var(--zb-warn)" />
      <div className="grow small">
        <b>
          {gameName} closed {t.secs} seconds after starting ({when(t.at)}{t.times > 1 ? `, ${t.times} times in a row` : ""}).
        </b>{" "}
        That's almost always a mod stopping it.{" "}
        {t.goodAt ? (
          what ? (
            <>
              Since it last ran fine ({when(t.goodAt)}, for {runLength(t.goodSecs ?? 0)}), you {what}.
            </>
          ) : (
            <>It ran fine {when(t.goodAt)} with the same mods, so something outside them changed (a game update, say).</>
          )
        ) : (
          <>No run with this profile went fine yet, so there's nothing to compare with.</>
        )}
        {t.changedSince && <span className="muted"> You've changed the profile since, so it may be fixed already: Play to find out.</span>}
        {tip && t.times > 1 && <span className="muted"> {tip}</span>}
      </div>
      {added.length > 0 && !t.changedSince && (
        <button className="btn sm primary" onClick={() => onSwitchOff(added.map((a) => a.modId))}>
          Switch off what's new
        </button>
      )}
      <button className="btn sm" onClick={onFind} title="Switch half the mods off, play, say if it still closes, and repeat">
        Find the problem mod
      </button>
    </div>
  );
}
