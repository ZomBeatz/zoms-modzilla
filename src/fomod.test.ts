import { describe, expect, it } from "vitest";
import type { Installer } from "./api";
import { problem, startPicks, toChoices, walk } from "./fomod";

const inst: Installer = {
  name: "Snow",
  steps: [
    {
      name: "Look",
      groups: [
        {
          name: "Snow",
          kind: "SelectExactlyOne",
          plugins: [
            { name: "Soft", description: "", flags: [["snow", "soft"]], kind: "Optional", patterns: [] },
            { name: "Hard", description: "", flags: [["snow", "hard"]], kind: "Recommended", patterns: [] },
          ],
        },
      ],
    },
    {
      name: "Soft extras",
      visible: { flag: "snow", value: "soft" },
      groups: [{ name: "Extra", kind: "SelectAny", plugins: [{ name: "Sparkle", description: "", flags: [], kind: "Optional", patterns: [{ when: { flag: "snow", value: "soft" }, kind: "Required" }] }] }],
    },
  ],
};

describe("mod installers", () => {
  it("shows a step only when its flags are set", () => {
    expect(walk(inst, { Look: { Snow: ["Hard"] } }).map((w) => w.step.name)).toEqual(["Look"]);
    expect(walk(inst, { Look: { Snow: ["Soft"] } }).map((w) => w.step.name)).toEqual(["Look", "Soft extras"]);
  });
  it("starts with the recommended option and keeps forced ones", () => {
    expect(startPicks(inst.steps[0].groups[0], {})).toEqual(["Hard"]);
    expect(startPicks(inst.steps[0].groups[0], {}, ["Soft"])).toEqual(["Soft"]);
    const c = toChoices(inst, { Look: { Snow: ["Soft"] } });
    expect(c).toEqual([
      { step: "Look", group: "Snow", plugins: ["Soft"] },
      { step: "Soft extras", group: "Extra", plugins: ["Sparkle"] },
    ]);
  });
  it("says when a group's picks don't fit", () => {
    expect(problem(inst.steps[0].groups[0], [], {})).toBe("Pick one.");
    expect(problem(inst.steps[0].groups[0], ["Soft"], {})).toBeNull();
  });
});
