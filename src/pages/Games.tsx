import { useEffect, useState } from "react";
import { Compass, FolderOpen, Gamepad2, Layers, Package, Pencil, Play, Plus, Trash2 } from "lucide-react";
import { api, isDesktop, pickFolder, type Game, type StoredMod } from "../api";
import type { Ctx } from "../App";
import { Cover } from "../art";
import { plural } from "../format";
import { SteamFolders } from "./SteamFolders";
import { Dialog, DiskBar } from "../ui";

/** Add a game, or change where an existing one is and where its mods go. */
export function GameDialog({ game, onClose, onSaved }: { game?: Game; onClose: () => void; onSaved: () => void }) {
  const [domain, setDomain] = useState(game?.domain ?? "");
  const [name, setName] = useState(game?.name ?? "");
  const [path, setPath] = useState(game?.path ?? "");
  const [modFolder, setModFolder] = useState(game?.modFolder ?? "");
  const [launch, setLaunch] = useState(game?.launch ?? "");
  const [error, setError] = useState("");
  const steam = game?.source === "steam";
  const save = async () => {
    try {
      await api.addGame(domain, name, path, modFolder, launch);
      onSaved();
      onClose();
    } catch (e) {
      setError((e as Error).message);
    }
  };
  return (
    <Dialog
      title={game ? game.name : "Add game"}
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" disabled={!domain.trim()} onClick={save}>
            {game ? "Save" : "Add game"}
          </button>
        </>
      }
    >
      <div className="form-grid">
        {!game && (
          <label className="field full">
            <span>Game on Nexus Mods</span>
            <input autoFocus placeholder="skyrimspecialedition" value={domain} onChange={(e) => setDomain(e.target.value)} />
            <span className="hint">The game's name as it's written in its Nexus Mods address, or paste the address of its page.</span>
          </label>
        )}
        {!steam && (
          <label className="field full">
            <span>Name</span>
            <input placeholder="Skyrim Special Edition" value={name} onChange={(e) => setName(e.target.value)} />
          </label>
        )}
        <label className="field full">
          <span>Install folder</span>
          <div className="row gap-sm">
            <input className="grow" placeholder={steam ? "Found in Steam" : "Where the game is installed"} value={path} onChange={(e) => setPath(e.target.value)} />
            {isDesktop && (
              <button className="btn" onClick={async () => setPath((await pickFolder("Where the game is installed", path || undefined)) ?? path)}>
                Choose…
              </button>
            )}
          </div>
        </label>
        <label className="field full">
          <span>Mod folder</span>
          <input placeholder="Data" value={modFolder} onChange={(e) => setModFolder(e.target.value)} />
          <span className="hint">
            Where mods, textures and skins go, inside the install folder: <b>Data</b> for Skyrim and Fallout, <b>Mods</b> for Stardew Valley with SMAPI. Leave it empty when mods go straight into the install folder (Cyberpunk 2077).
          </span>
        </label>
        {!steam && (
          <label className="field full">
            <span>Start command</span>
            <input placeholder="./StardewModdingAPI" value={launch} onChange={(e) => setLaunch(e.target.value)} />
            <span className="hint">Runs in the install folder when you press <b>Play</b>. Steam games start through Steam.</span>
          </label>
        )}
        {error && <span className="err full" style={{ color: "var(--zb-danger)", fontSize: 12 }}>{error}</span>}
      </div>
    </Dialog>
  );
}

