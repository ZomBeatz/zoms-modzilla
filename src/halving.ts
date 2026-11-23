// Finding the mod behind a problem by halves: switch half of the suspects off, play, say
// whether the problem is still there, and repeat until one is left. Kept per profile in this
// browser's storage, so it survives closing the app between rounds.

import type { ProfileMod } from "./api";

export interface Halving {
  /** The profile's mods as they were before, put back at the end. */
  original: ProfileMod[];
  /** Mods that could be the one. */
  suspects: number[];
  /** Of the suspects, the ones switched on this round. */
  on: number[];
  round: number;
}

const KEY = "zoms-nexus:halving";
function all(): Record<string, Halving> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}") as Record<string, Halving>;
  } catch {
    return {};
  }
}
export const halvingOf = (profile: string): Halving | null => all()[profile] ?? null;
export function setHalving(profile: string, h: Halving | null) {
  try {
    const a = all();
    if (h) a[profile] = h;
    else delete a[profile];
    localStorage.setItem(KEY, JSON.stringify(a));
  } catch {
    /* no storage: the round still works until the page closes */
  }
}

/** Split suspects: the first half stays on, plus whatever those need (so they still load). */
export function split(suspects: number[], needs: (id: number) => number[]): number[] {
  const on = new Set(suspects.slice(0, Math.ceil(suspects.length / 2)));
  const todo = [...on];
  while (todo.length) for (const n of needs(todo.pop()!)) if (suspects.includes(n) && !on.has(n)) on.add(n), todo.push(n);
  // Never all of them, or the round tells nothing.
  if (on.size === suspects.length && suspects.length > 1) on.delete(suspects[suspects.length - 1]);
  return suspects.filter((id) => on.has(id));
}

export function start(mods: ProfileMod[], needs: (id: number) => number[]): Halving {
  const suspects = mods.filter((m) => m.enabled).map((m) => m.modId);
  return { original: mods, suspects, on: split(suspects, needs), round: 1 };
}

/** The answer for this round: the problem still happens (it's among the ones on) or not. */
export function answer(h: Halving, stillThere: boolean, needs: (id: number) => number[]): Halving {
  const suspects = stillThere ? h.on : h.suspects.filter((id) => !h.on.includes(id));
  return { ...h, suspects, on: suspects.length > 1 ? split(suspects, needs) : suspects, round: h.round + 1 };
}

/** The profile's mods for this round: the originals, with suspects not on switched off. */
export const roundMods = (h: Halving): ProfileMod[] => h.original.map((m) => (m.enabled && h.suspects.includes(m.modId) && !h.on.includes(m.modId) ? { ...m, enabled: false } : m));
