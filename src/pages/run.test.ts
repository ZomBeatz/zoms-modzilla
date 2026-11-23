import { describe, expect, it } from "vitest";
import { changes, runLength } from "./RunPanel";

describe("what changed since the last good run", () => {
  it("says what was added, updated and taken out", () => {
    const t = { at: 0, secs: 9, times: 1, added: [{ modId: 1, name: "A" }], updated: [{ modId: 2, name: "B" }], removed: [], changedSince: false };
    expect(changes(t)).toBe("added A; updated B");
    expect(changes({ ...t, added: [1, 2, 3, 4].map((n) => ({ modId: n, name: `M${n}` })), updated: [] })).toBe("added M1, M2, M3 and 1 more");
    expect(runLength(7800)).toBe("2 h 10 min");
  });
});

import { protonTip } from "./RunPanel";
describe("the Proton tip", () => {
  it("only for stock Proton, and says what to do", () => {
    expect(protonTip("GE-Proton9-20", [], "Skyrim", true)).toBeNull();
    expect(protonTip("Proton 9.0-302", ["GE-Proton9-20"], "Skyrim", true)).toContain("GE-Proton9-20 is installed");
    expect(protonTip("Proton 9.0-302", [], "Skyrim", true)).toContain("ProtonUp-Qt");
  });
});
