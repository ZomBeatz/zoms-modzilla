import { describe, expect, it } from "vitest";
import type { Game } from "./api";
import { ordered } from "./GamePicker";

const g = (domain: string, name: string): Game => ({ domain, name, source: "steam" });

describe("game picker order", () => {
  it("puts recently picked games first, then the rest by name", () => {
    const games = [g("starfield", "Starfield"), g("cyberpunk2077", "Cyberpunk 2077"), g("skyrimspecialedition", "Skyrim Special Edition"), g("stardewvalley", "Stardew Valley")];
    const out = ordered(games, ["stardewvalley", "gone", "starfield"]).map((x) => x.domain);
    expect(out).toEqual(["stardewvalley", "starfield", "cyberpunk2077", "skyrimspecialedition"]);
  });
});
