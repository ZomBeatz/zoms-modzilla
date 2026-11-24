// A profile another page asked Profiles to open (the Activity page's "Open" buttons).
let wanted: string | null = null;
const wanters = new Set<() => void>();
export const wantProfile = (id: string | null) => {
  wanted = id;
  wanters.forEach((w) => w());
};
/** Hear about profiles asked for while the Profiles page is already open. */
export function onWantProfile(f: () => void) {
  wanters.add(f);
  return () => void wanters.delete(f);
}
/** The profile to open, once. */
export const takeWantedProfile = () => {
  const id = wanted;
  wanted = null;
  return id;
};

/** The profile last opened for each game, kept in this browser's storage. */
const LAST = "zoms-modzilla:last-profile";
export function rememberedProfile(game: string): string | null {
  try {
    return (JSON.parse(localStorage.getItem(LAST) ?? "{}") as Record<string, string>)[game] ?? null;
  } catch {
    return null;
  }
}
export function rememberProfile(game: string, id: string) {
  try {
    const all = JSON.parse(localStorage.getItem(LAST) ?? "{}") as Record<string, string>;
    all[game] = id;
    localStorage.setItem(LAST, JSON.stringify(all));
  } catch {
    /* no storage */
  }
}

/** Mods switched off to test a crash, per profile, and when (so they can be switched back on later). */
const TRIED = "zoms-modzilla:crash-test";
type Tried = { ids: number[]; at: number };
function allTried(): Record<string, Tried> {
  try {
    const all = JSON.parse(localStorage.getItem(TRIED) ?? "{}") as Record<string, Tried | number[]>;
    // Older entries were just a list.
    return Object.fromEntries(Object.entries(all).map(([k, v]) => [k, Array.isArray(v) ? { ids: v, at: 0 } : v]));
  } catch {
    return {};
  }
}
export function triedOff(profile: string): Tried | null {
  return allTried()[profile] ?? null;
}
export function setTriedOff(profile: string, ids: number[]) {
  try {
    const all = allTried();
    if (ids.length) all[profile] = { ids, at: Math.floor(Date.now() / 1000) };
    else delete all[profile];
    localStorage.setItem(TRIED, JSON.stringify(all));
  } catch {
    /* no storage */
  }
}

/** Profiles opened lately, across games (newest first, at most 5), for the game picker. */
const RECENT = "zoms-modzilla:recent-profiles";
export type RecentProfile = { id: string; name: string; game: string };
export function recentProfiles(): RecentProfile[] {
  try {
    return JSON.parse(localStorage.getItem(RECENT) ?? "[]") as RecentProfile[];
  } catch {
    return [];
  }
}
export function rememberRecent(p: RecentProfile) {
  try {
    localStorage.setItem(RECENT, JSON.stringify([p, ...recentProfiles().filter((x) => x.id !== p.id)].slice(0, 5)));
  } catch {
    /* no storage */
  }
}
