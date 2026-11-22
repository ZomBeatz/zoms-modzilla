import { useEffect, useRef, useState } from "react";
import { Archive, ArrowUpCircle, ExternalLink, FileUp, FolderOpen, HardDrive, Layers, Package, PackageOpen, Plus, RefreshCw, Search, StickyNote, Trash2 } from "lucide-react";
import { api, isDesktop, modUrl, openUrl, pickFile, type Game, type Profile, type StoredFile, type StoredMod, type UpdateReport } from "../api";
import { Cover, GameTag, Hero } from "../art";
import { AddToProfile } from "./AddToProfile";
import { ReviewSpace, usedKeys } from "./ReviewSpace";
import { NoteDialog } from "./NoteDialog";
import type { Ctx } from "../App";
import { bytes, plural, when } from "../format";
import { Confirm, Dialog, tidy } from "../ui";

/** Which game a mod from your own files is for, when no game is picked. */
function ImportFor({ games, onPick, onClose }: { games: Game[]; onPick: (g: string) => void; onClose: () => void }) {
  const [g, setG] = useState(games[0]?.domain ?? "");
  return (
    <Dialog
      title="Add a mod from your files"
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" disabled={!g} onClick={() => onPick(g)}>
            Choose file…
          </button>
        </>
      }
    >
      <p className="muted" style={{ marginTop: 0 }}>
        For mods from other sites, or your own: pick its zip, 7z or rar file. It's copied into the library and works like any other mod.
      </p>
      <label className="field">
        <span>Game</span>
        <select value={g} onChange={(e) => setG(e.target.value)}>
          {games.map((x) => (
            <option key={x.domain} value={x.domain}>
              {x.name}
            </option>
          ))}
        </select>
      </label>
    </Dialog>
  );
}

