// Small shared pieces: dialogs, the toast, the stepper, status pills.

import { bytes } from "./format";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { X } from "lucide-react";
import type { Status } from "./api";

export function Dialog({ title, onClose, children, foot, wide }: { title: ReactNode; onClose: () => void; children: ReactNode; foot?: ReactNode; wide?: boolean }) {
  useEffect(() => {
    const k = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onClose]);
  return (
    <div className="backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`dialog${wide ? " wide" : ""}`} role="dialog" aria-modal="true">
        <div className="dialog-head">
          <div className="dialog-title">
            <h2>{title}</h2>
            <button className="icon-btn" aria-label="Close" onClick={onClose}>
              <X size={18} />
            </button>
          </div>
        </div>
        <div className="dialog-body">{children}</div>
        {foot && <div className="dialog-foot">{foot}</div>}
      </div>
    </div>
  );
}

/** "Delete X? ..." with Cancel and the action in red. */
export function Confirm({ title, body, action, onConfirm, onClose }: { title: string; body: ReactNode; action: string; onConfirm: () => void | Promise<void>; onClose: () => void }) {
  const [busy, setBusy] = useState(false);
  return (
    <Dialog
      title={title}
      onClose={onClose}
      foot={
        <>
          <button className="btn" onClick={onClose}>
            Cancel
          </button>
          <button
            className="btn danger solid"
            disabled={busy}
            onClick={async () => {
              setBusy(true);
              try {
                await onConfirm();
              } finally {
                setBusy(false);
                onClose();
              }
            }}
          >
            {action}
          </button>
        </>
      }
    >
      <p style={{ margin: 0 }}>{body}</p>
    </Dialog>
  );
}

export interface ToastMsg {
  id: number;
  text: string;
  action?: { label: string; run: () => void };
}

export function Toast({ toast, onDone }: { toast: ToastMsg | null; onDone: () => void }) {
  const timer = useRef<number | undefined>(undefined);
  // The latest onDone, so a redraw (several a second while downloading) never restarts the clock.
  const done = useRef(onDone);
  done.current = onDone;
  // Long enough to read: about a second per 15 characters, 2.5 to 6 seconds.
  const ms = toast ? Math.min(6000, Math.max(2500, toast.text.length * 65)) + (toast.action ? 1500 : 0) : 0;
  const start = () => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => done.current(), ms);
  };
  useEffect(() => {
    if (!toast) return;
    start();
    return () => window.clearTimeout(timer.current);
  }, [toast?.id]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!toast) return null;
  return (
    <div className="toast" role="status" key={toast.id} onMouseEnter={() => window.clearTimeout(timer.current)} onMouseLeave={start}>
      <span>{toast.text}</span>
      {toast.action && (
        <button
          className="btn sm"
          onClick={() => {
            toast.action!.run();
            onDone();
          }}
        >
          {toast.action.label}
        </button>
      )}
      <button className="icon-btn" aria-label="Close" style={{ color: "inherit" }} onClick={onDone}>
        <X size={14} />
      </button>
    </div>
  );
}

export function Stepper({ value, min, max, onChange, disabled, label }: { value: number; min: number; max: number; onChange: (n: number) => void; disabled?: boolean; label: string }) {
  const set = (n: number) => onChange(Math.min(max, Math.max(min, n)));
  return (
    <span className={`stepper sm${disabled ? " disabled" : ""}`}>
      <button disabled={disabled || value <= min} aria-label={`Fewer ${label}`} onClick={() => set(value - 1)}>
        −
      </button>
      <input type="number" aria-label={label} value={value} min={min} max={max} disabled={disabled} onChange={(e) => set(Number(e.target.value) || min)} />
      <button disabled={disabled || value >= max} aria-label={`More ${label}`} onClick={() => set(value + 1)}>
        +
      </button>
    </span>
  );
}

const STATUS: Record<Status, [string, string]> = {
  queued: ["Waiting", ""],
  starting: ["Starting", "accent"],
  downloading: ["Downloading", "accent"],
  storing: ["Storing", "accent"],
  paused: ["Paused", ""],
  waiting: ["Trying again soon", "warn"],
  needsLink: ["Needs a new link", "warn"],
  failed: ["Didn't work", "danger"],
  done: ["In library", "ok"],
};

export function StatusPill({ status }: { status: Status }) {
  const [word, kind] = STATUS[status];
  return <span className={`pill ${kind}`}>{word}</span>;
}

/** The wordmark: icon, then Zom's in the text color and Nexus in the accent. */
export function Wordmark() {
  return (
    <div className="brand">
      <img src={`${import.meta.env.BASE_URL}app-icon.svg`} width={36} height={36} alt="" />
      <div className="brand-text">
        <div className="brand-name">
          <span>Zom's</span> Nexus
        </div>
        <div className="brand-tag">MOD MANAGER</div>
      </div>
    </div>
  );
}

