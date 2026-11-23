import { describe, expect, it } from "vitest";
import type { Game, Profile } from "./api";
import { whyAsk } from "./SavesGate";

const game = { domain: "g", name: "G", saveFolder: "/saves", activeProfile: "Shared one" } as Game;
const shared: Profile = { id: "a", name: "Shared one", game: "g", mods: [] };
const own: Profile = { id: "b", name: "Own one", game: "g", mods: [], ownSaves: true };
const other: Profile = { id: "c", name: "Another", game: "g", mods: [] };

describe("asking about saves before Play", () => {
  it("asks when the saves would switch, every time", () => {
    expect(whyAsk(own, game, [shared, own], ["b"])).toBe("switch");
  });
  it("asks the first time, then not when nothing switches", () => {
    expect(whyAsk(other, game, [shared, other], [])).toBe("first");
    expect(whyAsk(other, game, [shared, other], ["c"])).toBeNull();
  });
  it("says nothing when the save folder isn't known", () => {
    expect(whyAsk(own, { ...game, saveFolder: null }, [shared, own], [])).toBeNull();
  });
});
