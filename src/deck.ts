// The Steam Deck (and any big-screen or controller setup): a bigger interface, and the
// controller moving between buttons. "Auto" turns Deck mode on when the app runs on a Deck.

import { api, isDesktop } from "./api";

export type UiMode = "auto" | "deck" | "desktop";
const KEY = "zoms-nexus:ui";

export function readUi(): { mode: UiMode; scale: number } {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "{}") as { mode?: UiMode; scale?: number };
    return { mode: v.mode ?? "auto", scale: v.scale ?? 0 };
  } catch {
    return { mode: "auto", scale: 0 };
  }
}

export function saveUi(ui: { mode: UiMode; scale: number }) {
  try {
    localStorage.setItem(KEY, JSON.stringify(ui));
  } catch {
    /* kept for this run only */
  }
  void applyUi();
}

let device: { deck: boolean; steamos: boolean; gameMode: boolean; flatpak: boolean } | null = null;

/** This computer, asked once. */
export async function getDevice() {
  if (!device) device = await api.device().catch(() => ({ deck: false, steamos: false, gameMode: false, flatpak: false }));
  return device;
}

/** Whether Deck mode is on now. */
export const deckOn = () => document.documentElement.classList.contains("deck");

/** Apply the mode and size: Deck mode makes everything bigger and easier to reach. */
export async function applyUi() {
  const ui = readUi();
  const d = await getDevice();
  const deck = ui.mode === "deck" || (ui.mode === "auto" && (d.deck || d.gameMode));
  const root = document.documentElement;
  root.classList.toggle("deck", deck);
  // 0 means "the usual size for this mode".
  const scale = ui.scale || (deck ? 1.2 : 1);
  (root.style as CSSStyleDeclaration & { zoom: string }).zoom = scale === 1 ? "" : String(scale);
  window.dispatchEvent(new Event("zoms:ui"));
  // Steam's Game Mode shows one window at a time: fill the screen.
  if (d.gameMode && isDesktop && !fullOnce) {
    fullOnce = true;
    void import("@tauri-apps/api/window").then((w) => w.getCurrentWindow().setFullscreen(true)).catch(() => {});
  }
}
let fullOnce = false;
