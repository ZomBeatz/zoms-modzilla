// Getting started: the few steps a new person needs, shown at the top until they're done (or
// hidden). Each step says what it's for and has the button that does it.

import { useEffect, useState } from "react";
import { CheckCircle2, Circle, X } from "lucide-react";
import { api, type LinkStatus } from "./api";
import type { Ctx } from "./App";

const HIDDEN = "zoms-nexus:setup-hidden";
/** Settings > Show Getting started sets this, to show it even when every step is done. */
export const SHOW = "zoms-nexus:setup-show";

export function Setup({ overview, games, jobs, go, openAdd, toast, libraryVersion }: Ctx) {
  const [hidden, setHidden] = useState(() => {
    try {
      return localStorage.getItem(HIDDEN) === "1" && localStorage.getItem(SHOW) !== "1";
    } catch {
      return false;
    }
  });
  const forced = (() => {
    try {
      return localStorage.getItem(SHOW) === "1";
    } catch {
      return false;
    }
  })();
  const [links, setLinks] = useState<LinkStatus | null>(null);
  const [hasMods, setHasMods] = useState(false);
  useEffect(() => {
    api.linkStatus().then(setLinks, () => {});
  }, []);
  useEffect(() => {
    api.library().then((l) => setHasMods(l.length > 0), () => {});
  }, [libraryVersion]);

  const steps = [
    {
      done: overview.hasKey,
      title: "Connect your Nexus Mods account",
      text: "Paste your personal API key, from nexusmods.com > Settings > API keys. It stays on this computer.",
      action: <button className="btn sm primary" onClick={() => go("settings")}>Add API key</button>,
    },
    {
      done: !!links?.ours,
      title: "Let the website's download buttons open Zom's Nexus",
      text: "Then Mod manager download (and Vortex) buttons on nexusmods.com send files here.",
      action: (
        <button
          className="btn sm primary"
          onClick={() =>
            api.registerLinks().then(
              (s) => {
                setLinks(s);
                toast("Nexus Mods links now open in Zom's Nexus");
              },
              (e) => toast((e as Error).message),
            )
          }
        >
          Set it up
        </button>
      ),
    },
    {
      done: games.length > 0,
      title: "Find your games",
      text: "Steam games are found by themselves. Add others by hand, with their install folder.",
      action: <button className="btn sm primary" onClick={() => go("games")}>Go to Games</button>,
    },
    {
      done: hasMods || jobs.length > 0,
      title: "Get your first mod",
      text: "Click Mod manager download on a mod's Files tab, or paste a mod's or collection's address here.",
      action: <button className="btn sm primary" onClick={() => openAdd()}>Add mod…</button>,
    },
  ];
  const left = steps.filter((s) => !s.done).length;
  if (hidden || (left === 0 && !forced) || links === null) return null;
  const hide = () => {
    setHidden(true);
    try {
      localStorage.setItem(HIDDEN, "1");
      localStorage.removeItem(SHOW);
    } catch {
      /* no storage */
    }
  };
  const next = steps.findIndex((s) => !s.done);
  return (
    <div className="panel setup">
      <div className="row between">
        <h3 style={{ margin: 0 }}>{left === 0 ? "Getting started · all done" : `Getting started · ${steps.length - left} of ${steps.length} done`}</h3>
        <button className="icon-btn" aria-label="Hide getting started" title="Hide this (it's all in Settings too)" onClick={hide}>
          <X size={16} />
        </button>
      </div>
      <ol className="setup-steps">
        {steps.map((s, i) => (
          <li key={s.title} className={s.done ? "done" : i === next ? "next" : ""}>
            {s.done ? <CheckCircle2 size={18} color="var(--zb-ok)" /> : <Circle size={18} />}
            <div className="grow">
              <b>{s.title}</b>
              {!s.done && <div className="muted small">{s.text}</div>}
            </div>
            {!s.done && i === next && s.action}
          </li>
        ))}
      </ol>
    </div>
  );
}
