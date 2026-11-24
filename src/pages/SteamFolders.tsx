import { useEffect, useState } from "react";
import { FolderSearch } from "lucide-react";
import { api, type SteamLibrary } from "../api";
import { plural } from "../format";
import { DiskBar } from "../ui";

/** Where Zom's Modzilla looks for games: each Steam library folder, and what it found there. */
export function SteamFolders() {
  const [libs, setLibs] = useState<SteamLibrary[] | null>(null);
  useEffect(() => {
    api.steamLibraries().then(setLibs, () => setLibs([]));
  }, []);
  if (libs === null) return null;
  return (
    <div className="panel" style={{ marginTop: 20 }}>
      <h3>
        <FolderSearch size={15} /> Where games are found
      </h3>
      {libs.length === 0 ? (
        <p className="muted">
          Steam isn't installed here (looked in ~/.local/share/Steam, ~/.steam, and the Flatpak and Snap folders), so only games you add by hand are listed.
        </p>
      ) : (
        <>
          <p className="muted small" style={{ marginTop: 0 }}>
            Steam's library folders, read from its libraryfolders.vdf. A game shows up above when Steam has it installed in one of these and it's a game Zom's Modzilla knows on Nexus Mods. Add a library folder in Steam (Settings &gt; Storage) and it's searched too.
          </p>
          <div className="steam-libs">
            {libs.map((l) => (
              <div key={l.path} className="steam-lib">
                <div style={{ minWidth: 0 }}>
                  <div className="mono small ellipsis" title={l.path}>
                    {l.path}
                  </div>
                  <div className="sub">
                    {plural(l.installed, "Steam install")}
                    {l.nexusGames.length ? ` · on Nexus Mods: ${l.nexusGames.join(", ")}` : " · none Zom's Modzilla knows yet"}
                  </div>
                </div>
                <DiskBar free={l.freeSpace} total={l.totalSpace} />
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
