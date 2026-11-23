// What Nexus Mods says about mods (what they need, whether they're adult, their category and
// whether they're still published), shared by Downloads, Library and Profiles. One request per
// game for the mods not asked about yet this run; the app also keeps the answers for a day, so
// asking again after a restart is free.

import { useEffect, useState } from "react";
import { api, type ModFacts } from "./api";

const asked = new Set<string>();
const known = new Map<string, ModFacts>();
const listeners = new Set<() => void>();

export const factKey = (game: string, modId: number) => `${game}:${modId}`;

/** Ask about `keys` ("game:modId") not asked about yet. Local mods (huge made-up numbers) are skipped. */
export function askFacts(keys: string[]) {
  const byGame = new Map<string, number[]>();
  for (const k of keys) {
    if (asked.has(k)) continue;
    const [g, m] = k.split(":");
    const id = Number(m);
    if (!g || !id || id >= 1_000_000_000) continue;
    asked.add(k);
    byGame.set(g, [...(byGame.get(g) ?? []), id]);
  }
  for (const [g, ids] of byGame)
    api.modRequirements(g, ids).then(
      (found) => {
        for (const f of found) known.set(factKey(f.game, f.modId), f);
        listeners.forEach((l) => l());
      },
      () => ids.forEach((id) => asked.delete(factKey(g, id))),
    );
}

/** Facts for `keys`, filled in as answers come back. Pass the keys sorted and joined (stable). */
export function useFacts(keys: string[]): Record<string, ModFacts> {
  const [facts, setFacts] = useState<Record<string, ModFacts>>(() => Object.fromEntries(known));
  const joined = keys.join(",");
  useEffect(() => {
    const update = () => setFacts(Object.fromEntries(known));
    listeners.add(update);
    askFacts(joined ? joined.split(",") : []);
    update();
    return () => {
      listeners.delete(update);
    };
  }, [joined]);
  return facts;
}

/** Whether a mod is marked adult, from the library's note or Nexus Mods' answer. */
export const isAdult = (facts: Record<string, ModFacts>, game: string, modId: number, stored?: boolean) => !!stored || !!facts[factKey(game, modId)]?.adult;

/** True when Nexus Mods no longer shows the mod (hidden, removed, under moderation). */
export const isGone = (f?: ModFacts) => !!f && f.status !== "published";

/** The author's note makes a requirement a suggestion (matches the app's own rule in Rust). */
export function softNote(notes?: string | null): boolean {
  let n = (notes ?? "").toLowerCase();
  if (n.includes("mandatory") && !n.includes("optional")) return false;
  n = n.replaceAll("not required", "optional").replaceAll("not needed", "optional").replaceAll("isn't required", "optional").replaceAll("is not required", "optional");
  return n.includes("optional") || n.includes("addon") || n.includes("add-on") || n.includes("for devs") || n.includes("for developers") || (n.includes("recommend") && !n.includes("requir") && !n.includes("need"));
}

/** Switched-on mods in `mods` that need `id` (directly or through each other), from what Nexus Mods says. */
export function needersOf(id: number, game: string, mods: { modId: number; enabled: boolean }[], facts: Record<string, ModFacts>): number[] {
  const out: number[] = [];
  const todo = [id];
  while (todo.length) {
    const want = todo.pop()!;
    for (const m of mods) {
      if (!m.enabled || m.modId === id || out.includes(m.modId)) continue;
      if (facts[factKey(game, m.modId)]?.requires.some((q) => q.modId === want && !softNote(q.notes))) {
        out.push(m.modId);
        todo.push(m.modId);
      }
    }
  }
  return out;
}

/** What's known so far, for places that can't wait for an answer. */
export const knownFacts = (): Record<string, ModFacts> => Object.fromEntries(known);
