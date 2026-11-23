import { describe, expect, it } from "vitest";
import { merge } from "./merge";

describe("adding another profile's mods", () => {
  it("adds new ones after, switches on and updates ones there", () => {
    const r = merge(
      [
        { modId: 1, fileId: 10, enabled: true },
        { modId: 2, fileId: 20, enabled: false },
      ],
      [
        { modId: 2, fileId: 21, enabled: true },
        { modId: 3, fileId: 30, enabled: true },
        { modId: 1, fileId: 10, enabled: false },
      ],
    );
    expect(r.mods).toEqual([
      { modId: 1, fileId: 10, enabled: true },
      { modId: 2, fileId: 21, enabled: true },
      { modId: 3, fileId: 30, enabled: true },
    ]);
    expect([r.added, r.changed]).toEqual([1, 1]);
  });
});
