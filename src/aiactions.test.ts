import { describe, expect, it } from "vitest";
import { actionLabel, sameName, splitActions } from "./aiactions";

describe("actions in AI answers", () => {
  it("takes the block off and keeps known actions only", () => {
    const r = splitActions('Switch it off.\n\n```actions\n[{"do": "switchOff", "mod": "Quiet Footsteps"}, {"do": "rm -rf"}, {"do": "findProblem"}]\n```');
    expect(r.text).toBe("Switch it off.");
    expect(r.actions.map(actionLabel)).toEqual(["Switch off Quiet Footsteps", "Find the problem mod"]);
  });
  it("shows nothing for a half-written or missing block", () => {
    expect(splitActions('Hi\n```actions\n[{"do": "switch').actions).toEqual([]);
    expect(splitActions('Hi\n```actions\n[{"do": "switch').text).toBe("Hi");
    expect(splitActions("No block").actions).toEqual([]);
    expect(splitActions('```actions\n[{"do": "switchOff"}]\n```').actions).toEqual([]);
  });
  it("matches names loosely", () => {
    expect(sameName("DynamicReflections", "Dynamic Reflections")).toBe(true);
    expect(sameName("", "")).toBe(false);
  });
});
