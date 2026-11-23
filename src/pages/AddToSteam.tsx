// Zom's Nexus as a game in Steam's library (with artwork), so a Steam Deck opens it from Game
// Mode with the controller. Steam has to be closed while its list changes; Zom's Nexus can close
// and reopen it, after asking.

import { useEffect, useState } from "react";
import { Gamepad2 } from "lucide-react";
import { api } from "../api";
import { Dialog } from "../ui";

type Entry = Awaited<ReturnType<typeof api.steamEntry>>;

export function AddToSteam({ toast }: { toast: (t: string) => void }) {
  const [e, setE] = useState<Entry | null>(null);
  const [busy, setBusy] = useState(false);
  const [ask, setAsk] = useState(false);
  const load = () => api.steamEntry().then(setE, () => setE(null));
  useEffect(() => {
    void load();
  }, []);
  if (!e || !e.hasSteam) return null;
  const add = async (restart: boolean) => {
    setBusy(true);
    setAsk(false);
    try {
      const was = await api.steamAdd(restart);
      toast(was ? "Updated Zom's Nexus in Steam's library" : restart ? "Added to Steam's library. Steam is starting again: find it under Non-Steam." : "Added to Steam's library: find it under Non-Steam next time Steam starts");
    } catch (err) {
      const m = (err as Error).message;
      if (m === "steamRunning") setAsk(true);
      else toast(m);
    } finally {
      setBusy(false);
      void load();
    }
  };
  return (
    <div className="add-steam">
      <Gamepad2 size={20} color="var(--zb-accent)" />
      <div className="grow small">
        <b>{e.added ? "Zom's Nexus is in your Steam library" : "Add Zom's Nexus to Steam"}</b>
        <div className="muted">
          {e.added ? "Open it from Game Mode under Library › Non-Steam, with the controller. Add it again after moving the app to update where Steam looks." : "As a game in your library, with artwork, so Game Mode opens it full screen with the controller."}
          {e.dev ? " This is a development build: Steam would start it from the build folder." : ""}
        </div>
      </div>
      <button className={`btn sm${e.added ? "" : " primary"}`} disabled={busy} onClick={() => void add(false)}>
        {busy ? "Adding…" : e.added ? "Add again" : "Add to Steam"}
      </button>
      {ask && (
        <Dialog
          title="Steam is open"
          onClose={() => setAsk(false)}
          foot={
            <>
              <button className="btn" onClick={() => setAsk(false)}>
                Cancel
              </button>
              <button className="btn primary" onClick={() => void add(true)}>
                Close Steam, add, reopen
              </button>
            </>
          }
        >
          <p style={{ marginTop: 0 }}>Steam rewrites its list of games when it closes, so it has to be closed while Zom's Nexus adds itself. Zom's Nexus can close Steam, add itself, and start Steam again (a game running from Steam closes too).</p>
          <p className="muted small">Or close Steam yourself (Steam › Exit) and press Add to Steam again. A copy of Steam's list is kept beside it first.</p>
        </Dialog>
      )}
    </div>
  );
}
