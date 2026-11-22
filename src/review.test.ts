import { describe, expect, it } from "vitest";
import type { FileUsage, Profile, StoredMod } from "./api";
import { findItems } from "./pages/ReviewSpace";

const file = (fileId: number, unpacked = true) => ({ fileId, name: `File ${fileId}`, fileName: `f${fileId}.7z`, size: 100, downloadedAt: 0, archive: `f${fileId}.7z`, unpacked: unpacked ? `f${fileId}` : null });
const mod = (modId: number, files: ReturnType<typeof file>[]): StoredMod => ({ game: "g", modId, name: `Mod ${modId}`, files, folder: "" });

describe("review space", () => {
  it("finds unused mods, old files and spare archives, biggest first", () => {
    const mods = [mod(1, [file(10), file(11)]), mod(2, [file(20)]), mod(3, [file(30, false)])];
    const profiles: Profile[] = [{ id: "p", name: "P", game: "g", mods: [{ modId: 1, fileId: 11, enabled: true }, { modId: 3, fileId: 30, enabled: false }] }];
    const usage: FileUsage[] = [
      { game: "g", modId: 1, fileId: 10, archive: 100, unpacked: 300 },
      { game: "g", modId: 1, fileId: 11, archive: 150, unpacked: 400 },
      { game: "g", modId: 2, fileId: 20, archive: 1000, unpacked: 2000 },
      { game: "g", modId: 3, fileId: 30, archive: 50, unpacked: 0 },
    ];
    const items = findItems(mods, profiles, usage).map((i) => [i.kind, i.label, i.size]);
    expect(items).toEqual([
      ["unused", "Mod 2", 3000],
      ["old", "Mod 1", 400],
      ["archive", "Mod 1", 150],
    ]);
  });
});
