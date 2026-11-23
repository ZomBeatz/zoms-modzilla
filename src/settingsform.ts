// Mod settings files as a form: `key = value` lines (.ini, .cfg, .conf, .toml) and the plain
// values in a .json file. Changing a value rewrites only that value, so comments, order and
// everything else in the file stay exactly as they were.

export type Field = {
  /** Where it is: the line for text files, the key path for JSON. */
  at: number | string[];
  section: string;
  key: string;
  value: string;
  kind: "bool" | "number" | "text";
  /** The comment lines just above it, or BepInEx's "# Setting type" notes. */
  help: string;
  /** BepInEx lists the values it accepts: "# Acceptable values: Low, High". */
  choices?: string[];
};

const kindOf = (v: string): Field["kind"] => (/^(true|false)$/i.test(v.trim()) ? "bool" : /^-?\d+(\.\d+)?$/.test(v.trim()) ? "number" : "text");

/** The fields of a `key = value` file. */
export function lineFields(text: string): Field[] {
  const out: Field[] = [];
  let section = "";
  let help: string[] = [];
  let choices: string[] | undefined;
  text.split("\n").forEach((raw, i) => {
    const line = raw.trim();
    if (!line) {
      help = [];
      choices = undefined;
      return;
    }
    const sec = /^\[(.+)\]$/.exec(line);
    if (sec) {
      section = sec[1];
      help = [];
      return;
    }
    if (line.startsWith("#") || line.startsWith(";") || line.startsWith("//")) {
      const c = line.replace(/^(#+|;+|\/\/)\s?/, "");
      const acc = /^Acceptable values:\s*(.+)$/i.exec(c);
      if (acc) choices = acc[1].split(",").map((x) => x.trim()).filter(Boolean);
      // BepInEx's own notes say what the value is, not what it does.
      else if (!/^(Setting type|Default value):/i.test(c)) help.push(c);
      return;
    }
    const m = /^([^=]+?)\s*=\s*(.*)$/.exec(line);
    if (!m) return;
    const value = m[2].replace(/^"(.*)"$/, "$1");
    out.push({ at: i, section, key: m[1].trim(), value, kind: choices ? "text" : kindOf(value), help: help.join(" "), choices });
    help = [];
    choices = undefined;
  });
  return out;
}

/** The plain values (true/false, numbers, text) in a JSON file, nested ones too. */
export function jsonFields(text: string): Field[] | null {
  let doc: unknown;
  try {
    doc = JSON.parse(text);
  } catch {
    return null;
  }
  const out: Field[] = [];
  const walk = (v: unknown, path: string[]) => {
    if (v && typeof v === "object" && !Array.isArray(v)) for (const [k, x] of Object.entries(v)) walk(x, [...path, k]);
    else if (typeof v === "boolean" || typeof v === "number" || typeof v === "string") out.push({ at: path, section: path.slice(0, -1).join(" › "), key: path[path.length - 1] ?? "", value: String(v), kind: typeof v === "boolean" ? "bool" : typeof v === "number" ? "number" : "text", help: "" });
  };
  walk(doc, []);
  return out;
}

/** The fields of a settings file, or null when it's only editable as text. */
export function fieldsOf(path: string, text: string): Field[] | null {
  const ext = path.toLowerCase().split(".").pop() ?? "";
  if (ext === "json") return jsonFields(text);
  if (["ini", "cfg", "conf", "toml"].includes(ext)) {
    const f = lineFields(text);
    return f.length ? f : null;
  }
  return null;
}

/** `text` with one field's value changed. */
export function setField(path: string, text: string, f: Field, value: string): string {
  if (Array.isArray(f.at)) {
    const doc = JSON.parse(text) as Record<string, unknown>;
    let o = doc as Record<string, unknown>;
    for (const k of f.at.slice(0, -1)) o = o[k] as Record<string, unknown>;
    const last = f.at[f.at.length - 1];
    o[last] = f.kind === "bool" ? value === "true" : f.kind === "number" && value.trim() !== "" && !isNaN(Number(value)) ? Number(value) : value;
    const indent = /\n(\s+)"/.exec(text)?.[1] ?? "  ";
    return JSON.stringify(doc, null, indent) + (text.endsWith("\n") ? "\n" : "");
  }
  const lines = text.split("\n");
  const line = lines[f.at];
  const m = /^(\s*[^=]+?\s*=\s*)(.*?)(\r?)$/.exec(line);
  if (!m) return text;
  const quoted = /^".*"$/.test(m[2]);
  lines[f.at] = `${m[1]}${quoted ? `"${value}"` : value}${m[3]}`;
  void path;
  return lines.join("\n");
}
