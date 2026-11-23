// What went wrong in Stardew Valley's last run, from SMAPI's log, and the fixes: update the mods
// SMAPI names (through the usual update picker), or move the collection to a newer revision.

import { useState } from "react";
import { Wrench } from "lucide-react";
import { api, type Job, type Profile, type Smapi, type StoredMod, type Update } from "../api";
import { plural, when } from "../format";
import { pendingUpdates } from "../progress";
import { awaitingClick } from "../App";

type Fix = { state: "waiting" | "click" | "downloading" | "done"; label: string };

/** Where the fix for one mod stands: its update waiting, downloading, or in the profile since the log. */
export function fixFor(modId: number | null | undefined, _version: string, profile: Profile, library: StoredMod[], jobs: Job[], since: number): Fix | null {
  if (!modId) return null;
  const pm = profile.mods.find((m) => m.modId === modId);
  const lib = library.find((l) => l.game === profile.game && l.modId === modId);
  const file = lib?.files.find((f) => f.fileId === pm?.fileId);
  // The profile's file changed since SMAPI's run (another version, or downloaded after it).
  // Only a file downloaded after SMAPI's run counts: version labels on Nexus Mods and in mods differ too often.
  if (file && file.downloadedAt > since) return { state: "done", label: `updated${file.version ? ` to ${file.version}` : ""}` };
  if (pm && !pm.enabled) return { state: "done", label: "switched off" };
  const waiting = (pendingUpdates()[profile.id] ?? []).find((u) => u.modId === modId);
  if (!waiting) return null;
  const j = jobs.find((x) => x.game === profile.game && x.modId === modId && x.fileId === waiting.fileId);
  if (j && awaitingClick(j)) return { state: "click", label: "update waiting for your click on Nexus Mods" };
  if (j && j.status !== "done" && j.size) return { state: "downloading", label: `update downloading (${Math.floor((100 * j.received) / j.size)}%)` };
  return { state: "waiting", label: "update on its way" };
}

