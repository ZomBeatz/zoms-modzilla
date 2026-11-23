// Mod installers (FOMOD) on the page: which steps show, what each option is now (patterns can
// change it as flags are set) and what starts picked. Matches the app's own rules in fomod.rs.

import type { FomodChoice, FomodCond, FomodGroup, FomodPlugin, FomodStep, Installer } from "./api";

export type Flags = Record<string, string>;
/** Picks by step, then group: plugin names. */
export type Picks = Record<string, Record<string, string[]>>;

export function holds(c: FomodCond, flags: Flags): boolean {
  if ("all" in c) return c.all.every((x) => holds(x, flags));
  if ("any" in c) return c.any.length === 0 || c.any.some((x) => holds(x, flags));
  if ("flag" in c) return (flags[c.flag] ?? "") === c.value;
  return c.fixed;
}

/** What an option is with the flags set so far. */
export const kindOf = (p: FomodPlugin, flags: Flags) => p.patterns.find((pt) => holds(pt.when, flags))?.kind ?? p.kind;
const is = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();
export const usable = (p: FomodPlugin, flags: Flags) => !is(kindOf(p, flags), "NotUsable");
export const forced = (g: FomodGroup, p: FomodPlugin, flags: Flags) => is(g.kind, "SelectAll") || is(kindOf(p, flags), "Required");

/** The options picked in a group, forced ones included and unusable ones left out. */
export function pickedIn(g: FomodGroup, chosen: string[], flags: Flags): FomodPlugin[] {
  return g.plugins.filter((p) => usable(p, flags) && (forced(g, p, flags) || chosen.includes(p.name)));
}

/** The steps that show with these picks, and the flags as each one starts. */
export function walk(inst: Installer, picks: Picks): { step: FomodStep; flags: Flags }[] {
  const flags: Flags = {};
  const out: { step: FomodStep; flags: Flags }[] = [];
  for (const s of inst.steps) {
    if (s.visible && !holds(s.visible, flags)) continue;
    out.push({ step: s, flags: { ...flags } });
    for (const g of s.groups) for (const p of pickedIn(g, picks[s.name]?.[g.name] ?? [], flags)) for (const [k, v] of p.flags) flags[k] = v;
  }
  return out;
}

/** Starting picks for a step's group: last time's when they still fit, else the recommended ones. */
export function startPicks(g: FomodGroup, flags: Flags, before?: string[]): string[] {
  const ok = g.plugins.filter((p) => usable(p, flags));
  const one = is(g.kind, "SelectExactlyOne") || is(g.kind, "SelectAtMostOne");
  if (before) {
    const kept = before.filter((n) => ok.some((p) => p.name === n));
    if (kept.length || !is(g.kind, "SelectExactlyOne")) return one ? kept.slice(0, 1) : kept;
  }
  let want = ok.filter((p) => ["recommended", "required"].includes(kindOf(p, flags).toLowerCase())).map((p) => p.name);
  if (one) want = want.slice(0, 1);
  if (!want.length && (is(g.kind, "SelectExactlyOne") || is(g.kind, "SelectAtLeastOne")) && ok[0]) want = [ok[0].name];
  return want;
}

/** What's wrong with a group's picks, if anything. */
export function problem(g: FomodGroup, chosen: string[], flags: Flags): string | null {
  const n = pickedIn(g, chosen, flags).length;
  if (is(g.kind, "SelectExactlyOne") && n !== 1) return "Pick one.";
  if (is(g.kind, "SelectAtLeastOne") && n < 1) return "Pick at least one.";
  if (is(g.kind, "SelectAtMostOne") && n > 1) return "Pick one at most.";
  return null;
}

/** Picks as the app keeps them: visible steps only, forced options included. */
export function toChoices(inst: Installer, picks: Picks): FomodChoice[] {
  return walk(inst, picks).flatMap(({ step, flags }) => step.groups.map((g) => ({ step: step.name, group: g.name, plugins: pickedIn(g, picks[step.name]?.[g.name] ?? [], flags).map((p) => p.name) })));
}

export function fromChoices(c?: FomodChoice[] | null): Picks | null {
  if (!c) return null;
  const out: Picks = {};
  for (const x of c) (out[x.step] ??= {})[x.group] = x.plugins;
  return out;
}
