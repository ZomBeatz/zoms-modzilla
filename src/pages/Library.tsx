import { useEffect, useRef, useState } from "react";
import { Archive, ArrowUpCircle, ExternalLink, FileUp, FolderOpen, HardDrive, Layers, Package, PackageOpen, Plus, RefreshCw, Search, StickyNote, Tag, Trash2 } from "lucide-react";
import { api, isDesktop, modUrl, openUrl, pickFile, type Game, type Profile, type StoredFile, type StoredMod, type UpdateReport } from "../api";
import { Cover, GameTag, Hero } from "../art";
import { AddToProfile } from "./AddToProfile";
import { ReviewSpace, usedKeys } from "./ReviewSpace";
import { NoteDialog } from "./NoteDialog";
import { FomodDialog } from "./FomodDialog";
import { TagChips, TagDialog, tagsIn, useTags } from "../tags";
import type { Ctx } from "../App";
import { bytes, plural, when } from "../format";
import { factKey, isAdult, isGone, useFacts } from "../facts";
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

/** "Owner-Name" for a mod from Thunderstore. */
const tsOf = (m: StoredMod) => (m.source?.startsWith("thunderstore:") ? m.source.slice(13) : null);

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
  const [installing, setInstalling] = useState<{ mod: StoredMod; fileId: number } | null>(null);
  const [tagging, setTagging] = useState<StoredMod | null>(null);
  const tags = useTags();
  const [updates, setUpdates] = useState<UpdateReport | null>(null);
  const [checking, setChecking] = useState(false);
  const [filter, setFilter] = useState("");
  const [show, setShow] = useState("all");
  const [sort, setSort] = useState<"name" | "newest" | "biggest">("name");
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

  // Ctrl+K picked a mod: show it.
  useEffect(() => {
    const f = (e: Event) => {
      setShow("all");
      setFilter(String((e as CustomEvent).detail ?? ""));
    };
    window.addEventListener("zoms:library-filter", f);
    return () => window.removeEventListener("zoms:library-filter", f);
  }, []);
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
  const inGame = (mods ?? []).filter((m) => game === "all" || m.game === game);
  const facts = useFacts(inGame.filter((m) => !m.local).map((m) => factKey(m.game, m.modId)).sort());
  const adultMod = (m: StoredMod) => isAdult(facts, m.game, m.modId, m.adult);
  const blur = (overview.settings.adultContent ?? "blur") !== "show";
  const categories = [...new Set(inGame.map((m) => facts[factKey(m.game, m.modId)]?.category).filter((c): c is string => !!c))].sort();
  const fits = (m: StoredMod) => {
    const f = facts[factKey(m.game, m.modId)];
    switch (show) {
      case "all": return true;
      case "unused": return !used.mods.has(`${m.game}:${m.modId}`);
      case "updates": return m.files.some((x) => updateFor(m, x)?.newer);
      case "bad": return m.files.some((x) => x.verified === false);
      case "packed": return m.files.some((x) => !x.unpacked && x.archive);
      case "needs": return !!f?.requires.length || !!f?.dlc.length;
      case "gone": return isGone(f);
      case "adult": return adultMod(m);
      case "notAdult": return !adultMod(m);
      case "local": return !!m.local && !tsOf(m);
      case "thunderstore": return !!tsOf(m);
      case "untagged": return !tags[`${m.game}:${m.modId}`]?.length;
      default: if (show.startsWith("tag:")) return !!tags[`${m.game}:${m.modId}`]?.includes(show.slice(4));
        return show.startsWith("cat:") && f?.category === show.slice(4);
    }
  };
  const size = (m: StoredMod) => m.files.reduce((k, f) => k + f.size, 0);
  const newest = (m: StoredMod) => Math.max(0, ...m.files.map((f) => f.downloadedAt));
  const shown = inGame
    .filter((m) => fits(m) && (!filter || `${m.name} ${m.author ?? ""}`.toLowerCase().includes(filter.toLowerCase())))
    .sort(sort === "newest" ? (a, b) => newest(b) - newest(a) : sort === "biggest" ? (a, b) => size(b) - size(a) : (a, b) => a.name.localeCompare(b.name));
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
          <label className="row gap-sm">
            <span className="muted small">Show</span>
            <select value={show} onChange={(e) => setShow(e.target.value)} aria-label="Show">
              <option value="all">All mods</option>
              <option value="unused">Not in any profile</option>
              {updates && <option value="updates">Updates waiting</option>}
              <option value="needs">Needs other mods</option>
              <option value="packed">Not unpacked</option>
              {inGame.some((m) => m.files.some((f) => f.verified === false)) && <option value="bad">Doesn't match Nexus Mods</option>}
              {inGame.some((m) => isGone(facts[factKey(m.game, m.modId)])) && <option value="gone">Gone from Nexus Mods</option>}
              {inGame.some(adultMod) && (
                <>
                  <option value="adult">Adult</option>
                  <option value="notAdult">Not adult</option>
                </>
              )}
              {inGame.some((m) => m.local && !tsOf(m)) && <option value="local">Your own files</option>}
              {inGame.some((m) => tsOf(m)) && <option value="thunderstore">From Thunderstore</option>}
              {tagsIn(tags, inGame.map((m) => `${m.game}:${m.modId}`)).length > 0 && (
                <optgroup label="Your tags">
                  {tagsIn(tags, inGame.map((m) => `${m.game}:${m.modId}`)).map((t) => (
                    <option key={t} value={`tag:${t}`}>
                      {t}
                    </option>
                  ))}
                  <option value="untagged">No tags</option>
                </optgroup>
              )}
              {categories.length > 0 && (
                <optgroup label="Category">
                  {categories.map((c) => (
                    <option key={c} value={`cat:${c}`}>
                      {c}
                    </option>
                  ))}
                </optgroup>
              )}
            </select>
          </label>
          <label className="row gap-sm">
            <span className="muted small">Sort</span>
            <select value={sort} onChange={(e) => setSort(e.target.value as typeof sort)} aria-label="Sort">
              <option value="name">By name</option>
              <option value="newest">Newest first</option>
              <option value="biggest">Biggest first</option>
            </select>
          </label>
          {(show !== "all" || filter) && (
            <span className="muted small num">
              {shown.length} of {inGame.length}
            </span>
          )}
          <span className="grow" />
          <label className="search">
            <Search size={14} />
            <input ref={search} placeholder="Filter mods  /" value={filter} onChange={(e) => setFilter(e.target.value)} aria-label="Filter mods" />
          </label>
        </div>
      )}
      {mods === null ? null : inGame.length === 0 ? (
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
          <p>{filter ? `No mods match “${filter}”` : "No mods are like that"}{show !== "all" && filter ? " with this filter" : ""}.</p>
          <button
            className="btn"
            onClick={() => {
              setFilter("");
              setShow("all");
            }}
          >
            Show all mods
          </button>
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
                    { label: "Tags…", icon: <Tag size={14} />, run: () => setTagging(m) },
                    { label: "Open folder", icon: <FolderOpen size={14} />, run: () => void act(api.openFolder({ game: m.game, modId: m.modId })) },
                    !m.local && { label: "Open on Nexus Mods", icon: <ExternalLink size={14} />, run: () => void act(openUrl(modUrl(m.game, m.modId))) },
                    "sep",
                    { label: "Delete mod…", icon: <Trash2 size={14} />, danger: true, run: () => setDeleting({ mod: m }) },
                  ]),
                );
              }}
            >
              <div className="thumb">{m.pictureUrl ? <img className={`thumb${blur && adultMod(m) ? " blurred" : ""}`} src={m.pictureUrl} alt="" loading="lazy" /> : <Cover domain={m.game} name={m.game} />}</div>
              <div className="grow">
                <div className="row gap">
                  <h3 className="grow">
                    {m.local ? (
                      m.name
                    ) : (
                      <button className="link-name" title="Open the mod's page on Nexus Mods" onClick={() => void act(openUrl(modUrl(m.game, m.modId)))}>
                        {m.name}
                        <ExternalLink size={12} />
                      </button>
                    )}
                  </h3>
                  {tsOf(m) ? (
                    <button className="pill accent" title="From Thunderstore: open its page" onClick={() => act(openUrl(`https://thunderstore.io/package/${tsOf(m)!.replace("-", "/")}/`))}>
                      Thunderstore
                    </button>
                  ) : m.local ? (
                    <span className="pill" title="Added from a file on this computer">Your file</span>
                  ) : (
                    <button className="icon-btn" title="Open on Nexus Mods" aria-label="Open on Nexus Mods" onClick={() => act(openUrl(modUrl(m.game, m.modId)))}>
                      <ExternalLink size={15} />
                    </button>
                  )}
                  <button className="icon-btn" title="Tags" aria-label="Tags" onClick={() => setTagging(m)}>
                    <Tag size={15} />
                  </button>
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
                  {adultMod(m) && <span className="pill warn">Adult</span>}
                  {isGone(facts[factKey(m.game, m.modId)]) && (
                    <span className="pill warn" title="Nexus Mods doesn't show this mod any more (hidden, removed or being checked). Your copy still works; keep it if you rely on it.">
                      Gone from Nexus Mods
                    </span>
                  )}
                  <GameTag domain={m.game} />
                  <TagChips tags={tags[`${m.game}:${m.modId}`]} />
                  {facts[factKey(m.game, m.modId)]?.category && <span>{facts[factKey(m.game, m.modId)]?.category}</span>}
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
                      {(f.hasInstaller || f.installed) && (
                        <button className={`btn sm${f.installed ? "" : " primary"}`} title={f.installed ? "Installed with your installer choices. Change them." : "This file has an installer: pick its options"} onClick={() => setInstalling({ mod: m, fileId: f.fileId })}>
                          {f.installed ? "Options…" : "Choose options"}
                        </button>
                      )}
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
      {tagging && <TagDialog game={tagging.game} modId={tagging.modId} name={tagging.name} known={tagsIn(tags, Object.keys(tags))} onClose={() => setTagging(null)} toast={toast} />}
      {installing && (
        <FomodDialog
          mod={installing.mod}
          fileId={installing.fileId}
          onClose={() => setInstalling(null)}
          onDone={(m) => {
            setMods((ms) => ms && ms.map((x) => (x.game === m.game && x.modId === m.modId ? { ...m, files: m.files.map((f) => ({ ...f, hasInstaller: x.files.find((y) => y.fileId === f.fileId)?.hasInstaller ?? f.hasInstaller })) } : x)));
            toast(`Saved the options for “${m.name}”. Put the mods in the game again to use them.`);
          }}
        />
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