export function Games({ toast, go, libraryVersion, reloadGames }: Ctx) {
  const [games, setGames] = useState<Game[] | null>(null);
  const [mods, setMods] = useState<StoredMod[]>([]);
  const [editing, setEditing] = useState<Game | "new" | null>(null);
  const load = () => {
    api.games().then(setGames, (e) => toast((e as Error).message));
    reloadGames();
    api.library().then(setMods, () => {});
  };
  useEffect(load, [libraryVersion]); // eslint-disable-line react-hooks/exhaustive-deps
  const act = (p: Promise<unknown>, done?: string) =>
    p.then(
      () => {
        if (done) toast(done);
        load();
      },
      (e) => toast((e as Error).message),
    );
  const count = (d: string) => mods.filter((m) => m.game === d).length;

  return (
    <div className="page">
      <div className="page-head">
        <h1>Games</h1>
        <button className="btn" onClick={() => setEditing("new")}>
          <Plus size={15} /> Add game…
        </button>
      </div>
      <p className="muted" style={{ marginTop: -8 }}>
        Games Steam has installed are found by themselves, in every Steam library folder (listed below), whether they run natively or through Proton. Add any other game by hand.
      </p>
      {games === null ? null : games.length === 0 ? (
        <div className="empty">
          <Gamepad2 size={32} />
          <p>No Nexus Mods games were found in Steam. Add a game by hand to keep its mods together.</p>
          <button className="btn primary" onClick={() => setEditing("new")}>
            Add game…
          </button>
        </div>
      ) : (
        <div className="game-grid">
          {games.map((g) => (
            <div key={g.domain} className="game-card">
              <button className="game-art" onClick={() => go("library", g.domain)} title={`Pick ${g.name} and show its mods`}>
                <Cover domain={g.domain} name={g.name} />
                {g.activeProfile && (
                  <span className="pill ok game-badge">
                    <Layers size={11} /> {g.activeProfile}
                  </span>
                )}
              </button>
              <div className="game-info">
                <div className="name ellipsis" title={g.name}>
                  {g.name}
                </div>
                <div className="sub">
                  {g.source === "steam" ? (g.protonPrefix ? "Steam, Proton" : "Steam") : "Added by you"} · {count(g.domain) ? plural(count(g.domain), "mod") : "no mods yet"}
                </div>
                {g.freeSpace != null && (
                  <div style={{ marginTop: 6 }}>
                    <DiskBar free={g.freeSpace} total={g.totalSpace} label="its drive" />
                  </div>
                )}
                {g.modFolder == null && <div className="warn-text">Set where its mods go</div>}
                <div className="row gap-sm" style={{ marginTop: 8 }}>
                  {(g.steamAppId || g.launch) && (
                    <button className="btn sm primary" onClick={() => act(api.launch(g.domain), `Starting ${g.name}`)} title="Start the game with the mods that are in it now">
                      <Play size={13} /> Play
                    </button>
                  )}
                  <span className="grow" />
                  <button className="icon-btn" title="Browse mods" aria-label="Browse mods" onClick={() => go("browse", g.domain)}>
                    <Compass size={15} />
                  </button>
                  <button className="icon-btn" title="Profiles" aria-label="Profiles" onClick={() => go("profiles", g.domain)}>
                    <Layers size={15} />
                  </button>
                  {count(g.domain) > 0 && (
                    <button className="icon-btn" title="Its mods" aria-label="Its mods" onClick={() => go("library", g.domain)}>
                      <Package size={15} />
                    </button>
                  )}
                  {g.path && (
                    <button className="icon-btn" title="Open install folder" aria-label="Open install folder" onClick={() => act(api.openFolder({ game: g.domain, install: true }))}>
                      <FolderOpen size={15} />
                    </button>
                  )}
                  <button className="icon-btn" title="Folders and start command" aria-label="Edit game" onClick={() => setEditing(g)}>
                    <Pencil size={15} />
                  </button>
                  {g.source === "added" && (
                    <button className="icon-btn danger" title="Take off the list" aria-label="Take off the list" onClick={() => act(api.removeGame(g.domain))}>
                      <Trash2 size={15} />
                    </button>
                  )}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
      <SteamFolders />
      {editing && <GameDialog game={editing === "new" ? undefined : editing} onClose={() => setEditing(null)} onSaved={load} />}
    </div>
  );
}
