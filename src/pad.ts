// Controller and arrow-key navigation, for the Steam Deck and couch play. The D-pad (or left
// stick, or the arrow keys Steam Input sends) moves to the nearest button in that direction, the
// way a console menu works. A presses it, B goes back (closes a dialog, or returns to the page
// list), Y plays, LB/RB change page, the triggers and right stick scroll, Menu finds anything.
// A text box opens Steam's on-screen keyboard on a Deck.

import { api, on } from "./api";
import { deckOn, getDevice } from "./deck";

const FOCUSABLE = 'button:not([disabled]), a[href], input:not([disabled]):not([type="hidden"]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export type Dir = "up" | "down" | "left" | "right";
export type Box = { left: number; top: number; right: number; bottom: number };

/** Where to look: the dialog on top when one is open, else the whole page. */
function scope(): ParentNode {
  const ds = document.querySelectorAll('[role="dialog"], [role="menu"]');
  return ds.length ? ds[ds.length - 1] : document;
}

function visible(el: HTMLElement) {
  if (!el.getClientRects().length) return false;
  const s = getComputedStyle(el);
  return s.visibility !== "hidden" && s.display !== "none" && !el.closest("[inert], [aria-hidden='true']");
}

export function targets(): HTMLElement[] {
  return [...scope().querySelectorAll<HTMLElement>(FOCUSABLE)].filter(visible);
}

/** The best next box from `from` in a direction: nearest along it, close to in line across it. */
export function pick(from: Box, dir: Dir, boxes: Box[]): number {
  let best = -1;
  let bestScore = Infinity;
  const cx = (b: Box) => (b.left + b.right) / 2;
  const cy = (b: Box) => (b.top + b.bottom) / 2;
  boxes.forEach((b, i) => {
    let along: number;
    let across: number;
    const gapX = Math.max(0, Math.max(b.left - from.right, from.left - b.right));
    const gapY = Math.max(0, Math.max(b.top - from.bottom, from.top - b.bottom));
    if (dir === "down") {
      if (cy(b) <= cy(from) + 1 || b.top < from.top + 2) return;
      along = Math.max(0, b.top - from.bottom);
      across = gapX * 2 + Math.abs(cx(b) - cx(from)) * 0.2;
    } else if (dir === "up") {
      if (cy(b) >= cy(from) - 1 || b.bottom > from.bottom - 2) return;
      along = Math.max(0, from.top - b.bottom);
      across = gapX * 2 + Math.abs(cx(b) - cx(from)) * 0.2;
    } else if (dir === "right") {
      if (cx(b) <= cx(from) + 1 || b.left < from.left + 2) return;
      along = Math.max(0, b.left - from.right);
      across = gapY * 4 + Math.abs(cy(b) - cy(from)) * 0.5;
    } else {
      if (cx(b) >= cx(from) - 1 || b.right > from.right - 2) return;
      along = Math.max(0, from.left - b.right);
      across = gapY * 4 + Math.abs(cy(b) - cy(from)) * 0.5;
    }
    const score = along + across;
    if (score < bestScore) {
      bestScore = score;
      best = i;
    }
  });
  return best;
}

function show(el: HTMLElement) {
  el.focus({ preventScroll: true });
  el.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
}

/** Start somewhere sensible: the dialog's first control, or the page's first one. */
function start() {
  const all = targets();
  const inMain = all.find((e) => e.closest(".main")) ?? all[0];
  if (inMain) show(inMain);
}

const typing = (el: Element | null) => !!el && (el.matches("textarea") || (el.matches("input") && !["checkbox", "radio", "range", "button", "submit"].includes((el as HTMLInputElement).type)));

