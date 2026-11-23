import { describe, expect, it } from "vitest";
import { pick, type Box } from "./pad";

const box = (left: number, top: number, w = 80, h = 30): Box => ({ left, top, right: left + w, bottom: top + h });

describe("moving with the D-pad", () => {
  // A row of three buttons, and a wide one below the middle.
  const row = [box(0, 0), box(100, 0), box(200, 0), box(60, 60, 200)];
  it("goes to the nearest in that direction", () => {
    expect(pick(row[0], "right", row)).toBe(1);
    expect(pick(row[2], "left", row)).toBe(1);
    expect(pick(row[1], "down", row)).toBe(3);
    expect(pick(row[3], "up", row)).toBe(1);
  });
  it("prefers the one in line over a nearer one off to the side", () => {
    const grid = [box(0, 0), box(0, 100), box(300, 40)];
    expect(pick(grid[0], "down", grid)).toBe(1);
  });
  it("stops at the edge", () => {
    expect(pick(row[2], "right", row)).toBe(-1);
    expect(pick(row[0], "up", row)).toBe(-1);
  });
});
