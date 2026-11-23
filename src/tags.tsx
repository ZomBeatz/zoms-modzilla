// Your tags on mods ("textures", "must have"), shared by Library and Profiles. Kept by the app.

import { useEffect, useState } from "react";
import { Tag } from "lucide-react";
import { api } from "./api";
import { Dialog } from "./ui";

let all: Record<string, string[]> = {};
let loaded = false;
const listeners = new Set<(t: Record<string, string[]>) => void>();
const tell = () => listeners.forEach((l) => l({ ...all }));

export function useTags(): Record<string, string[]> {
  const [t, setT] = useState(all);
  useEffect(() => {
    listeners.add(setT);
    if (!loaded) {
      loaded = true;
      api.tags().then((x) => {
        all = x;
        tell();
      }, () => (loaded = false));
    }
    return () => void listeners.delete(setT);
  }, []);
  return t;
}

export async function saveTags(game: string, modId: number, tags: string[]) {
  all = await api.setTags(game, modId, tags);
  tell();
}

/** Every tag in use for these "game:mod" keys, sorted. */
export const tagsIn = (t: Record<string, string[]>, keys: string[]) => [...new Set(keys.flatMap((k) => t[k] ?? []))].sort((a, b) => a.localeCompare(b));

export function TagChips({ tags }: { tags?: string[] }) {
  if (!tags?.length) return null;
  return (
    <>
      {tags.map((t) => (
        <span key={t} className="pill tiny-pill tag-chip">
          <Tag size={10} /> {t}
        </span>
      ))}
    </>
  );
}

export function TagDialog({ game, modId, name, known, onClose, toast }: { game: string; modId: number; name: string; known: string[]; onClose: () => void; toast: (t: string) => void }) {
  const t = useTags();
  const [text, setText] = useState((t[`${game}:${modId}`] ?? []).join(", "));
  const list = text.split(",").map((x) => x.trim()).filter(Boolean);
  const add = (k: string) => !list.some((x) => x.toLowerCase() === k.toLowerCase()) && setText([...list, k].join(", "));
  const save = async () => {
    try {
      await saveTags(game, modId, list);
      onClose();
    } catch (e) {
      toast((e as Error).message);
    }
  };
  return (
    <Dialog
      title={`Tags for “${name}”`}
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" onClick={save}>
            Save
          </button>
        </>
      }
    >
      <label className="field">
        <span>Tags, separated by commas</span>
        <input autoFocus value={text} placeholder="textures, must have" onChange={(e) => setText(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void save()} />
      </label>
      {known.filter((k) => !list.includes(k)).length > 0 && (
        <div className="row gap-sm wrap" style={{ marginTop: 8 }}>
          <span className="muted small">Used before:</span>
          {known
            .filter((k) => !list.includes(k))
            .map((k) => (
              <button key={k} className="pill tiny-pill pill-btn" onClick={() => add(k)}>
                + {k}
              </button>
            ))}
        </div>
      )}
    </Dialog>
  );
}
