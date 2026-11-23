import { describe, expect, it } from "vitest";
import { steamMatch } from "./pages/AddGame";

const installed = [
  { appId: 489830, name: "The Elder Scrolls V: Skyrim Special Edition", path: "/s/Skyrim Special Edition" },
  { appId: 1091500, name: "Cyberpunk 2077", path: "/s/Cyberpunk 2077" },
  { appId: 1, name: "Portal", path: "/s/Portal" },
];

describe("adding a game from Steam", () => {
  it("matches Nexus Mods names to Steam's, loosely but not wildly", () => {
    expect(steamMatch("Cyberpunk 2077", installed)?.appId).toBe(1091500);
    expect(steamMatch("Skyrim Special Edition", installed)?.appId).toBe(489830);
    expect(steamMatch("Skyrim", installed)).toBeUndefined();
    expect(steamMatch("Portal 2", installed)).toBeUndefined();
  });
});
