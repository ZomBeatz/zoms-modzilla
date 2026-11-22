import { useEffect, useState } from "react";
import { ExternalLink, Info, Search } from "lucide-react";
import { api, fileUrl, openUrl, type Collection, type FileInfo, type ModInfo, type StoredMod } from "../api";
import type { Ctx } from "../App";
import { bytes, plural, when } from "../format";
import { Dialog } from "../ui";

const CATEGORY: Record<string, string> = {
  MAIN: "Main file",
  UPDATE: "Update",
  OPTIONAL: "Optional",
  MISCELLANEOUS: "Other",
  OLD_VERSION: "Old version",
  ARCHIVED: "Archived",
};

/** Add files from a mod page address, a whole collection, or an nxm:// link. */
export function AddDialog({ overview, toast, go, onClose, initial }: Ctx & { onClose: () => void; initial?: string }) {
  const [address, setAddress] = useState(initial ?? "");
  const [collection, setCollection] = useState<Collection | null>(null);
  const [optional, setOptional] = useState(false);
  const [found, setFound] = useState<{ mod: ModInfo; files: FileInfo[] } | null>(null);
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const premium = overview.user?.isPremium ?? false;
  // What's already downloaded, so it isn't downloaded again.
  const [library, setLibrary] = useState<StoredMod[]>([]);
  useEffect(() => {
    api.library().then(setLibrary, () => {});
  }, []);
  const have = (game: string, modId: number, fileId: number) => library.some((m) => m.game === game && m.modId === modId && m.files.some((f) => f.fileId === fileId));
  const isLink = address.trim().startsWith("nxm://");

  const look = async () => {
    setError("");
    setBusy(true);
    try {
      if (address.includes("/collections/")) {
        setCollection(await api.lookupCollection(address));
        return;
      }
      if (isLink) {
        const job = await api.addLink(address);
        toast(`Added “${job.name}” to downloads`);
        onClose();
        go("downloads");
        return;
      }
      const r = await api.lookupMod(address);
      setFound(r);
      const main = r.files.filter((f) => f.category === "MAIN");
      // The main file picked to start with, unless it's already downloaded.
      setPicked(new Set((main.length === 1 ? main : r.files.slice(0, 1)).filter((f) => !have(r.mod.game, r.mod.modId, f.fileId)).map((f) => f.fileId)));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const add = async () => {
    setBusy(true);
    try {
      const jobs = await api.addFiles(address, [...picked]);
      toast(jobs.length === 1 ? `Added “${jobs[0].name}” to downloads` : `Added ${plural(jobs.length, "file")} to downloads`);
      onClose();
      go("downloads");
    } catch (e) {
      setError((e as Error).message);
      setBusy(false);
    }
  };

  const addCollection = async () => {
    setBusy(true);
    try {
      const r = await api.addCollection(address, optional);
      toast(`Added ${plural(r.queued, "file")} to downloads${r.skipped ? ` (${r.skipped} you already have were skipped)` : ""}, and the profile “${r.profile.name}”`);
      onClose();
      go("downloads");
    } catch (e) {
      setError((e as Error).message);
      setBusy(false);
    }
  };

  // Opened from Browse with a mod's address: look it up straight away.
  useEffect(() => {
    if (initial && overview.hasKey) void look();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  if (!overview.hasKey) {
    return (
      <Dialog
        title="Add mod"
        onClose={onClose}
        foot={
          <button
            className="btn primary"
            onClick={() => {
              onClose();
              go("settings");
            }}
          >
            Go to Settings
          </button>
        }
      >
        <p style={{ margin: 0 }}>Zom's Nexus needs your Nexus Mods API key before it can download anything. Add it in Settings.</p>
      </Dialog>
    );
  }

  if (collection) {
    const wanted = collection.files.filter((f) => optional || !f.optional);
    const files = wanted.filter((f) => !have(f.game, f.modId, f.fileId));
    const already = wanted.length - files.length;
    const opt = collection.files.filter((f) => f.optional).length;
    return (
      <Dialog
        wide
        title={collection.name}
        onClose={onClose}
        foot={
          <>
            <button className="btn left" onClick={() => setCollection(null)}>
              Back
            </button>
            <button className="btn" onClick={onClose}>
              Cancel
            </button>
            <button className="btn primary" disabled={busy || (files.length === 0 && already === 0)} onClick={addCollection}>
              {files.length ? `Download ${plural(files.length, "file")} (${bytes(files.reduce((n, f) => n + f.size, 0))})` : "Make the profile"}
            </button>
          </>
        }
      >
        <div className="dialog-meta" style={{ marginTop: -6, marginBottom: 12 }}>
          <span className="pill accent">Collection</span>
          {collection.author && <span>by {collection.author}</span>}
          {collection.revision != null && <span>· revision {collection.revision}</span>}
          <span>· {collection.game}</span>
        </div>
        {collection.summary && <p className="muted" style={{ marginTop: 0 }}>{collection.summary}</p>}
        <div className="notice">
          <Info size={16} />
          <div>
            Every file goes into Downloads, and a profile named after the collection is made with them, ready to check and play.
            {!premium && " Your account is free, so each file waits for its Mod manager download click on Nexus Mods."} Some collections also need files from other sites or manual steps; see its page on Nexus Mods.
          </div>
        </div>
        {already > 0 && (
          <p className="muted small" style={{ margin: "0 0 10px" }}>
            {already === wanted.length ? "You already have every file, so nothing needs downloading." : `${plural(already, "file")} you already have won't be downloaded again.`}
          </p>
        )}
        {opt > 0 && (
          <label className="check" style={{ marginBottom: 10 }}>
            <input type="checkbox" checked={optional} onChange={(e) => setOptional(e.target.checked)} /> Include the {plural(opt, "optional mod")}
          </label>
        )}
        {error && <p className="err-text">{error}</p>}
        <div className="pick-list">
          {collection.files.map((f) => (
            <div key={`${f.modId}:${f.fileId}`} className="pick-item" style={{ cursor: "default", opacity: f.optional && !optional ? 0.5 : 1 }}>
              <div className="grow">
                <div className="row gap-sm wrap">
                  <b>{f.modName}</b>
                  {f.optional && <span className="pill">Optional</span>}
                  {have(f.game, f.modId, f.fileId) && <span className="pill ok">In your library</span>}
                </div>
                <div className="meta num">{[f.name, f.version && `version ${f.version}`, f.size ? bytes(f.size) : ""].filter(Boolean).join(" · ")}</div>
              </div>
            </div>
          ))}
        </div>
      </Dialog>
    );
  }

  const toggle = (id: number) => {
    const n = new Set(picked);
    if (n.has(id)) n.delete(id);
    else n.add(id);
    setPicked(n);
  };

  const shown = found?.files.filter((f) => f.category !== "ARCHIVED" && f.category !== "DELETED") ?? [];
  const size = shown.filter((f) => picked.has(f.fileId)).reduce((n, f) => n + f.size, 0);

  return (
    <Dialog
      wide
      title={found ? found.mod.name : "Add mod or collection"}
      onClose={onClose}
      foot={
        found ? (
          <>
            <button className="btn left" onClick={() => setFound(null)}>
              Back
            </button>
            <button className="btn" onClick={onClose}>
              Cancel
            </button>
            <button className="btn primary" disabled={busy || picked.size === 0} onClick={add}>
              Download {picked.size > 1 ? plural(picked.size, "file") : ""} {size ? `(${bytes(size)})` : ""}
            </button>
          </>
        ) : (
          <>
            <button className="btn" onClick={onClose}>
              Cancel
            </button>
            <button className="btn primary" disabled={busy || !address.trim()} onClick={look}>
              {isLink && !address.includes("/collections/") ? "Add to downloads" : "Find files"}
            </button>
          </>
        )
      }
    >
      {!found ? (
        <>
          <label className="field">
            <span>Mod or collection address, or download link</span>
            <input
              autoFocus
              placeholder="https://www.nexusmods.com/skyrimspecialedition/mods/266"
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && address.trim() && void look()}
            />
            <span className="hint">Copy the address of a mod's or a collection's page on nexusmods.com, or an nxm:// link.</span>
            {error && <span className="err">{error}</span>}
          </label>
          {!premium && (
            <div className="notice" style={{ marginTop: 16, marginBottom: 0 }}>
              <Info size={16} />
              <div>
                <b>With a free Nexus Mods account,</b> start downloads from the website: open the file's page, click <b>Mod manager download</b>, and Zom's Nexus adds it here by itself. Nexus Mods only lets Premium
                members download straight from an app. Set this up under <b>Settings &gt; Nexus Mods links</b>.
              </div>
            </div>
          )}
        </>
      ) : (
        <>
          <div className="dialog-meta" style={{ marginTop: -6, marginBottom: 12 }}>
            {found.mod.author && <span>by {found.mod.author}</span>}
            {found.mod.version && <span>· version {found.mod.version}</span>}
            <span>· {found.mod.game}</span>
          </div>
          {found.mod.summary && <p className="muted" style={{ marginTop: 0 }}>{found.mod.summary}</p>}
          {!premium && (
            <div className="notice warn">
              <Info size={16} />
              <div>Your account is free, so Nexus Mods asks you to start each download on its website. Files you pick wait in Downloads until you click <b>Mod manager download</b> for them there (<b>Open on Nexus Mods</b> takes you to the file).</div>
            </div>
          )}
          {error && <p className="err" style={{ color: "var(--zb-danger)" }}>{error}</p>}
          {shown.length === 0 ? (
            <div className="empty">
              <Search size={28} />
              <p>This mod has no files to download.</p>
            </div>
          ) : (
            <div className="pick-list">
              {shown.map((f) => (
                <label key={f.fileId} className="pick-item">
                  <input type="checkbox" checked={picked.has(f.fileId)} onChange={() => toggle(f.fileId)} />
                  <div className="grow">
                    <div className="row gap-sm wrap">
                      <b>{f.name}</b>
                      {f.category && <span className={`pill${f.category === "MAIN" ? " accent" : ""}`}>{CATEGORY[f.category] ?? f.category}</span>}
                      {have(found.mod.game, found.mod.modId, f.fileId) && (
                        <span className="pill ok" title="Picking it downloads it again, replacing your copy">
                          In your library
                        </span>
                      )}
                    </div>
                    <div className="meta num">
                      {[f.version && `version ${f.version}`, bytes(f.size), f.uploaded && when(f.uploaded)].filter(Boolean).join(" · ")}
                    </div>
                  </div>
                  <button
                    className="btn sm ghost"
                    onClick={(e) => {
                      e.preventDefault();
                      void openUrl(fileUrl(found.mod.game, found.mod.modId, f.fileId));
                    }}
                  >
                    <ExternalLink size={14} /> Open on Nexus Mods
                  </button>
                </label>
              ))}
            </div>
          )}
        </>
      )}
    </Dialog>
  );
}
