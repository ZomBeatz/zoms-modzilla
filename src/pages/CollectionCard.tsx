// The collection a profile was made from, kept apart from what you change on top: add-ons you
// can switch on and off or share, your own changes, and moving to a newer revision.

import { useEffect, useState } from "react";
import { ChevronDown, ChevronRight, ExternalLink, FileDown, Package, RefreshCw, Trash2 } from "lucide-react";
import { api, isDesktop, openUrl, pickSave, type CollectionUpdate, type Layer, type Layers, type Profile } from "../api";
import { plural } from "../format";
import { Confirm } from "../ui";

export const collectionUrl = (game: string, slug: string, revision?: number | null) => `https://www.nexusmods.com/games/${game}/collections/${slug}${revision ? `/revisions/${revision}` : ""}`;

/** "adds 3, switches off 2, another file for 1" */
function summary(l: Layer) {
  const bits = [l.added.length ? `adds ${plural(l.added.length, "mod")}` : "", l.off.length ? `switches off ${l.off.length}` : "", l.files.length ? `another file for ${l.files.length}` : ""].filter(Boolean);
  return bits.length ? bits.join(", ") : "no changes";
}

function LayerMods({ l, names }: { l: Layer; names: Record<string, string> }) {
  const name = (id: number) => names[id] ?? `Mod ${id}`;
  const part = (title: string, ids: number[]) =>
    ids.length > 0 && (
      <div className="layer-part">
        <span className="muted small">{title}</span> {ids.map(name).join(", ")}
      </div>
    );
  return (
    <div className="layer-mods small">
      {part("Adds:", l.added)}
      {part("Switches off:", l.off)}
      {part("Another file for:", l.files)}
    </div>
  );
}