/** How full a drive is: a bar with the free space in words. Turns amber under 10% free. */
export function DiskBar({ free, total, label }: { free?: number | null; total?: number | null; label?: string }) {
  if (free == null) return null;
  const used = total ? Math.min(1, Math.max(0, (total - free) / total)) : null;
  const low = total ? free / total < 0.1 : false;
  return (
    <div className={`disk${low ? " low" : ""}`} title={total ? `${bytes(total - free)} used of ${bytes(total)}` : undefined}>
      {used != null && (
        <div className="disk-bar" role="meter" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(used * 100)} aria-label={label ?? "Drive space used"}>
          <span style={{ width: `${used * 100}%` }} />
        </div>
      )}
      <span className="disk-text num">
        {bytes(free)} free{total ? ` of ${bytes(total)}` : ""}
        {label ? ` · ${label}` : ""}
      </span>
    </div>
  );
}

/** One drive, split into what fills it: unpacked mods, mod archives, Steam games, everything
 * else, and free space. */
export function SpaceBar({ d }: { d: { holds: string[]; total: number; free: number; mods: number; archives: number; steam: number } }) {
  if (!d.total) return null;
  const used = d.total - d.free;
  const other = Math.max(0, used - d.mods - d.archives - d.steam);
  const parts = [
    { k: "mods", label: "Mods", v: d.mods },
    { k: "zips", label: "Zips", v: d.archives },
    { k: "steam", label: "Steam", v: d.steam },
    { k: "other", label: "Other", v: other },
  ].filter((p) => p.v > 0);
  const low = d.free / d.total < 0.1;
  return (
    <div className={`disk space${low ? " low" : ""}`}>
      <span className="disk-text num">
        <b>{d.holds.join(" and ")}</b> · {bytes(d.free)} free of {bytes(d.total)}
      </span>
      <div className="space-bar" role="img" aria-label={`${parts.map((p) => `${p.label} ${bytes(p.v)}`).join(", ")}, ${bytes(d.free)} free`}>
        {parts.map((p) => (
          <span key={p.k} className={`seg-${p.k}`} style={{ width: `${(p.v / d.total) * 100}%` }} title={`${p.label}: ${bytes(p.v)}`} />
        ))}
      </div>
      <span className="space-key">
        {parts.map((p) => (
          <span key={p.k} title={bytes(p.v)}>
            <i className={`seg-${p.k}`} />
            {p.label} {bytes(p.v)}
          </span>
        ))}
      </span>
    </div>
  );
}

/** A right-click menu entry, or a separator line between groups. */
export type MenuEntry = { label: string; icon?: ReactNode; run: () => void; danger?: boolean } | "sep";
export interface MenuState {
  x: number;
  y: number;
  items: MenuEntry[];
}

/** The right-click menu: icon and label rows, groups split by lines, dangerous ones last. */
export function ContextMenu({ menu, onClose }: { menu: MenuState | null; onClose: () => void }) {
  const box = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: 0, top: 0 });
  useEffect(() => {
    if (!menu) return;
    // Keep it inside the window.
    const w = box.current?.offsetWidth ?? 240;
    const h = box.current?.offsetHeight ?? 200;
    setPos({ left: Math.max(8, Math.min(menu.x, window.innerWidth - w - 8)), top: Math.max(8, Math.min(menu.y, window.innerHeight - h - 8)) });
    const away = (e: Event) => !box.current?.contains(e.target as Node) && onClose();
    const key = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("mousedown", away);
    window.addEventListener("keydown", key);
    window.addEventListener("resize", onClose);
    window.addEventListener("wheel", onClose, { passive: true });
    return () => {
      window.removeEventListener("mousedown", away);
      window.removeEventListener("keydown", key);
      window.removeEventListener("resize", onClose);
      window.removeEventListener("wheel", onClose);
    };
  }, [menu, onClose]);
  if (!menu) return null;
  return (
    <div className="menu" role="menu" ref={box} style={pos} onContextMenu={(e) => e.preventDefault()}>
      {menu.items.map((it, i) =>
        it === "sep" ? (
          <div key={i} className="menu-sep" />
        ) : (
          <button
            key={i}
            role="menuitem"
            className={`menu-item${it.danger ? " danger" : ""}`}
            onClick={() => {
              onClose();
              it.run();
            }}
          >
            <span className="menu-icon">{it.icon}</span>
            <span className="grow">{it.label}</span>
          </button>
        ),
      )}
    </div>
  );
}

/** Drop separators at the ends and doubled ones, so menus built from conditions stay tidy. */
export function tidy(items: (MenuEntry | false | null | undefined)[]): MenuEntry[] {
  const out: MenuEntry[] = [];
  for (const it of items) {
    if (!it) continue;
    if (it === "sep" && (out.length === 0 || out[out.length - 1] === "sep")) continue;
    out.push(it);
  }
  while (out[out.length - 1] === "sep") out.pop();
  return out;
}
