import { GoneList } from "./GoneGames";
import { useEffect, useState } from "react";
import { Compass, ExternalLink, FolderOpen, Gamepad2, Info, Layers, Package, Pencil, Play, Plus, Trash2 } from "lucide-react";
import { TmlPanel } from "./TmlPanel";
import { api, playWithoutMods, isDesktop, modFolderHelp, openUrl, pickFolder, type Game, type GameState, type StoredMod } from "../api";
import { usePlayLabel } from "../GameStatus";
import type { Ctx } from "../App";
import { Cover } from "../art";
import { bytes, plural } from "../format";
import { SteamFolders } from "./SteamFolders";
import { AddGame } from "./AddGame";
import { Dialog } from "../ui";

/** Add a game, or change where an existing one is and where its mods go. Every folder can be
 * picked with Browse; the mod folder is kept relative to the install folder. */
export function GameDialog({ game, onClose, onSaved, focusModFolder }: { game?: Game; onClose: () => void; onSaved: () => void; focusModFolder?: boolean }) {
  const [domain, setDomain] = useState(game?.domain ?? "");
  const [name, setName] = useState(game?.name ?? "");
  const [path, setPath] = useState(game?.path ?? "");
  // null: not known yet. "": straight into the install folder. Otherwise a folder inside it.
  const [modFolder, setModFolder] = useState<string | null>(game?.modFolder ?? null);
  const [launch, setLaunch] = useState(game?.launch ?? "");
  const [saveFolder, setSaveFolder] = useState(game?.saveFolder ?? "");
  const [error, setError] = useState("");
  const steam = game?.source === "steam";
  const shownName = name || game?.name || domain;
  const save = async () => {
    try {
      await api.addGame(domain, name, path, modFolder === null ? null : modFolder, launch, null, saveFolder.trim() || null);
      onSaved();
      onClose();
    } catch (e) {
      setError((e as Error).message);
    }
  };
  /** A folder picked anywhere, as a path inside the install folder (or why it can't be). */
  const pickModFolder = async () => {
    setError("");
    const picked = await pickFolder("Where this game's mods go (inside its install folder)", path ? `${path}/${modFolder ?? ""}` : undefined);
    if (!picked) return;
    const root = path.replace(/\/+$/, "");
    if (!root) return setError("Choose the install folder first.");
    if (picked === root) return setModFolder("");
    if (!picked.startsWith(`${root}/`)) return setError("Pick a folder inside the install folder. Mods for this game go somewhere in there.");
    setModFolder(picked.slice(root.length + 1));
  };
  return (
    <Dialog
      title={game ? game.name : "Add game"}
      onClose={onClose}
      foot={
        <>
          {shownName && !game?.modTool && (
            <button className="btn left" title={`Searches the web for where ${shownName} mods go`} onClick={() => void openUrl(modFolderHelp(shownName))}>
              <ExternalLink size={14} /> Help
            </button>
          )}
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
        <div className="field full">
          <span>Install folder</span>
          <div className="row gap-sm">
            <input className="grow" placeholder={steam ? "Found in Steam" : "Where the game is installed"} value={path} onChange={(e) => setPath(e.target.value)} aria-label="Install folder" />
            {isDesktop && (
              <button className="btn" onClick={async () => setPath((await pickFolder("Where the game is installed", path || undefined)) ?? path)}>
                <FolderOpen size={14} /> Browse…
              </button>
            )}
          </div>
        </div>
        <div className="field full">
          <span>Mod folder</span>
          {game?.modTool ? (
            <span className="hint">{game.modTool}</span>
          ) : (
            <>
              <label className="check">
                <input type="checkbox" checked={modFolder === ""} onChange={(e) => setModFolder(e.target.checked ? "" : null)} /> Mods go straight into the install folder
              </label>
              {modFolder !== "" && (
                <div className="row gap-sm">
                  <input className="grow" autoFocus={focusModFolder} placeholder="Not set yet, for example Data or Mods" value={modFolder ?? ""} onChange={(e) => setModFolder(e.target.value.trim() ? e.target.value : null)} aria-label="Mod folder" />
                  {isDesktop && (
                    <button className="btn" onClick={() => void pickModFolder()}>
                      <FolderOpen size={14} /> Browse…
                    </button>
                  )}
                </div>
              )}
              <span className="hint">
                Where mods, textures and skins go: <b>Data</b> for Skyrim and Fallout, <b>Mods</b> for Stardew Valley with SMAPI, the install folder itself for Cyberpunk 2077.{" "}
              </span>
            </>
          )}
        </div>
        <div className="field full">
          <span>Save folder</span>
          <div className="row gap-sm">
            <input className="grow" placeholder="Not known: only needed for saves per profile" value={saveFolder} onChange={(e) => setSaveFolder(e.target.value)} aria-label="Save folder" />
            {isDesktop && (
              <button className="btn" onClick={async () => setSaveFolder((await pickFolder("Where the game keeps its saves", saveFolder || undefined)) ?? saveFolder)}>
                <FolderOpen size={14} /> Browse…
              </button>
            )}
          </div>
          <span className="hint">Where the game keeps its saves. Profiles set to keep their own saves swap this folder when they go into the game. Known for most big games already.</span>
        </div>
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

/** A folder for a game's mods on the game's own drive: beside its Steam library ("…/SteamLibrary/Zom's Nexus mods"), or next to the game's folder. */
export function sameDriveFolder(gamePath: string): string | null {
  const i = gamePath.indexOf("/steamapps/");
  if (i > 0) return `${gamePath.slice(0, i)}/Zom's Nexus mods`;
  const parent = gamePath.replace(/\/+$/, "").split("/").slice(0, -1).join("/");
  return parent ? `${parent}/Zom's Nexus mods` : null;
}

export function Games({ toast, go, libraryVersion, reloadGames, gameStates }: Ctx) {
  const [games, setGames] = useState<Game[] | null>(null);
  const [mods, setMods] = useState<StoredMod[]>([]);
  const [editing, setEditing] = useState<{ game?: Game; focus?: boolean } | null>(null);
  const [adding, setAdding] = useState(false);
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
        <button className="btn" onClick={() => setAdding(true)}>
          <Plus size={15} /> Add game…
        </button>
      </div>
      <p className="muted" style={{ marginTop: -8 }}>
        Games Steam has installed are found by themselves, in every Steam library folder (listed below), whether they run natively or through Proton. Add any other game by hand.
      </p>
      <GoneList toast={toast} />
      {games === null ? null : games.length === 0 ? (
        <div className="empty">
          <Gamepad2 size={32} />
          <p>No Nexus Mods games were found in Steam. Add a game by hand to keep its mods together.</p>
          <button className="btn primary" onClick={() => setAdding(true)}>
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
                  {g.source === "steam" ? (g.protonPrefix ? `Steam, ${g.proton ?? "Proton"}` : "Steam") : g.source === "heroic" ? `Heroic${g.proton ? `, ${g.proton}` : ""}` : g.source === "lutris" ? `Lutris${g.proton ? `, ${g.proton}` : ""}` : "Added by you"} · {count(g.domain) ? plural(count(g.domain), "mod") : "no mods yet"}
                </div>
                {g.freeSpace != null && g.totalSpace && g.freeSpace / g.totalSpace < 0.1 && (
                  <div className="warn-text small" title="Mods copied into this game need room on its drive">
                    Its drive is nearly full: {bytes(g.freeSpace)} left
                  </div>
                )}
                {g.modTool ? (
                  <div className="muted small row gap-sm" title={g.modTool}>
                    <Info size={12} /> Uses its own mod tool
                  </div>
                ) : (
                  g.modFolder == null && (
                    <div className="small row gap-sm wrap">
                      <button className="link-name accent-link" onClick={() => setEditing({ game: g, focus: true })}>
                        Mod folder not set · Set it
                      </button>
                    </div>
                  )
                )}
                {g.modsCopied && (
                  <div className="muted small row gap-sm wrap" title="Your mod library is on another drive (an SD card, say), so Play copies mods into this game instead of linking them: twice the space, and slower for big packs.">
                    Mods are copied here (library on another drive)
                    {g.path && sameDriveFolder(g.path) && (
                      <button className="btn sm ghost" onClick={() => act(api.moveGameLibrary(g.domain, sameDriveFolder(g.path!)!), `Moving ${g.name}'s mods to its own drive`)} title={`Keeps ${g.name}'s mods in ${sameDriveFolder(g.path)}, so they're linked, not copied`}>
                        Keep its mods on its drive
                      </button>
                    )}
                  </div>
                )}
                <div className="row gap-sm" style={{ marginTop: 8 }}>
                  {(g.steamAppId || g.launch) && (
                    <GamePlay state={gameStates[g.domain]} onPlay={() => act(api.launch(g.domain), `Starting ${g.name}`)} />
                  )}
                  {g.activeProfile && (g.steamAppId || g.launch) && (
                    <button className="btn sm ghost" title="Take the mods out and start the game as it came" onClick={() => act(playWithoutMods(g.domain), `Starting ${g.name} without mods`)}>
                      Without mods
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
                  <button className="icon-btn" title="Folders and start command" aria-label="Edit game" onClick={() => setEditing({ game: g })}>
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
      <TmlPanel toast={toast} />
      <SteamFolders />
      {adding && (
        <AddGame
          games={games ?? []}
          toast={toast}
          onClose={() => setAdding(false)}
          onAdded={load}
          onManual={() => {
            setAdding(false);
            setEditing({});
          }}
        />
      )}
      {editing && <GameDialog game={editing.game} focusModFolder={editing.focus} onClose={() => setEditing(null)} onSaved={load} />}
    </div>
  );
}

/** The card's Play, showing whether the game is starting or running. */
function GamePlay({ state, onPlay }: { state?: GameState; onPlay: () => void }) {
  const p = usePlayLabel(state);
  return (
    <button className={`btn sm ${p.tone === "ok" ? "playing" : p.tone === "bad" ? "play-bad" : "primary"}`} disabled={p.busy} onClick={onPlay} title={state?.state === "failed" ? state.detail ?? undefined : "Start the game with the mods that are in it now"}>
      <Play size={13} /> {p.label ?? "Play"}
    </button>
  );
}
