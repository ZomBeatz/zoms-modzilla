import type { Profile, StoredMod } from "./api";

/** How many of a profile's mods (the ones switched on) are downloaded into the library. */
export function profileProgress(p: Profile, library: StoredMod[]): { have: number; total: number } {
  const on = p.mods.filter((m) => m.enabled);
  const have = on.filter((m) => library.some((l) => l.game === p.game && l.modId === m.modId && l.files.some((f) => f.fileId === m.fileId))).length;
  return { have, total: on.length };
}

/** Updates a profile is waiting for: when each file finishes, it replaces the mod's old file
 * there. Kept in this browser's storage until then. */
const PENDING = "zoms-nexus:profile-updates";
type Pending = Record<string, { modId: number; fileId: number }[]>;
export function pendingUpdates(): Pending {
  try {
    return JSON.parse(localStorage.getItem(PENDING) ?? "{}") as Pending;
  } catch {
    return {};
  }
}
export function rememberUpdates(profileId: string, files: { modId: number; fileId: number }[]) {
  const all = pendingUpdates();
  all[profileId] = [...(all[profileId] ?? []).filter((f) => !files.some((n) => n.modId === f.modId)), ...files];
  try {
    localStorage.setItem(PENDING, JSON.stringify(all));
  } catch {
    /* no storage */
  }
}
export function takeUpdate(profileId: string, modId: number, fileId: number) {
  const all = pendingUpdates();
  all[profileId] = (all[profileId] ?? []).filter((f) => !(f.modId === modId && f.fileId === fileId));
  if (!all[profileId].length) delete all[profileId];
  try {
    localStorage.setItem(PENDING, JSON.stringify(all));
  } catch {
    /* no storage */
  }
}
