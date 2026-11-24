// The app actions an AI answer suggests, in a ```actions block at its end: shown as buttons the
// person presses (the AI never does anything by itself).

export type AiAction = { do: "switchOff" | "switchOn" | "findProblem" | "playWithoutMods" | "checkAgain" | "modSettings"; mod?: string };

const KNOWN = ["switchOff", "switchOn", "findProblem", "playWithoutMods", "checkAgain", "modSettings"];

/** The answer without its actions block, and the actions in it (unknown ones left out). */
export function splitActions(text: string): { text: string; actions: AiAction[] } {
  const m = /```actions\s*([\s\S]*?)(```|$)/.exec(text);
  if (!m) return { text, actions: [] };
  let actions: AiAction[] = [];
  try {
    const v = JSON.parse(m[1].trim()) as unknown;
    if (Array.isArray(v))
      actions = v
        .filter((a): a is AiAction => !!a && typeof a === "object" && KNOWN.includes((a as AiAction).do))
        .filter((a) => !["switchOff", "switchOn"].includes(a.do) || (typeof a.mod === "string" && a.mod.trim() !== ""))
        .slice(0, 6);
  } catch {
    /* half-written while streaming, or not JSON: no buttons */
  }
  return { text: text.slice(0, m.index).trimEnd(), actions };
}

export function actionLabel(a: AiAction): string {
  switch (a.do) {
    case "switchOff":
      return `Switch off ${a.mod}`;
    case "switchOn":
      return `Switch on ${a.mod}`;
    case "findProblem":
      return "Find the problem mod";
    case "playWithoutMods":
      return "Play without mods";
    case "checkAgain":
      return "Check again";
    case "modSettings":
      return "Open mod settings";
  }
}

/** "Dynamic Reflections" matches "DynamicReflections" and "dynamic reflections". */
export const sameName = (a: string, b: string) => {
  const k = (s: string) => s.toLowerCase().replace(/[^a-z0-9]/g, "");
  return k(a) !== "" && k(a) === k(b);
};
