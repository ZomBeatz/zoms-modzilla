import { describe, expect, it } from "vitest";
import { bytes, eta, percent, plural } from "./format";

describe("format", () => {
  it("writes sizes with 3 significant digits and decimal units", () => {
    expect(bytes(0)).toBe("0 B");
    expect(bytes(512)).toBe("512 B");
    expect(bytes(1_920_000_000)).toBe("1.92 GB");
    expect(bytes(12_400_000)).toBe("12.4 MB");
    expect(bytes(512_000_000)).toBe("512 MB");
    expect(bytes(999_999)).toBe("1.00 MB");
  });
  it("says how long is left", () => {
    expect(eta(100, 0)).toBe("");
    expect(eta(30, 1)).toBe("30 s");
    expect(eta(600, 1)).toBe("10 min");
    expect(eta(3900, 1)).toBe("1 h 5 min");
  });
  it("counts properly", () => {
    expect(plural(1, "file")).toBe("1 file");
    expect(plural(3, "file")).toBe("3 files");
    expect(percent(50, 200)).toBe(25);
    expect(percent(5, 0)).toBe(0);
  });
});
