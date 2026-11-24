// The first time Zom's Modzilla opens on a Steam Deck: everything that makes it feel at home there,
// in one place. Shown once (Settings › Steam Deck can show it again).

import { useEffect, useState } from "react";
import { Check, Gamepad2 } from "lucide-react";
import { api, on, type Game } from "./api";
import { getDevice } from "./deck";
import { AddToSteam } from "./pages/AddToSteam";
import { sameDriveFolder } from "./pages/Games";
import { Dialog } from "./ui";

const DONE = "zoms-modzilla:deck-setup";

export const showDeckSetup = () => window.dispatchEvent(new Event("zoms:deck-setup"));

export function DeckSetup({ games, hasKey, toast, goSettings, reloadGames }: { games: Game[]; hasKey: boolean; toast: (t: string) => void; goSettings: () => void; reloadGames: () => void }) {
  const [open, setOpen] = useState(false);
  const [moved, setMoved] = useState(false);
  useEffect(() => {
    const show = () => setOpen(true);
    window.addEventListener("zoms:deck-setup", show);
    void getDevice().then((d) => {
      let done = false;
      try {
        done = localStorage.getItem(DONE) === "1";
      } catch {
        /* asked again next time */
      }
      if ((d.deck || d.gameMode) && !done) setOpen(true);
    });
    // The controller works: any press ticks it off.
    const off = on("pad", () => setMoved(true));
    const key = (e: KeyboardEvent) => e.key.startsWith("Arrow") && setMoved(true);
    window.addEventListener("keydown", key);
    return () => {
      window.removeEventListener("zoms:deck-setup", show);
      window.removeEventListener("keydown", key);
      void off.then((f) => f());
    };
  }, []);
  if (!open) return null;
  const copied = games.filter((g) => g.modsCopied && g.path && sameDriveFolder(g.path));
  const finish = () => {
    try {
      localStorage.setItem(DONE, "1");
    } catch {
      /* fine */
    }
    setOpen(false);
  };
  return (
    <Dialog
      wide
      title="Set up for the Steam Deck"
      onClose={finish}
      foot={
        <>
          <button className="btn ghost left" onClick={() => setOpen(false)}>
            Remind me next time
          </button>
          <button className="btn primary" onClick={finish}>
            Done
          </button>
        </>
      }
    >
      <ol className="deck-steps">
        <li>
          <b>Open it from Game Mode.</b>
          <AddToSteam toast={toast} />
        </li>
        <li>
          <b>The controller.</b> {moved ? <span className="ok-text"><Check size={14} /> It works.</span> : <>Press the D-pad now: the highlight should move.</>}
          <div className="muted small">
            <Gamepad2 size={13} /> A selects, B goes back, X opens a row's menu, Y plays, L1/R1 change page, the triggers scroll, ☰ finds anything. If nothing moves in Game Mode, set Zom's Modzilla's controller layout in Steam to <i>Gamepad</i>.
          </div>
        </li>
        <li>
          <b>Games on the SD card.</b>{" "}
          {copied.length === 0 ? (
            <span className="muted">Every game's mods are on the same drive as the game, so they're linked, not copied.</span>
          ) : (
            <>
              <span className="muted">These games are on another drive than your mods, so mods would be copied (twice the space). Keep each one's mods on its own drive:</span>
              <div className="row gap-sm wrap" style={{ marginTop: 6 }}>
                {copied.map((g) => (
                  <button
                    key={g.domain}
                    className="btn sm"
                    onClick={async () => {
                      try {
                        await api.moveGameLibrary(g.domain, sameDriveFolder(g.path!)!);
                        toast(`${g.name}'s mods are kept on its drive now`);
                        reloadGames();
                      } catch (e) {
                        toast((e as Error).message);
                      }
                    }}
                  >
                    {g.name}
                  </button>
                ))}
              </div>
            </>
          )}
        </li>
        <li>
          <b>Your Nexus Mods account.</b>{" "}
          {hasKey ? (
            <span className="ok-text">
              <Check size={14} /> Signed in.
            </span>
          ) : (
            <>
              <button className="btn sm primary" onClick={() => { setOpen(false); goSettings(); }}>
                Sign in
              </button>
            </>
          )}
          <div className="muted small">Typing: choosing a text box opens Steam's keyboard (STEAM + X any time).</div>
        </li>
      </ol>
    </Dialog>
  );
}
