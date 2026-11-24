// "Only <game>" on pages that cover every game (Downloads, Activity). Off by default; each page
// remembers its own choice on this computer.

import { useState } from "react";

export function useOnlyGame(page: string): [boolean, (on: boolean) => void] {
  const key = `zoms-modzilla:only-game:${page}`;
  const [only, setOnly] = useState(() => {
    try {
      return localStorage.getItem(key) === "1";
    } catch {
      return false;
    }
  });
  const set = (on: boolean) => {
    setOnly(on);
    try {
      if (on) localStorage.setItem(key, "1");
      else localStorage.removeItem(key);
    } catch {
      /* not kept, still works */
    }
  };
  return [only, set];
}

export function OnlyGameToggle({ only, onChange, gameName }: { only: boolean; onChange: (on: boolean) => void; gameName?: string }) {
  if (!gameName) return null;
  return (
    <label className="check" title={`Show only ${gameName}, the game picked at the top`}>
      <input type="checkbox" checked={only} onChange={(e) => onChange(e.target.checked)} /> Only {gameName}
    </label>
  );
}