/** Move the focus one step. */
export function move(dir: Dir) {
  document.documentElement.classList.add("pad");
  const cur = document.activeElement as HTMLElement | null;
  const all = targets();
  if (!cur || cur === document.body || !all.includes(cur)) return start();
  // A list to choose from: left and right change its choice.
  if (cur instanceof HTMLSelectElement && (dir === "left" || dir === "right")) {
    const i = Math.min(cur.options.length - 1, Math.max(0, cur.selectedIndex + (dir === "right" ? 1 : -1)));
    if (i !== cur.selectedIndex) {
      cur.selectedIndex = i;
      cur.dispatchEvent(new Event("change", { bubbles: true }));
    }
    return;
  }
  // Messages at the bottom are only stepped into on purpose: from inside one, or with nothing else that way.
  const inToast = !!cur.closest(".toast");
  const main = all.filter((e) => e !== cur && (inToast || !e.closest(".toast")));
  const others = pick(cur.getBoundingClientRect(), dir, main.map((e) => e.getBoundingClientRect())) >= 0 ? main : all.filter((e) => e !== cur);
  const i = pick(cur.getBoundingClientRect(), dir, others.map((e) => e.getBoundingClientRect()));
  if (i >= 0) show(others[i]);
}

/** A: press what has the focus. A text box gets Steam's keyboard on a Deck. */
async function press() {
  const el = document.activeElement as HTMLElement | null;
  if (!el || el === document.body) return start();
  if (typing(el)) {
    if ((await getDevice()).deck || (await getDevice()).gameMode) void api.openSteam("keyboard").catch(() => {});
    return;
  }
  if (el instanceof HTMLSelectElement) {
    move("right");
    return;
  }
  el.click();
}

function key(k: string, extra: KeyboardEventInit = {}) {
  const target = (document.activeElement as HTMLElement | null) ?? document.body;
  target.dispatchEvent(new KeyboardEvent("keydown", { key: k, bubbles: true, cancelable: true, ...extra }));
}

/** B: close the dialog or menu on top, else go back to the page list. */
function back() {
  if (document.querySelector('[role="dialog"], [role="menu"]')) return key("Escape");
  const nav = document.querySelector<HTMLElement>(".nav-item.active");
  if (nav && document.activeElement !== nav) show(nav);
}

/** X: the focused row's menu (what a right-click opens), at the row. */
function menu() {
  const el = document.activeElement as HTMLElement | null;
  if (!el || el === document.body) return;
  const r = el.getBoundingClientRect();
  // From the focused button, so it reaches whichever row around it has a menu.
  el.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: r.left + 12, clientY: r.bottom }));
  // Into the menu that opened.
  setTimeout(() => document.querySelector<HTMLElement>('[role="menu"] button')?.focus(), 30);
}

function scrollBy(dy: number) {
  let el = (document.activeElement as HTMLElement | null) ?? null;
  while (el && el !== document.body) {
    const s = getComputedStyle(el);
    if (/(auto|scroll)/.test(s.overflowY) && el.scrollHeight > el.clientHeight) break;
    el = el.parentElement;
  }
  const box = el && el !== document.body ? el : (document.querySelector(".main") as HTMLElement | null) ?? document.scrollingElement;
  box?.scrollBy({ top: dy });
}

/** What the app does for page and play buttons. */
export type PadActions = { page: (delta: number) => void; play: () => void; find: () => void };
let actions: PadActions | null = null;
export const setPadActions = (a: PadActions | null) => {
  actions = a;
};

const ARROWS: Record<string, Dir> = { ArrowUp: "up", ArrowDown: "down", ArrowLeft: "left", ArrowRight: "right" };

/** Arrow keys move like the D-pad in Deck mode (Steam Input's keyboard layouts send these). */
function onKey(e: KeyboardEvent) {
  const dir = ARROWS[e.key];
  if (!dir || !deckOn() || e.ctrlKey || e.altKey || e.metaKey || e.defaultPrevented) return;
  const el = document.activeElement;
  // In a text box, left and right edit; up and down leave it.
  if (typing(el) && (dir === "left" || dir === "right")) return;
  if (el?.closest(".pal-search, .palette") && (dir === "up" || dir === "down")) return; // The finder moves its own list.
  e.preventDefault();
  move(dir);
}

// Standard gamepad buttons.
const A = 0, B = 1, X = 2, Y = 3, LB = 4, RB = 5, LT = 6, RT = 7, MENU = 9, UP = 12, DOWN = 13, LEFT = 14, RIGHT = 15;

/** The app reads controllers itself: then the page's own gamepad reading stays quiet. */
let native = false;