export function SmapiPanel({ s, profile, library, jobs, toast, onUpdates, onPlay, onSwitchOff }: { s: Smapi; profile: Profile; library: StoredMod[]; jobs: Job[]; toast: (t: string) => void; onUpdates: (u: Update[]) => void; onPlay: () => void; onSwitchOff: (modId: number) => void }) {
  const [busy, setBusy] = useState("");
  const [more, setMore] = useState(false);
  const culprits = (s.culprits ?? []).filter((c) => c.count >= 3 || c.atTheEnd);
  const ids = new Set<number>([...s.updates.map((u) => u.nexusId ?? 0), ...s.broken.map((b) => b.modId ?? 0), ...culprits.map((c) => c.modId ?? 0)].filter(Boolean));
  const noNexus = s.updates.filter((u) => !u.nexusId);
  const roots = s.broken;
  if (!roots.length && !s.updates.length && !culprits.length) return null;
  const fixes = roots.map((b) => fixFor(b.modId, b.version, profile, library, jobs, s.at));
  const culpritFixes = culprits.map((c) => fixFor(c.modId, "", profile, library, jobs, s.at));
  const all = [...fixes, ...culpritFixes];
  const fixedCount = all.filter((f) => f?.state === "done").length;
  const onTheWay = all.filter((f) => f && f.state !== "done").length;
  // Fixed only when every problem has a fix in: one without a mod to update (something missing) isn't.
  const allFixed = all.length > 0 && all.every((f) => f?.state === "done");
  const run = async (label: string, f: () => Promise<void>) => {
    setBusy(label);
    try {
      await f();
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setBusy("");
    }
  };
  return (
    <div className={`panel smapi-panel${allFixed ? " fixed" : ""}`}>
      <div className="row gap wrap">
        <Wrench size={18} color={allFixed ? "var(--zb-ok)" : "var(--zb-warn)"} />
        <div className="grow">
          <b>
            {allFixed
              ? `Fixes are in for all ${plural(fixedCount, "problem")} from the last run. Play to check they worked.`
              : onTheWay
                ? `Fixing: ${plural(onTheWay, "update")} on the way${fixedCount ? `, ${fixedCount} in` : ""}`
                : s.crashed && culprits.length
                  ? `The game crashed last time: ${culprits[0].name} kept failing`
                  : roots.length
                  ? `SMAPI skipped ${plural(s.skipped, "mod")} last time`
                  : `SMAPI found ${plural(s.updates.length, "update")}`}
          </b>
          <div className="muted small">
            {when(s.at)}
            {s.gameVersion && ` · Stardew Valley ${s.gameVersion}`}
            {s.smapiVersion && ` · SMAPI ${s.smapiVersion}`}
          </div>
        </div>
      </div>
      {culprits.length > 0 && (
        <ul className="smapi-roots small">
          {culprits.map((c, i) => {
            const up = s.updates.find((u) => u.name.replace(/\s/g, "").toLowerCase() === c.name.replace(/\s/g, "").toLowerCase());
            const f = culpritFixes[i];
            return (
              <li key={c.name} className={f?.state === "done" ? "fixed" : undefined}>
                {f && <span className={`pill tiny-pill${f.state === "done" ? " ok" : f.state === "click" ? " warn" : ""}`}>{f.label}</span>} <b>{c.name}</b> failed {c.count === 1 ? "once" : `${c.count} times`}
                {c.atTheEnd && s.crashed ? ", right up to the crash" : ""}
                {up ? <span className="muted"> · {up.version} is out (you have {up.have})</span> : ""}
                <div className="muted mono small ellipsis" title={c.what}>
                  {c.what}
                </div>
                {c.modId && f?.state !== "done" && (
                  <button className="btn ghost sm" onClick={() => onSwitchOff(c.modId!)} title="Switch it off in this profile (and what needs it), until it's updated">
                    Switch it off
                  </button>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {roots.length > 0 && (
        <ul className="smapi-roots small">
          {(more ? roots : roots.slice(0, 4)).map((b, i) => (
            <li key={b.name} className={fixes[i]?.state === "done" ? "fixed" : undefined}>
              {fixes[i] && <span className={`pill tiny-pill${fixes[i]!.state === "done" ? " ok" : fixes[i]!.state === "click" ? " warn" : ""}`}>{fixes[i]!.label}</span>}{" "}
              <b>{b.name}</b> {b.version && <span className="muted">{b.version}</span>}:{" "}
              {b.kind === "old" ? "too old for this version of the game or SMAPI" : b.kind === "missing" ? `needs ${b.needs.join(", ")}, which ${b.needs.length === 1 ? "isn't" : "aren't"} there` : b.why}
              {b.dependents.length > 0 && (
                <span className="muted" title={b.dependents.join(", ")}>
                  {" "}
                  · {plural(b.dependents.length, "other mod")} need{b.dependents.length === 1 ? "s" : ""} it
                </span>
              )}
            </li>
          ))}
          {roots.length > 4 && (
            <li>
              <button className="btn ghost sm" onClick={() => setMore((m) => !m)}>
                {more ? "Fewer" : `${roots.length - 4} more`}
              </button>
            </li>
          )}
        </ul>
      )}
      <div className="row gap wrap">
        {allFixed && (
          <button className="btn primary sm" onClick={onPlay}>
            Play to check
          </button>
        )}
        {ids.size > 0 && !allFixed && (
          <button
            className="btn primary sm"
            disabled={!!busy}
            title="Looks up the newest files for the mods SMAPI named (one request per mod in the profile), then lets you pick"
            onClick={() =>
              run("updates", async () => {
                const r = await api.profileUpdates(profile.id);
                const picked = r.updates.filter((u) => u.newer && ids.has(u.modId));
                if (picked.length) onUpdates(picked);
                else toast("Nexus Mods has no newer files for those mods that this profile could use. A newer collection revision may have them.");
              })
            }
          >
            {busy === "updates" ? "Looking…" : `Update the mods SMAPI named`}
          </button>
        )}
        {profile.base && (
          <button
            className="btn sm"
            disabled={!!busy}
            title="Asks Nexus Mods for the collection's newest revision (one request)"
            onClick={() =>
              run("rev", async () => {
                const r = await api.collectionUpdate(profile.id, false);
                toast(r.to != null && r.to !== r.from ? `Revision ${r.to} of the collection is out. Use “Update to revision ${r.to}” on the collection card above: it's usually made for the newer game.` : "The collection is on its newest revision. Updating the mods is the way.");
              })
            }
          >
            {busy === "rev" ? "Looking…" : "Newer collection revision?"}
          </button>
        )}
      </div>
      {noNexus.length > 0 && <p className="muted small" style={{ marginBottom: 0 }}>Updates only on smapi.io (unofficial): {noNexus.map((u) => `${u.name} ${u.version}`).join(", ")}.</p>}
      {s.errors.length > 0 && (
        <p className="muted small" style={{ marginBottom: 0 }}>
          Errors while playing: {s.errors.map(([who, n]) => `${who} (${n})`).join(", ")}. Errors from SMAPI itself usually come from content mods missing what they need, and go away with the fixes above.
        </p>
      )}
    </div>
  );
}

/** A mod that crashed the game last time and is still on with nothing done about it. */
export function unfixedCrash(s: Smapi | null | undefined, profile: Profile, library: StoredMod[], jobs: Job[]) {
  if (!s?.crashed) return null;
  return (s.culprits ?? []).find((c) => c.atTheEnd && c.modId && profile.mods.some((m) => m.modId === c.modId && m.enabled) && fixFor(c.modId, "", profile, library, jobs, s.at)?.state !== "done") ?? null;
}
