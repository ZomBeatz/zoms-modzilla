import { describe, expect, it } from "vitest";
import type { CheckReport } from "./api";
import { problemsOf } from "./pages/Activity";

const clean: CheckReport = { conflicts: [], plugins: [], loadOrder: [], missing: [], installers: [], files: 0, at: 0, needs: [], compat: [], requirementsChecked: true };

describe("problemsOf", () => {
  it("lists what stops mods working, with the fixes it has", () => {
    const r: CheckReport = {
      ...clean,
      needs: [
        { name: "Snowy Roads", modId: 9004, kind: "notDownloaded", neededBy: ["Lanterns"] },
        { name: "Old Thing", modId: 5, kind: "unavailable", neededBy: ["Lanterns"] },
        { name: "Extra", modId: 6, kind: "optional", neededBy: ["Lanterns"] },
      ],
      launchFix: { missing: ["winmm"], current: "", suggested: "WINEDLLOVERRIDES=\"winmm=n,b\" %command%" },
      outOfDate: true,
    };
    const p = problemsOf(r);
    expect(p.map((x) => x.fix)).toEqual(["auto", undefined, "copy", "apply"]);
    expect(p[0].key).toBe("need:notDownloaded:9004");
    expect(problemsOf(clean)).toEqual([]);
  });

  it("leaves out game logs from before the last change", () => {
    const r: CheckReport = { ...clean, gameLog: { at: 1, beforeChange: true, issues: [{ modId: 1, modName: "A", kind: "oldPlugin", detail: "old" }] } };
    expect(problemsOf(r)).toEqual([]);
  });
});
