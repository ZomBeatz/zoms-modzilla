import { describe, expect, it } from "vitest";
import { tidy } from "./ui";

describe("right-click menus", () => {
  it("drops left-out entries and stray separators", () => {
    const a = { label: "A", run: () => {} };
    const b = { label: "B", run: () => {} };
    expect(tidy(["sep", false, a, "sep", null, "sep", b, "sep", undefined])).toEqual([a, "sep", b]);
    expect(tidy([false, "sep"])).toEqual([]);
  });
});
