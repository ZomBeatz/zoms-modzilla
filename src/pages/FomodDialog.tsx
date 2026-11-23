// A mod's installer (FOMOD), one step at a time: pick the options, see each one's picture and
// description, then install. The picks are kept, so a newer version can be installed the same way.

import { useEffect, useMemo, useState } from "react";
import { ImageOff, RotateCcw } from "lucide-react";
import { api, type Installer, type StoredMod } from "../api";
import { forced, fromChoices, kindOf, pickedIn, problem, startPicks, toChoices, usable, walk, type Picks } from "../fomod";
import { Dialog } from "../ui";

const KIND_WORDS: Record<string, string> = { required: "Required", recommended: "Recommended", notusable: "Can't be used", couldbeusable: "Might not work" };

function Picture({ game, modId, fileId, path }: { game: string; modId: number; fileId: number; path?: string | null }) {
  const [src, setSrc] = useState<string | null | undefined>(undefined);
  useEffect(() => {
    setSrc(undefined);
    if (!path) return setSrc(null);
    let live = true;
    api.installerImage(game, modId, fileId, path).then((s) => live && setSrc(s), () => live && setSrc(null));
    return () => {
      live = false;
    };
  }, [game, modId, fileId, path]);
  if (src === undefined) return <div className="fomod-pic" />;
  if (!src)
    return (
      <div className="fomod-pic empty">
        <ImageOff size={22} />
      </div>
    );
  return <img className="fomod-pic" src={src} alt="" />;
}

