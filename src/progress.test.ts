import { describe, expect, it } from "vitest";
import type { Profile, StoredMod } from "./api";
import { profileProgress } from "./progress";

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
