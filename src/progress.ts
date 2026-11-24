import type { CheckReport, Profile, StoredMod } from "./api";

/** How many of a profile's mods (the ones switched on) are downloaded into the library. */
export function profileProgress(p: Profile, library: StoredMod[]): { have: number; total: number } {
  const on = p.mods.filter((m) => m.enabled);
  const have = on.filter((m) => library.some((l) => l.game === p.game && l.modId === m.modId && l.files.some((f) => f.fileId === m.fileId))).length;
  return { have, total: on.length };
}

/** Updates a profile is waiting for: when each file finishes, it replaces the mod's old file
 * there. Kept in this browser's storage until then. */
const PENDING = "zoms-modzilla:profile-updates";
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

/** What "Fix" changed in a profile, in words, and the requirements it couldn't find locally. */
export interface Fixed {
  profile: Profile;
  done: string[];
  /** Requirements that need downloading (mod numbers). */
  download: number[];
}

/**
 * Fix what can be fixed without asking: put in requirements that are in the library, switch on
 * ones that are off, point entries whose file was deleted at a file of the same mod that's still
 * here, and drop entries listed twice. Downloads are left to the caller (they cost requests).
 */
export function autoFix(p: Profile, library: StoredMod[], report?: CheckReport | null): Fixed {
  const done: string[] = [];
  const mine = library.filter((l) => l.game === p.game);
  const newest = (modId: number) => [...(mine.find((l) => l.modId === modId)?.files ?? [])].sort((a, b) => b.downloadedAt - a.downloadedAt)[0];
  let mods = [...p.mods];

  // Entries whose file isn't here, but a file of the same mod is.
  let swapped = 0;
  mods = mods.map((m) => {
    const lib = mine.find((l) => l.modId === m.modId);
    if (!lib || lib.files.some((f) => f.fileId === m.fileId)) return m;
    const f = newest(m.modId);
    if (!f) return m;
    swapped++;
    return { ...m, fileId: f.fileId };
  });
  if (swapped) done.push(`pointed ${swapped === 1 ? "a mod" : `${swapped} mods`} at the file you have`);

  // The same file twice: keep the first.
  const seen = new Set<string>();
  const before = mods.length;
  mods = mods.filter((m) => {
    const k = `${m.modId}:${m.fileId}`;
    if (seen.has(k)) return false;
    seen.add(k);
    return true;
  });
  if (mods.length < before) done.push(`took out ${plural(before - mods.length, "copy", "copies")} listed twice`);

  const needs = report?.needs ?? [];
  // Requirements switched off.
  const off = needs.filter((n) => n.kind === "switchedOff").map((n) => n.modId);
  if (off.length) {
    mods = mods.map((m) => (off.includes(m.modId) ? { ...m, enabled: true } : m));
    done.push(`switched on ${plural(off.length, "requirement")}`);
  }
  // Requirements in the library but not in the profile.
  let added = 0;
  for (const n of needs.filter((n) => n.kind === "notInProfile")) {
    const f = newest(n.modId);
    if (!f || mods.some((m) => m.modId === n.modId)) continue;
    mods.push({ modId: n.modId, fileId: f.fileId, enabled: true });
    added++;
  }
  if (added) done.push(`added ${plural(added, "requirement")} from your library`);

  return { profile: { ...p, mods }, done, download: needs.filter((n) => n.kind === "notDownloaded").map((n) => n.modId) };
}

function plural(n: number, one: string, many = `${one}s`) {
  return `${n} ${n === 1 ? one : many}`;
}