/** A game is starting or running: the controller is the game's, whatever has focus. */
let gameOn = false;
export const setGameRunning = (on: boolean) => {
  gameOn = on;
};

/** Only while this window is the one in front and no game is playing (in Steam's Game Mode a
 *  game covers the app without always taking its focus). */
const ours = () => !gameOn && document.hasFocus() && document.visibilityState === "visible";

function nativePad() {
  const held: Record<string, number> = {};
  const repeat = (b: string, every: number, f: () => void) => {
    f();
    window.clearInterval(held[b]);
    const start = Date.now();
    held[b] = window.setInterval(() => {
      if (!ours()) return;
      if (Date.now() - start > 380) f();
    }, every);
  };
  void on<{ button: string; down: boolean }>("pad", ({ button, down }) => {
    native = true;
    if (!down) {
      window.clearInterval(held[button]);
      delete held[button];
      return;
    }
    if (!ours()) return;
    document.documentElement.classList.add("pad", "pad-controller");
    switch (button) {
      case "up":
      case "down":
      case "left":
      case "right":
        return repeat(button, 95, () => move(button as Dir));
      case "a":
        return void press();
      case "b":
        return back();
      case "x":
        return menu();
      case "y":
        return actions?.play();
      case "lb":
        return actions?.page(-1);
      case "rb":
        return actions?.page(1);
      case "menu":
        return actions?.find();
      case "lt":
        return repeat(button, 16, () => scrollBy(-18));
      case "rt":
        return repeat(button, 16, () => scrollBy(18));
    }
  });
  void on<number>("pad-scroll", (v) => {
    if (ours()) scrollBy(v * 22);
  });
}

let polling = false;
function loop() {
  polling = true;
  const was: boolean[] = [];
  let held: { dir: Dir; since: number; last: number } | null = null;
  const frame = (now: number) => {
    const pad = [...(navigator.getGamepads?.() ?? [])].find((p) => p && p.connected);
    if (!pad) {
      polling = false;
      return;
    }
    const down = (i: number) => !!pad.buttons[i]?.pressed;
    const fresh = (i: number) => down(i) && !was[i];
    const ax = pad.axes[0] ?? 0;
    const ay = pad.axes[1] ?? 0;
    const dir: Dir | null = down(UP) || ay < -0.6 ? "up" : down(DOWN) || ay > 0.6 ? "down" : down(LEFT) || ax < -0.6 ? "left" : down(RIGHT) || ax > 0.6 ? "right" : null;
    if (!native && ours()) {
      if (dir) {
        // Held: one step, a pause, then steady steps.
        if (!held || held.dir !== dir) {
          held = { dir, since: now, last: now };
          move(dir);
        } else if (now - held.since > 380 && now - held.last > 95) {
          held.last = now;
          move(dir);
        }
      } else held = null;
      if (fresh(A)) void press();
      if (fresh(B)) back();
      if (fresh(X)) menu();
      if (fresh(Y)) actions?.play();
      if (fresh(LB)) actions?.page(-1);
      if (fresh(RB)) actions?.page(1);
      if (fresh(MENU)) actions?.find();
      if (down(LT)) scrollBy(-18);
      if (down(RT)) scrollBy(18);
      const ry = pad.axes[3] ?? 0;
      if (Math.abs(ry) > 0.2) scrollBy(ry * 22);
      if (pad.buttons.some((b) => b.pressed)) document.documentElement.classList.add("pad", "pad-controller");
    }
    pad.buttons.forEach((b, i) => (was[i] = b.pressed));
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
}

/** Listen for the controller and the arrow keys. Call once. */
export function startPad() {
  window.addEventListener("keydown", onKey);
  nativePad();
  window.addEventListener("gamepadconnected", () => {
    if (!polling) loop();
  });
  if ([...(navigator.getGamepads?.() ?? [])].some(Boolean)) loop();
  // The mouse or touch screen takes over: hide the focus ring until the controller is used.
  const quiet = () => document.documentElement.classList.remove("pad", "pad-controller");
  window.addEventListener("mousedown", quiet);
  window.addEventListener("touchstart", quiet, { passive: true });
}
