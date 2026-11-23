// Terraria's tModLoader on the Games page: switch its Workshop mods on and off, and use or save
// mod packs. Same files as tModLoader's own Manage Mods menu, so both always agree. Getting new
// mods stays on Steam (only Steam can download from the Workshop).

import { useEffect, useMemo, useState } from "react";
import { ExternalLink, Pickaxe, Search } from "lucide-react";
import { api, type TmlState } from "../api";
import { bytes, plural } from "../format";

export function TmlPanel({ toast }: { toast: (t: string) => void }) {
  const [s, setS] = useState<TmlState | null>(null);
  const [on, setOn] = useState<Set<string>>(new Set());
  const [q, setQ] = useState("");
  const [naming, setNaming] = useState<string | null>(null);
  const load = () =>
    api.tmlState().then(
      (x) => {
        setS(x);
        setOn(new Set(x?.mods.filter((m) => m.enabled).map((m) => m.name)));
      },
      () => setS(null),
    );
  useEffect(() => {
    void load();
  }, []);
  const was = useMemo(() => new Set(s?.mods.filter((m) => m.enabled).map((m) => m.name)), [s]);
  if (!s) return null;
  const changed = on.size !== was.size || [...on].some((n) => !was.has(n));
  const shown = s.mods.filter((m) => `${m.label} ${m.name}`.toLowerCase().includes(q.toLowerCase()));
  const use = async (names: string[], what: string) => {
    try {
      await api.tmlSet(names);
      toast(what);
      await load();
    } catch (e) {
      toast((e as Error).message);
    }
  };
  return (
    <div className="panel tml">
      <div className="row gap">
        <Pickaxe size={18} color="var(--zb-accent)" />
        <h3 className="grow" style={{ margin: 0 }}>
          tModLoader (Terraria)
        </h3>
        <button className="btn sm" onClick={() => void api.openSteam("tmodloader-workshop").catch((e) => toast((e as Error).message))} title="Subscribe to mods on Steam; they show up here">
          <ExternalLink size={13} /> Workshop on Steam
        </button>
      </div>
      <p className="muted small">
        {plural(s.mods.length, "mod")}, {on.size} switched on. Get mods by subscribing on the Workshop; switch them on and off here, and keep sets as mod packs (tModLoader's own menu shows them too). Close tModLoader before changing them.
      </p>
      {s.missing.length > 0 && <p className="warn-text small">Switched on but not installed any more: {s.missing.join(", ")}. Subscribe again on the Workshop, or they're left out.</p>}
      {s.packs.length > 0 && (
        <div className="row gap-sm wrap" style={{ marginBottom: 8 }}>
          <span className="small muted">Mod packs:</span>
          {s.packs.map((p) => (
            <button key={p.name} className="btn sm" title={p.mods.join(", ")} onClick={() => void use(p.mods, `Using the mod pack “${p.name}” (${plural(p.mods.length, "mod")})`)}>
              {p.name}
            </button>
          ))}
        </div>
      )}
      <label className="pal-search small-search">
        <Search size={14} />
        <input placeholder="Find a mod" value={q} onChange={(e) => setQ(e.target.value)} />
      </label>
      <div className="tml-list">
        {shown.map((m) => (
          <label key={m.name} className="tml-mod">
            <input
              type="checkbox"
              checked={on.has(m.name)}
              onChange={(e) =>
                setOn((x) => {
                  const n = new Set(x);
                  if (e.target.checked) n.add(m.name);
                  else n.delete(m.name);
                  return n;
                })
              }
            />
            <span className="grow ellipsis" title={m.name}>
              {m.label}
            </span>
            {!m.workshopId && <span className="pill tiny-pill">your file</span>}
            <span className="muted small num">{bytes(m.size)}</span>
          </label>
        ))}
      </div>
      <div className="row gap-sm wrap" style={{ marginTop: 8 }}>
        <button className="btn sm primary" disabled={!changed} onClick={() => void use([...on], `tModLoader will start with ${plural(on.size, "mod")}`)}>
          Use these
        </button>
        <button className="btn sm" disabled={!changed} onClick={() => setOn(new Set(was))}>
          Undo
        </button>
        <span className="grow" />
        {naming === null ? (
          <button className="btn sm ghost" onClick={() => setNaming("")}>
            Save as a mod pack…
          </button>
        ) : (
          <>
            <input autoFocus placeholder="Pack name" value={naming} onChange={(e) => setNaming(e.target.value)} />
            <button
              className="btn sm"
              disabled={!naming.trim()}
              onClick={async () => {
                try {
                  await api.tmlSavePack(naming, [...on]);
                  toast(`Saved the mod pack “${naming.trim()}”`);
                  setNaming(null);
                  await load();
                } catch (e) {
                  toast((e as Error).message);
                }
              }}
            >
              Save
            </button>
            <button className="btn sm ghost" onClick={() => setNaming(null)}>
              Cancel
            </button>
          </>
        )}
      </div>
    </div>
  );
}
