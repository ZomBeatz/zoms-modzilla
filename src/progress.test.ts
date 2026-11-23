import { describe, expect, it } from "vitest";
import type { CheckReport, Profile, StoredMod } from "./api";
import { autoFix, profileProgress } from "./progress";

const lib = (game: string, modId: number, fileId: number): StoredMod => ({ game, modId, name: "m", files: [{ fileId, name: "f", fileName: "f.7z", size: 1, downloadedAt: 0 }], folder: "" });

describe("profile progress", () => {
  it("counts switched-on mods whose exact file is in the library for that game", () => {
    const p: Profile = { id: "p", name: "P", game: "cyberpunk2077", mods: [
      { modId: 1, fileId: 10, enabled: true },
      { modId: 2, fileId: 20, enabled: true },
      { modId: 3, fileId: 30, enabled: false },
    ] };
    const library = [lib("cyberpunk2077", 1, 10), lib("cyberpunk2077", 2, 21), lib("starfield", 2, 20), lib("cyberpunk2077", 3, 30)];
    expect(profileProgress(p, library)).toEqual({ have: 1, total: 2 });
  });
});

describe("auto fix", () => {
  it("swaps deleted files, drops doubles, switches on and adds requirements, and leaves downloads to the caller", () => {
    const p: Profile = { id: "p", name: "P", game: "g", mods: [
      { modId: 1, fileId: 10, enabled: true },
      { modId: 1, fileId: 10, enabled: true },
      { modId: 2, fileId: 99, enabled: true },
      { modId: 3, fileId: 30, enabled: false },
    ] };
    const library = [lib("g", 1, 10), lib("g", 2, 21), lib("g", 3, 30), lib("g", 4, 40)];
    const need = (modId: number, kind: CheckReport["needs"][number]["kind"]) => ({ name: `M${modId}`, modId, kind, neededBy: ["x"] });
    const report = { needs: [need(3, "switchedOff"), need(4, "notInProfile"), need(5, "notDownloaded")] } as unknown as CheckReport;
    const r = autoFix(p, library, report);
    expect(r.profile.mods).toEqual([
      { modId: 1, fileId: 10, enabled: true },
      { modId: 2, fileId: 21, enabled: true },
      { modId: 3, fileId: 30, enabled: true },
      { modId: 4, fileId: 40, enabled: true },
    ]);
    expect(r.done).toHaveLength(4);
    expect(r.download).toEqual([5]);
  });
});
