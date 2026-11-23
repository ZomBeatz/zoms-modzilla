import { describe, expect, it } from "vitest";
import type { ModFacts } from "./api";
import { needersOf, softNote } from "./facts";

const f = (modId: number, requires: [number, string?][]): ModFacts => ({ modId, game: "g", name: `M${modId}`, status: "published", requires: requires.map(([id, notes]) => ({ modId: id, name: `M${id}`, otherGame: false, notes })), dlc: [] });

describe("requirements", () => {
  it("tells suggestions from musts", () => {
    expect(softNote("ADDON: modded vehicle list")).toBe(true);
    expect(softNote("Optional - MANDATORY for LEO addon")).toBe(true);
    expect(softNote("MANDATORY")).toBe(false);
    expect(softNote("Required for v3.xx")).toBe(false);
    expect(softNote(null)).toBe(false);
  });
  it("finds what needs a mod, through chains, ignoring add-ons and switched-off mods", () => {
    const facts = { "g:2": f(2, [[1]]), "g:3": f(3, [[2]]), "g:4": f(4, [[1, "ADDON"]]), "g:5": f(5, [[1]]) };
    const mods = [1, 2, 3, 4, 5].map((modId) => ({ modId, enabled: modId !== 5 }));
    expect(needersOf(1, "g", mods, facts).sort()).toEqual([2, 3]);
  });
});
