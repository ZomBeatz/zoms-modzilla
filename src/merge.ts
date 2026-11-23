// Put another profile's mods on top of this one (RimWorld players keep a dozen lists and mix them).

import type { ProfileMod } from "./api";

/** `base` with `extra`'s mods added after it: mods already there keep their place, but switch on
 *  and take the other profile's file when it's on there. */
export function merge(base: ProfileMod[], extra: ProfileMod[]): { mods: ProfileMod[]; added: number; changed: number } {
  const mods = base.map((m) => ({ ...m }));
  let added = 0;
  let changed = 0;
  for (const e of extra) {
    const have = mods.find((m) => m.modId === e.modId);
    if (!have) {
      mods.push({ ...e });
      added++;
    } else if (e.enabled && (!have.enabled || have.fileId !== e.fileId)) {
      have.enabled = true;
      have.fileId = e.fileId;
      changed++;
    }
  }
  return { mods, added, changed };
}
