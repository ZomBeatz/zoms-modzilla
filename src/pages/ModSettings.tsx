// A profile's mod settings: every settings file its mods have, as a simple form where the file
// allows (switches, numbers, choices, with the file's own comments as help) or as text. Saved
// with the profile, so each profile can set the same mod differently; Reset goes back to the
// mod's own file.

import { useEffect, useMemo, useState } from "react";
import { RotateCcw, Search, SlidersHorizontal } from "lucide-react";
import { api, type Profile, type SettingFile } from "../api";
import { fieldsOf, setField, type Field } from "../settingsform";
import { Dialog } from "../ui";

export function ModSettings({ profile, onClose, toast }: { profile: Profile; onClose: () => void; toast: (t: string) => void }) {
  const [files, setFiles] = useState<SettingFile[] | null>(null);
  const [q, setQ] = useState("");
  const [at, setAt] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [saved, setSaved] = useState("");
  const [asText, setAsText] = useState(false);
  const [error, setError] = useState("");

  const load = () => api.modSettings(profile.id).then(setFiles, (e) => setError((e as Error).message));
  useEffect(() => {
    void load();
  }, [profile.id]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (!at && files?.length) setAt(files[0].path);
  }, [files, at]);
  useEffect(() => {
    if (!at) return;
    setError("");
    api.readSetting(profile.id, at).then(
      (t) => {
        setText(t);
        setSaved(t);
      },
      (e) => setError((e as Error).message),
    );
  }, [at, profile.id]);

  const shown = useMemo(() => (files ?? []).filter((f) => `${f.path} ${f.modName ?? ""}`.toLowerCase().includes(q.toLowerCase())), [files, q]);
  const groups = useMemo(() => {
    const g = new Map<string, SettingFile[]>();
    for (const f of shown) g.set(f.modName ?? "Made by mods while playing", [...(g.get(f.modName ?? "Made by mods while playing") ?? []), f]);
    return [...g];
  }, [shown]);
  const fields = at ? fieldsOf(at, text) : null;
  const dirty = text !== saved;
  const current = files?.find((f) => f.path === at);

  const save = async () => {
    if (!at) return;
    try {
      await api.writeSetting(profile.id, at, text);
      setSaved(text);
      toast(`Saved for “${profile.name}”. Other profiles keep their own.`);
      void load();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <Dialog
      wide
      title={`Mod settings · ${profile.name}`}
      onClose={() => {
        if (!dirty || confirm("Close without saving your changes?")) onClose();
      }}
      foot={
        <>
          {current?.own && current.modId && (
            <button
              className="btn ghost left"
              title="Forget this profile's version and use the mod's own file"
              onClick={async () => {
                await api.resetSetting(profile.id, at!).catch((e) => setError((e as Error).message));
                const t = await api.readSetting(profile.id, at!).catch(() => "");
                setText(t);
                setSaved(t);
                void load();
              }}
            >
              <RotateCcw size={14} /> Back to the mod's own
            </button>
          )}
          {error && <span className="err-text small">{error}</span>}
          <button className="btn" disabled={!dirty} onClick={() => setText(saved)}>
            Undo changes
          </button>
          <button className="btn primary" disabled={!dirty} onClick={save}>
            Save
          </button>
        </>
      }
    >
      {files === null ? (
        <p className="muted">Looking through the mods…</p>
      ) : files.length === 0 ? (
        <p className="muted">None of this profile's switched-on mods has a settings file (.ini, .cfg, .json …). Some make theirs the first time the game runs: play once, and they show up here.</p>
      ) : (
        <div className="msettings">
          <div className="msettings-list">
            <label className="pal-search small-search">
              <Search size={14} />
              <input placeholder="Find a mod or file" value={q} onChange={(e) => setQ(e.target.value)} />
            </label>
            {groups.map(([mod, list]) => (
              <div key={mod} className="msettings-group">
                <div className="muted small ellipsis">{mod}</div>
                {list.map((f) => (
                  <button
                    key={f.path}
                    className={`msettings-file${f.path === at ? " on" : ""}`}
                    title={f.path}
                    onClick={() => {
                      if (!dirty || confirm("Leave without saving your changes?")) setAt(f.path);
                    }}
                  >
                    <span className="ellipsis grow">{f.path.split("/").pop()}</span>
                    {f.own && <span className="pill tiny-pill accent" title="This profile has its own version">changed</span>}
                  </button>
                ))}
              </div>
            ))}
          </div>
          <div className="msettings-edit">
            {at && (
              <div className="msettings-head">
                <code className="ellipsis grow small">{at}</code>
                {fields && (
                  <button className="btn ghost sm" onClick={() => setAsText((x) => !x)}>
                    <SlidersHorizontal size={13} /> {asText ? "Simple view" : "As text"}
                  </button>
                )}
              </div>
            )}
            {fields && !asText ? <FieldList fields={fields} onChange={(f, v) => setText((t) => setField(at!, t, f, v))} /> : <textarea className="msettings-text" spellCheck={false} value={text} onChange={(e) => setText(e.target.value)} aria-label="Settings file" />}
          </div>
        </div>
      )}
    </Dialog>
  );
}

function FieldList({ fields, onChange }: { fields: Field[]; onChange: (f: Field, v: string) => void }) {
  let last = "\0";
  return (
    <div className="msettings-fields">
      {fields.map((f) => {
        const head = f.section !== last ? f.section : null;
        last = f.section;
        return (
          <div key={String(f.at)}>
            {head && <div className="msettings-section">{head}</div>}
            <label className="msettings-field">
              <span className="what">
                <b>{f.key}</b>
                {f.help && <span className="muted small">{f.help}</span>}
              </span>
              {f.kind === "bool" ? (
                <input type="checkbox" checked={/^true$/i.test(f.value)} onChange={(e) => onChange(f, matchCase(f.value, e.target.checked))} />
              ) : f.choices ? (
                <select value={f.value} onChange={(e) => onChange(f, e.target.value)}>
                  {[...new Set([f.value, ...f.choices])].map((c) => (
                    <option key={c}>{c}</option>
                  ))}
                </select>
              ) : (
                <input className={f.kind === "number" ? "num" : ""} inputMode={f.kind === "number" ? "decimal" : undefined} value={f.value} onChange={(e) => onChange(f, e.target.value)} />
              )}
            </label>
          </div>
        );
      })}
    </div>
  );
}

/** "True"/"true"/"TRUE": write back the way the file had it. */
function matchCase(was: string, on: boolean) {
  const w = on ? "true" : "false";
  if (was === was.toUpperCase()) return w.toUpperCase();
  if (was[0] === was[0].toUpperCase()) return w[0].toUpperCase() + w.slice(1);
  return w;
}
