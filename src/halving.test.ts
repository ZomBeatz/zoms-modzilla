import { describe, expect, it } from "vitest";
import { answer, roundMods, start } from "./halving";

const mods = [1, 2, 3, 4, 5, 6, 7, 8].map((m) => ({ modId: m, fileId: m * 10, enabled: m !== 8 }));
const none = () => [];

describe("finding a mod by halves", () => {
  it("narrows down to one in a few rounds", () => {
    const bad = 6;
    let h = start(mods, none);
    expect(h.suspects).toEqual([1, 2, 3, 4, 5, 6, 7]);
    while (h.suspects.length > 1) h = answer(h, h.on.includes(bad), none);
    expect(h.suspects).toEqual([bad]);
    expect(h.round).toBeLessThanOrEqual(4);
  });
  it("keeps what an on mod needs switched on, and never touches mods that were off", () => {
    const h = start(mods, (id) => (id === 1 ? [7] : []));
    expect(h.on).toContain(7);
    expect(roundMods(h).find((m) => m.modId === 8)?.enabled).toBe(false);
  });
});
