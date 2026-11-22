import { useState } from "react";
import { api } from "../api";
import { Dialog } from "../ui";

/** Your note on a mod, kept with the library and shown wherever the mod is used. */
export function NoteDialog({ game, modId, modName, note, onClose, onSaved }: { game: string; modId: number; modName: string; note: string; onClose: () => void; onSaved: (text: string) => void }) {
  const [text, setText] = useState(note);
  const [error, setError] = useState("");
  const save = async () => {
    try {
      await api.setNote(game, modId, text);
      onSaved(text.trim());
      onClose();
    } catch (e) {
      setError((e as Error).message);
    }
  };
  return (
    <Dialog
      title={`Note on “${modName}”`}
      onClose={onClose}
      foot={
        <>
          {note && (
            <button className="btn left danger" onClick={() => void api.setNote(game, modId, "").then(() => (onSaved(""), onClose()))}>
              Remove note
            </button>
          )}
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" onClick={save}>
            Save note
          </button>
        </>
      }
    >
      <textarea autoFocus rows={4} style={{ width: "100%" }} placeholder="Keep below Pine Forest Textures, or the trees go grey." value={text} onChange={(e) => setText(e.target.value)} maxLength={2000} />
      <p className="muted small">Only for you, on this computer. It shows in the library and in every profile with this mod.</p>
      {error && <p className="err-text">{error}</p>}
    </Dialog>
  );
}
