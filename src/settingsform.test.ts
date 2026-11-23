import { describe, expect, it } from "vitest";
import { fieldsOf, setField } from "./settingsform";

describe("settings as a form", () => {
  it("reads BepInEx and ini files and changes one value only", () => {
    const cfg = "[General]\n\n## How loud it is\n# Setting type: Int32\n# Default value: 5\nVolume = 5\n\n# Acceptable values: Low, High\nQuality = Low\nOn = true\n";
    const f = fieldsOf("BepInEx/config/x.cfg", cfg)!;
    expect(f.map((x) => [x.section, x.key, x.kind])).toEqual([["General", "Volume", "number"], ["General", "Quality", "text"], ["General", "On", "bool"]]);
    expect(f[0].help).toBe("How loud it is");
    expect(f[1].choices).toEqual(["Low", "High"]);
    expect(setField("x.cfg", cfg, f[0], "9")).toBe(cfg.replace("Volume = 5", "Volume = 9"));
  });
  it("reads JSON values, nested too, and keeps the file's indent", () => {
    const json = '{\n    "a": 1,\n    "b": { "c": true, "d": "x" },\n    "list": [1]\n}\n';
    const f = fieldsOf("config.json", json)!;
    expect(f.map((x) => x.key)).toEqual(["a", "c", "d"]);
    const out = setField("config.json", json, f[1], "false");
    expect(JSON.parse(out).b.c).toBe(false);
    expect(out).toContain('\n    "a": 1');
    expect(fieldsOf("bad.json", "{ nope")).toBeNull();
  });
});