export function CollectionCard({ profile, onChanged, toast }: { profile: Profile; onChanged: (p: Profile) => void; toast: (t: string) => void }) {
  const [layers, setLayers] = useState<Layers | null>(null);
  const [busy, setBusy] = useState("");
  const [update, setUpdate] = useState<CollectionUpdate | null>(null);
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const [naming, setNaming] = useState("");
  const [removing, setRemoving] = useState<Layer | null>(null);
  const slug = profile.base?.slug ?? profile.collection;
  // Worked out on this computer, again whenever the profile's mods change.
  const key = JSON.stringify([profile.mods, profile.addons, profile.base?.revision]);
  useEffect(() => {
    if (!slug) return;
    api.layers(profile.id).then(setLayers, () => setLayers(null));
  }, [profile.id, key]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => setUpdate(null), [profile.id]);
  if (!slug) return null;

  const run = async <T,>(label: string, f: () => Promise<T>) => {
    setBusy(label);
    try {
      return await f();
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setBusy("");
    }
  };
  const changed = (p?: Profile | null) => {
    if (p) onChanged(p);
  };
  const base = layers?.base;
  const revision = base?.revision ?? layers?.revision;
  const yours = layers?.yours;
  const ch = update?.changes;
  const newer = !!update && update.to != null && update.to !== update.from;
  const nm = (ids: number[]) => ids.map((id) => update?.names[id] ?? layers?.names[id] ?? `Mod ${id}`);

  return (
    <div className="panel collection-card">
      <div className="row gap wrap">
        <Package size={18} color="var(--zb-accent)" />
        <div className="grow">
          <b>{base?.name ?? "Made from a collection"}</b>
          <div className="muted small">
            {revision != null ? `Revision ${revision}` : "Revision not known"}
            {base && ` · ${plural(base.count, "mod")} from the collection`}
            {layers && layers.addons.length > 0 && ` · ${plural(layers.addons.filter((a) => a.enabled).length, "add-on")} on`}
          </div>
        </div>
        <button className="btn ghost sm" onClick={() => void openUrl(collectionUrl(profile.game, slug, revision))} title="Open this collection on Nexus Mods">
          <ExternalLink size={14} /> View on Nexus Mods
        </button>
        {base && (
          <button
            className="btn sm"
            disabled={!!busy}
            title="Asks Nexus Mods for the collection's newest revision (one request)"
            onClick={async () => {
              const r = await run("look", () => api.collectionUpdate(profile.id, false));
              if (r) {
                setUpdate(r);
                if (r.to === r.from) toast(`“${base.name}” is on its newest revision (${r.to})`);
              }
            }}
          >
            <RefreshCw size={14} className={busy === "look" ? "spin" : ""} /> {busy === "look" ? "Looking…" : "Newer revision?"}
          </button>
        )}
      </div>

      {layers && !base && (
        <div className="collection-note">
          <p className="small" style={{ margin: 0 }}>
            This profile was made before Zom's Nexus kept collections apart from your changes. Look up {revision != null ? `revision ${revision}` : "the collection"} once to sort out what's yours, so you can update the collection later and keep your changes, or share them as an add-on pack.
          </p>
          <button
            className="btn primary sm"
            disabled={!!busy}
            onClick={async () => {
              const l = await run("link", () => api.linkCollection(profile.id));
              if (l) {
                setLayers(l);
                toast(l.yours && summary(l.yours) !== "no changes" ? `Found your changes: ${summary(l.yours)}` : "No changes of yours on top of the collection");
              }
            }}
          >
            {busy === "link" ? "Looking it up…" : "Find my changes"}
          </button>
        </div>
      )}

      {update && newer && ch && (
        <div className="collection-note accent">
          <div className="grow small">
            <b>Revision {update.to} is out.</b>{" "}
            {([ch.added.length ? `adds ${nm(ch.added).join(", ")}` : "", ch.removed.length ? `takes out ${nm(ch.removed).join(", ")}` : "", ch.updated.length ? `updates ${plural(ch.updated.length, "mod")}` : ""].filter(Boolean).join("; ") || "no changes to the mods").replace(/^./, (c) => c.toUpperCase())}
            .{ch.kept.length > 0 && ` You keep your own file for ${nm(ch.kept).join(", ")}.`}
            {ch.joined.length > 0 && ` ${nm(ch.joined).join(", ")} ${ch.joined.length === 1 ? "is" : "are"} now part of the collection.`} Your add-ons and changes stay.
          </div>
          <button
            className="btn primary sm"
            disabled={!!busy}
            onClick={async () => {
              const r = await run("update", () => api.collectionUpdate(profile.id, true, update.to));
              if (r) {
                setUpdate(null);
                changed(r.profile);
                toast(`Moved to revision ${r.to}${r.queued ? `; ${plural(r.queued, "file")} added to downloads` : ""}. Put the mods in the game again to use it.`);
              }
            }}
          >
            {busy === "update" ? "Updating…" : `Update to revision ${update.to}`}
          </button>
          <button className="btn ghost sm" onClick={() => setUpdate(null)}>
            Not now
          </button>
        </div>
      )}

      {base && layers && (
        <div className="layers">
          {layers.addons.map((a) => (
            <div key={a.id} className={`layer${a.enabled ? "" : " off"}`}>
              <div className="row gap">
                <button className="icon-btn" aria-label={open[a.id] ? "Hide mods" : "Show mods"} onClick={() => setOpen((o) => ({ ...o, [a.id]: !o[a.id] }))}>
                  {open[a.id] ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                </button>
                <label className="check" title={a.enabled ? "Switch this add-on off: its changes come off the profile" : "Switch this add-on on"}>
                  <input type="checkbox" checked={a.enabled} disabled={!!busy} onChange={async (e) => changed(await run("set", () => api.setAddon(profile.id, a.id, { enabled: e.target.checked })))} />
                </label>
                <input
                  className="layer-name"
                  defaultValue={a.name}
                  aria-label="Add-on name"
                  onBlur={async (e) => {
                    const n = e.target.value.trim();
                    if (n && n !== a.name) changed(await run("set", () => api.setAddon(profile.id, a.id, { name: n })));
                  }}
                />
                <span className="muted small grow">{summary(a)}</span>
                {isDesktop && (
                  <button
                    className="btn ghost sm"
                    title="Save as an add-on pack: the collection it goes on and what it changes, by Nexus Mods numbers"
                    onClick={async () => {
                      const path = await pickSave("Save add-on pack", `${a.name}.json`);
                      if (path) {
                        const ok = await run("export", () => api.exportAddon(profile.id, a.id, path).then(() => true));
                        if (ok) toast("Saved the add-on pack. Whoever opens it gets the collection with your add-on on top.");
                      }
                    }}
                  >
                    <FileDown size={14} /> Share…
                  </button>
                )}
                <button className="icon-btn danger" title="Remove this add-on (its changes come off first)" aria-label="Remove add-on" onClick={() => setRemoving(a)}>
                  <Trash2 size={14} />
                </button>
              </div>
              {open[a.id] && <LayerMods l={a} names={layers.names} />}
            </div>
          ))}

          {yours && (
            <div className="layer yours">
              <div className="row gap wrap">
                <button className="icon-btn" aria-label={open.yours ? "Hide mods" : "Show mods"} onClick={() => setOpen((o) => ({ ...o, yours: !o.yours }))} disabled={summary(yours) === "no changes"}>
                  {open.yours ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                </button>
                <b className="small">Your changes</b>
                <span className="muted small grow">{summary(yours) === "no changes" ? "none yet: the profile is the collection as it is" : summary(yours)}</span>
                {summary(yours) !== "no changes" && (
                  <>
                    <input className="layer-name" placeholder="Add-on name" value={naming} onChange={(e) => setNaming(e.target.value)} aria-label="Name for the new add-on" />
                    <button
                      className="btn sm"
                      disabled={!naming.trim() || !!busy}
                      title="Make your changes into an add-on you can switch off, keep through collection updates, and share"
                      onClick={async () => {
                        const p = await run("save", () => api.saveAddon(profile.id, naming));
                        if (p) {
                          changed(p);
                          toast(`Made the add-on “${naming.trim()}”`);
                          setNaming("");
                        }
                      }}
                    >
                      Make an add-on
                    </button>
                  </>
                )}
              </div>
              {open.yours && <LayerMods l={yours} names={layers.names} />}
            </div>
          )}
        </div>
      )}

      {removing && (
        <Confirm
          title={`Remove “${removing.name}”?`}
          body="Its changes come off the profile first: the mods it adds go out (they stay in your library) and the collection's mods it switched off go back on."
          action="Remove"
          onConfirm={async () => changed(await run("set", () => api.setAddon(profile.id, removing.id, { remove: true })))}
          onClose={() => setRemoving(null)}
        />
      )}
    </div>
  );
}