export function Library({ toast, libraryVersion, openAdd, overview, game: picked, games: gameList, menu }: Ctx) {
  const [mods, setMods] = useState<StoredMod[] | null>(null);
  const game = picked ?? "all";
  const [importing, setImporting] = useState(false);
  const [reviewing, setReviewing] = useState(false);
  const [notes, setNotes] = useState<Record<string, string>>({});
  const [noting, setNoting] = useState<StoredMod | null>(null);
  useEffect(() => {
    api.notes().then(setNotes, () => {});
  }, []);
  // Which mods and files profiles use, to mark the rest.
  const [profiles, setProfiles] = useState<Profile[]>([]);
  useEffect(() => {
    api.profiles().then(setProfiles, () => {});
  }, [libraryVersion]);
  const used = usedKeys(profiles);
  const [profiling, setProfiling] = useState<{ mod: StoredMod; file: StoredFile } | null>(null);
  const [updates, setUpdates] = useState<UpdateReport | null>(null);
  const [checking, setChecking] = useState(false);
  const [filter, setFilter] = useState("");
  const [deleting, setDeleting] = useState<{ mod: StoredMod; file?: StoredFile } | null>(null);
  const [busy, setBusy] = useState<number | null>(null);
  const search = useRef<HTMLInputElement>(null);

  const load = () => api.library().then(setMods, (e) => toast((e as Error).message));
  useEffect(() => {
    void load();
  }, [libraryVersion]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    api.lastUpdates().then(setUpdates, () => {});
  }, []);

  const checkUpdates = async () => {
    setChecking(true);
    try {
      const r = await api.checkUpdates();
      setUpdates(r);
      const n = r.updates.filter((u) => u.newer).length;
      toast(n ? `${plural(n, "update")} found` : `All ${plural(r.checked, "mod")} are up to date`);
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setChecking(false);
    }
  };
  const updateFor = (m: StoredMod, f: StoredFile) => updates?.updates.find((u) => u.game === m.game && u.modId === m.modId && u.fileId === f.fileId);
  const waiting = updates?.updates.filter((u) => u.newer && !(mods ?? []).some((m) => m.modId === u.modId && m.files.some((f) => f.fileId === u.newer!.fileId))) ?? [];

  // "/" filters the list.
  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      if (e.key !== "/" || (e.target as HTMLElement).closest("input, textarea, select")) return;
      e.preventDefault();
      search.current?.focus();
    };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, []);

  const gameName = (d: string) => gameList.find((g) => g.domain === d)?.name ?? d;
  const importFile = async (g: string) => {
    setImporting(false);
    const path = await pickFile("Pick a mod's zip, 7z or rar file", "mod");
    if (!path) return;
    toast("Adding the mod…");
    await act(api.importMod(g, path).then((m) => toast(`Added “${m.name}” to the library`)));
  };
  const shown = (mods ?? []).filter((m) => (game === "all" || m.game === game) && (!filter || `${m.name} ${m.author ?? ""}`.toLowerCase().includes(filter.toLowerCase())));
  const total = (mods ?? []).reduce((n, m) => n + m.files.reduce((k, f) => k + f.size, 0), 0);

  const act = async (p: Promise<unknown>, done?: string) => {
    try {
      await p;
      if (done) toast(done);
      await load();
    } catch (e) {
      toast((e as Error).message);
    }
  };

  const unpack = async (m: StoredMod, f: StoredFile) => {
    setBusy(f.fileId);
    await act(api.unpack(m.game, m.modId, f.fileId), `Unpacked “${f.name}”`);
    setBusy(null);
  };

  return (
    <div className="page">
      <div className="page-head">
        <h1>Library</h1>
        {mods && mods.length > 0 && (
          <span className="muted num">
            {plural(mods.length, "mod")} · {bytes(total)}
          </span>
        )}
        {mods && mods.length > 0 && overview.hasKey && (
          <button className="btn" disabled={checking} onClick={checkUpdates} title="Asks Nexus Mods about each mod (one request per mod)">
            <RefreshCw size={15} className={checking ? "spin" : ""} /> {checking ? "Checking…" : "Check for updates"}
          </button>
        )}
        {waiting.length > 1 && (
          <button
            className="btn primary"
            onClick={async () => {
              for (const u of waiting) await api.add(u.game, u.modId, u.newer!.fileId).catch(() => {});
              toast(`Added ${plural(waiting.length, "update")} to downloads`);
            }}
          >
            <ArrowUpCircle size={15} /> Get all {waiting.length} updates
          </button>
        )}
        {mods && mods.length > 0 && (
          <button className="btn" onClick={() => setReviewing(true)} title="Find mods and files that take space without being used">
            <HardDrive size={15} /> Review space
          </button>
        )}
        <button className="btn" onClick={() => act(api.openFolder())} title={overview.settings.libraryDir}>
          <FolderOpen size={15} /> Open library folder
        </button>
        {isDesktop && (
          <button className="btn" onClick={() => (picked ? void importFile(picked) : setImporting(true))} title="A mod from another site, or your own: its zip, 7z or rar file">
            <FileUp size={15} /> Add from file…
          </button>
        )}
        <button className="btn primary" onClick={() => openAdd()} title="Add a mod or collection (N)">
          <Plus size={15} /> Add mod…
        </button>
      </div>
      {game !== "all" && mods && mods.some((m) => m.game === game) && <Hero domain={game} name={gameName(game)} />}
      {mods && mods.length > 0 && (
        <div className="toolbar">
          <span className="grow" />
          <label className="search">
            <Search size={14} />
            <input ref={search} placeholder="Filter mods  /" value={filter} onChange={(e) => setFilter(e.target.value)} aria-label="Filter mods" />
          </label>
        </div>
      )}
      {mods === null ? null : shown.length === 0 && !filter ? (
        <div className="empty">
          <Package size={32} />
          <p>{game === "all" ? "Mods you download are kept here, one folder per mod, sorted by game." : `${gameName(game)} mods you download are kept here, one folder per mod.`}</p>
          <button className="btn primary" onClick={() => openAdd()}>
            Add mod…
          </button>
        </div>
      ) : shown.length === 0 ? (
        <div className="empty">
          <Search size={28} />
          <p>No mods match “{filter}”.</p>
        </div>
      ) : (
        <div className="mods">
          {shown.map((m) => (
            <div
              key={`${m.game}-${m.modId}`}
              className="panel mod-card"
              onContextMenu={(e) => {
                const newest = [...m.files].sort((a, b) => b.downloadedAt - a.downloadedAt)[0];
                const packed = m.files.find((f) => !f.unpacked && f.archive);
                menu(
                  e,
                  tidy([
                    newest && { label: "Add to profile…", icon: <Layers size={14} />, run: () => setProfiling({ mod: m, file: newest }) },
                    packed && { label: "Unpack", icon: <PackageOpen size={14} />, run: () => void unpack(m, packed) },
                    { label: notes[`${m.game}:${m.modId}`] ? "Change note…" : "Add a note…", icon: <StickyNote size={14} />, run: () => setNoting(m) },
                    { label: "Open folder", icon: <FolderOpen size={14} />, run: () => void act(api.openFolder({ game: m.game, modId: m.modId })) },
                    !m.local && { label: "Open on Nexus Mods", icon: <ExternalLink size={14} />, run: () => void act(openUrl(modUrl(m.game, m.modId))) },
                    "sep",
                    { label: "Delete mod…", icon: <Trash2 size={14} />, danger: true, run: () => setDeleting({ mod: m }) },
                  ]),
                );
              }}
            >
              <div className="thumb">{m.pictureUrl ? <img className="thumb" src={m.pictureUrl} alt="" loading="lazy" /> : <Cover domain={m.game} name={m.game} />}</div>
              <div className="grow">
                <div className="row gap">
                  <h3 className="grow">{m.name}</h3>
                  {m.local ? (
                    <span className="pill" title="Added from a file on this computer">Your file</span>
                  ) : (
                    <button className="icon-btn" title="Open on Nexus Mods" aria-label="Open on Nexus Mods" onClick={() => act(openUrl(modUrl(m.game, m.modId)))}>
                      <ExternalLink size={15} />
                    </button>
                  )}
                  <button className="icon-btn" title="Open folder" aria-label="Open folder" onClick={() => act(api.openFolder({ game: m.game, modId: m.modId }))}>
                    <FolderOpen size={15} />
                  </button>
                  <button className="icon-btn danger" title="Delete mod" aria-label="Delete mod" onClick={() => setDeleting({ mod: m })}>
                    <Trash2 size={15} />
                  </button>
                </div>
                <div className="meta">
                  {!used.mods.has(`${m.game}:${m.modId}`) && (
                    <span className="pill" title="No profile uses this mod. Review space can clear it.">
                      Not in any profile
                    </span>
                  )}
                  <GameTag domain={m.game} />
                  {m.author && <span>by {m.author}</span>}
                  {m.version && <span>version {m.version}</span>}
                </div>
                {notes[`${m.game}:${m.modId}`] && (
                  <button className="note link-name" title="Change your note" onClick={() => setNoting(m)}>
                    “{notes[`${m.game}:${m.modId}`]}”
                  </button>
                )}
                <div className="files">
                  {m.files.map((f) => (
                    <div key={f.fileId} className="file-row">
                      {f.unpacked ? <PackageOpen size={14} className="muted" /> : <Archive size={14} className="muted" />}
                      <span className="grow ellipsis" title={f.fileName}>
                        {f.name}
                        {f.version ? ` · ${f.version}` : ""}
                      </span>
                      {(() => {
                        const u = updateFor(m, f);
                        if (u?.newer)
                          return (
                            <button className="btn sm primary" title={`Download ${u.newer.name} ${u.newer.version ?? ""}`} onClick={() => act(api.add(m.game, m.modId, u.newer!.fileId), `Added the update to “${m.name}” to downloads`)}>
                              <ArrowUpCircle size={13} /> Update{u.newer.version ? ` to ${u.newer.version}` : ""}
                            </button>
                          );
                        if (u?.outdated) return <span className="pill warn" title="Nexus Mods lists this file as an old version">Old version</span>;
                        return null;
                      })()}
                      {used.mods.has(`${m.game}:${m.modId}`) && !used.files.has(`${m.game}:${m.modId}:${f.fileId}`) && (
                        <span className="pill" title="Your profiles use another file of this mod">
                          Not used
                        </span>
                      )}
                      {f.verified === true && (
                        <span className="pill ok" title="Matches Nexus Mods' copy of this file">
                          Checked
                        </span>
                      )}
                      {f.verified === false && (
                        <span className="pill warn" title="This file doesn't match any file Nexus Mods knows. It may have been damaged while downloading: delete it and download it again.">
                          Doesn't match
                        </span>
                      )}
                      {f.unpacked ? <span className="pill ok">Unpacked</span> : <span className="pill">Archive</span>}
                      <span className="muted small num hide-narrow">
                        {bytes(f.size)} · {when(f.downloadedAt)}
                      </span>
                      {!f.unpacked && f.archive && (
                        <button className="btn sm" disabled={busy === f.fileId} onClick={() => unpack(m, f)}>
                          {busy === f.fileId ? "Unpacking…" : "Unpack"}
                        </button>
                      )}
                      <button className="icon-btn" title="Add to a profile" aria-label="Add to a profile" onClick={() => setProfiling({ mod: m, file: f })}>
                        <Layers size={14} />
                      </button>
                      {m.files.length > 1 && (
                        <button className="icon-btn danger" title="Delete this file" aria-label="Delete this file" onClick={() => setDeleting({ mod: m, file: f })}>
                          <Trash2 size={14} />
                        </button>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}
      {noting && (
        <NoteDialog
          game={noting.game}
          modId={noting.modId}
          modName={noting.name}
          note={notes[`${noting.game}:${noting.modId}`] ?? ""}
          onClose={() => setNoting(null)}
          onSaved={(t) => {
            const k = `${noting.game}:${noting.modId}`;
            setNotes((n) => {
              const next = { ...n };
              if (t) next[k] = t;
              else delete next[k];
              return next;
            });
          }}
        />
      )}
      {reviewing && mods && (
        <ReviewSpace
          mods={(mods ?? []).filter((m) => game === "all" || m.game === game)}
          onClose={() => setReviewing(false)}
          onDone={(t) => {
            toast(t);
            void load();
          }}
        />
      )}
      {importing && <ImportFor games={gameList} onPick={(g) => void importFile(g)} onClose={() => setImporting(false)} />}
      {profiling && (
        <AddToProfile
          game={profiling.mod.game}
          gameName={gameName(profiling.mod.game)}
          modId={profiling.mod.modId}
          fileId={profiling.file.fileId}
          modName={profiling.mod.name}
          onClose={() => setProfiling(null)}
          onDone={(t) => toast(t)}
        />
      )}
      {deleting && (
        <Confirm
          title={deleting.file ? "Delete this file?" : "Delete this mod?"}
          body={
            deleting.file
              ? `Delete “${deleting.file.name}” from “${deleting.mod.name}”? Its archive and unpacked folder are deleted from this computer.`
              : `Delete “${deleting.mod.name}” and its ${plural(deleting.mod.files.length, "file")}? The mod's folder is deleted from this computer. You can download it again from Nexus Mods.`
          }
          action="Delete"
          onConfirm={() =>
            deleting.file
              ? act(api.removeFile(deleting.mod.game, deleting.mod.modId, deleting.file.fileId), `Deleted “${deleting.file.name}”`)
              : act(api.removeMod(deleting.mod.game, deleting.mod.modId), `Deleted “${deleting.mod.name}”`)
          }
          onClose={() => setDeleting(null)}
        />
      )}
    </div>
  );
}
