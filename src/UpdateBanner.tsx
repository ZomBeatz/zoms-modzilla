// A newer Zom's Modzilla is out: say so once per version, with what's new and the download.

import { useEffect, useState } from "react";
import { Sparkles, X } from "lucide-react";
import { api, openUrl } from "./api";

const SEEN = "zoms-modzilla:update-seen";

export function UpdateBanner() {
  const [r, setR] = useState<{ version: string; url: string; notes: string; appImage?: string | null } | null>(null);
  useEffect(() => {
    // A little after starting, so it never slows the first screen.
    const t = setTimeout(() => {
      api.appUpdate().then(
        (u) => {
          let seen = "";
          try {
            seen = localStorage.getItem(SEEN) ?? "";
          } catch {
            /* shown again next time */
          }
          if (u.release && u.release.version !== seen) setR(u.release);
        },
        () => {},
      );
    }, 4000);
    return () => clearTimeout(t);
  }, []);
  if (!r) return null;
  const close = () => {
    try {
      localStorage.setItem(SEEN, r.version);
    } catch {
      /* fine */
    }
    setR(null);
  };
  return (
    <div className="panel collection-note accent update-banner">
      <Sparkles size={16} color="var(--zb-accent)" />
      <span className="grow small">
        <b>Zom's Modzilla {r.version} is out.</b> {r.notes.split("\n").find((l) => l.trim() && !l.startsWith("#"))?.slice(0, 140)}
      </span>
      <button className="btn sm" onClick={() => void openUrl(r.url)}>
        What's new
      </button>
      {r.appImage && (
        <button className="btn sm primary" onClick={() => void openUrl(r.appImage!)}>
          Download
        </button>
      )}
      <button className="icon-btn" aria-label="Not now" title="Not now (until the next version)" onClick={close}>
        <X size={14} />
      </button>
    </div>
  );
}