export function FomodDialog({ mod, fileId, onClose, onDone }: { mod: StoredMod; fileId: number; onClose: () => void; onDone: (m: StoredMod) => void }) {
  const [inst, setInst] = useState<Installer | null>(null);
  const [error, setError] = useState("");
  const [picks, setPicks] = useState<Picks>({});
  const [at, setAt] = useState(0);
  const [focus, setFocus] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.installer(mod.game, mod.modId, fileId).then(
      (i) => {
        setInst(i);
        setPicks(fromChoices(i.current ?? i.previous) ?? {});
      },
      (e) => setError((e as Error).message),
    );
  }, [mod.game, mod.modId, fileId]);

  // Fill in starting picks for groups not answered yet, as steps come into view.
  const steps = useMemo(() => (inst ? walk(inst, picks) : []), [inst, picks]);
  useEffect(() => {
    if (!inst) return;
    let changed = false;
    const next: Picks = structuredClone(picks);
    for (const { step, flags } of steps)
      for (const g of step.groups)
        if (next[step.name]?.[g.name] === undefined) {
          (next[step.name] ??= {})[g.name] = startPicks(g, flags);
          changed = true;
        }
    if (changed) setPicks(next);
  }, [steps]); // eslint-disable-line react-hooks/exhaustive-deps

  const here = steps[Math.min(at, Math.max(0, steps.length - 1))];
  const last = at >= steps.length - 1;
  const stepProblem = here?.step.groups.map((g) => problem(g, picks[here.step.name]?.[g.name] ?? [], here.flags)).find(Boolean) ?? null;
  const shown = here?.step.groups.flatMap((g) => g.plugins).find((p) => p.name === focus) ?? here?.step.groups.flatMap((g) => pickedIn(g, picks[here.step.name]?.[g.name] ?? [], here.flags))[0] ?? here?.step.groups[0]?.plugins[0];

  const set = (group: string, names: string[]) => setPicks((p) => ({ ...p, [here.step.name]: { ...p[here.step.name], [group]: names } }));
  const install = async () => {
    if (!inst) return;
    setBusy(true);
    try {
      onDone(await api.install(mod.game, mod.modId, fileId, toChoices(inst, picks)));
      onClose();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const title = inst?.name || mod.name;
  if (error && !inst)
    return (
      <Dialog title={title} onClose={onClose} foot={<button className="btn" onClick={onClose}>Close</button>}>
        <p className="err-text">{error}</p>
      </Dialog>
    );
  if (!inst) return <Dialog title={title} onClose={onClose}>{<p className="muted">Reading the installer…</p>}</Dialog>;

  const noQuestions = steps.length === 0;
  return (
    <Dialog
      wide
      title={`Install ${title}`}
      onClose={onClose}
      foot={
        <>
          {inst.current && (
            <button
              className="btn ghost left"
              title="Forget these choices and use the mod's files as they are"
              onClick={async () => {
                await api.resetInstall(mod.game, mod.modId, fileId).catch((e) => setError((e as Error).message));
                onDone({ ...mod, files: mod.files.map((f) => (f.fileId === fileId ? { ...f, installed: null } : f)) });
                onClose();
              }}
            >
              <RotateCcw size={14} /> Undo the install
            </button>
          )}
          {error && <span className="err-text small">{error}</span>}
          {at > 0 && (
            <button className="btn" onClick={() => setAt((a) => a - 1)}>
              Back
            </button>
          )}
          {!last && !noQuestions ? (
            <button className="btn primary" disabled={!!stepProblem} title={stepProblem ?? undefined} onClick={() => setAt((a) => a + 1)}>
              Next
            </button>
          ) : (
            <button className="btn primary" disabled={!!stepProblem || busy} onClick={install}>
              {busy ? "Installing…" : inst.current ? "Install again" : "Install"}
            </button>
          )}
        </>
      }
    >
      {noQuestions ? (
        <p>This installer has no questions. Install puts in the files it always installs.</p>
      ) : (
        <div className="fomod">
          <div className="fomod-steps muted small">
            Step {Math.min(at, steps.length - 1) + 1} of {steps.length}: <b>{here.step.name || "Options"}</b>
            {inst.previous && !inst.current && <span> · {inst.fromCollection ? "starting with the collection curator's choices" : "starting with the choices you made last time"}</span>}
          </div>
          <div className="fomod-body">
            <div className="fomod-groups">
              {here.step.groups.map((g) => {
                const chosen = picks[here.step.name]?.[g.name] ?? [];
                const one = /exactlyone|atmostone/i.test(g.kind);
                const why = problem(g, chosen, here.flags);
                return (
                  <fieldset key={g.name} className="fomod-group">
                    <legend>
                      {g.name} {why && <span className="warn-text small">{why}</span>}
                    </legend>
                    {g.plugins.map((p) => {
                      const k = kindOf(p, here.flags);
                      const lock = forced(g, p, here.flags);
                      const can = usable(p, here.flags);
                      const on = can && (lock || chosen.includes(p.name));
                      return (
                        <label key={p.name} className={`fomod-opt${focus === p.name ? " focus" : ""}${can ? "" : " off"}`} onMouseEnter={() => setFocus(p.name)} onFocus={() => setFocus(p.name)}>
                          <input
                            type={one ? "radio" : "checkbox"}
                            name={`${here.step.name}/${g.name}`}
                            checked={on}
                            disabled={!can || lock}
                            onChange={(e) => set(g.name, one ? [p.name] : e.target.checked ? [...chosen, p.name] : chosen.filter((n) => n !== p.name))}
                          />
                          <span className="grow">{p.name}</span>
                          {KIND_WORDS[k.toLowerCase()] && <span className={`pill tiny-pill${/notusable|couldbe/i.test(k) ? " warn" : ""}`}>{KIND_WORDS[k.toLowerCase()]}</span>}
                        </label>
                      );
                    })}
                    {/atmostone/i.test(g.kind) && chosen.length > 0 && (
                      <button className="btn ghost sm" onClick={() => set(g.name, [])}>
                        None of these
                      </button>
                    )}
                  </fieldset>
                );
              })}
            </div>
            {shown && (
              <div className="fomod-about">
                <Picture game={mod.game} modId={mod.modId} fileId={fileId} path={shown.image ?? inst.image} />
                <b>{shown.name}</b>
                <p className="small fomod-desc">{shown.description || "No description."}</p>
              </div>
            )}
          </div>
        </div>
      )}
    </Dialog>
  );
}
