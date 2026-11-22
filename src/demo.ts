// The demo: a made-up account, games, mods and downloads, kept in this browser tab only.
// Downloads are pretend, but they follow the same rules as the real queue (one at a time, or
// several at once up to the limit), so the demo shows how the modes behave.

import type { CheckReport, FileInfo, Game, Job, LinkStatus, ModInfo, Overview, Profile, Settings, StoredMod } from "./api";

const listeners = new Map<string, Set<(p: unknown) => void>>();
const emit = (event: string, payload: unknown) => listeners.get(event)?.forEach((h) => h(payload));

export function on(event: string, handler: (p: unknown) => void) {
  if (!listeners.has(event)) listeners.set(event, new Set());
  listeners.get(event)!.add(handler);
  return () => listeners.get(event)?.delete(handler);
}

const now = () => Math.floor(Date.now() / 1000);

// Placeholder pictures for the demo: drawn here, so no real game or mod art is used.
const HUES: Record<string, number> = { cyberhunk2088: 52, frostrim: 210, stardudevalley: 95 };
const hue = (k: string) => HUES[k] ?? [...k].reduce((n, c) => (n * 31 + c.charCodeAt(0)) % 360, 7);
const svg = (w: number, h: number, body: string) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${w} ${h}" width="${w}" height="${h}">${body}</svg>`)}`;
const esc = (t: string) => t.replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]!);
function placeholder(key: string, title: string, kind: "cover" | "hero" | "logo" | "mod") {
  const h = hue(key);
  const [w, ht] = kind === "cover" ? [600, 900] : kind === "hero" ? [1920, 620] : kind === "logo" ? [800, 260] : [640, 360];
  const size = (base: number, per: number) => Math.min(base, Math.floor(per / Math.max(1, title.length)));
  if (kind === "logo")
    return svg(w, ht, `<text x="50%" y="58%" text-anchor="middle" font-family="sans-serif" font-weight="800" font-size="${size(88, 1300)}" fill="#fff" stroke="hsl(${h} 50% 20%)" stroke-width="3">${esc(title)}</text>`);
  const scene = key.startsWith("cyberhunk") ? city(w, ht, h) : key.startsWith("frostrim") ? peaks(w, ht, h) : hills(w, ht, h);
  // Covers and banners get the game's hero standing in front of its scene.
  const who = kind === "mod" ? "" : key.startsWith("cyberhunk") ? runner(w, ht) : key.startsWith("frostrim") ? warrior(w, ht) : farmer(w, ht);
  const label =
    kind === "cover"
      ? `<rect y="${ht * 0.1}" width="${w}" height="${ht * 0.16}" fill="rgba(0,0,0,.6)"/><text x="50%" y="${ht * 0.2}" text-anchor="middle" font-family="sans-serif" font-weight="800" font-size="${size(56, 1000)}" fill="#fff">${esc(title)}</text>`
      : kind === "mod"
        ? `<rect y="${ht - 70}" width="${w}" height="70" fill="rgba(0,0,0,.4)"/><text x="24" y="${ht - 24}" font-family="sans-serif" font-weight="700" font-size="${size(30, 900)}" fill="#fff">${esc(title)}</text>`
        : "";
  return svg(w, ht, scene + who + label);
}

/** A picture in public/demo-art/ (named like "cyberhunk2088-cover.webp"), if one is there. */
async function demoArt(name: string): Promise<string | null> {
  for (const ext of ["webp", "jpg", "png"]) {
    const url = `/demo-art/${name}.${ext}`;
    try {
      const r = await fetch(url, { method: "HEAD" });
      if (r.ok && (r.headers.get("content-type") ?? "").startsWith("image/")) return url;
    } catch {
      /* not there */
    }
  }
  return null;
}

/** Where the hero stands, and how big: centred on covers, to the right on wide banners. */
function stage(w: number, ht: number) {
  const tall = ht > w;
  const s = tall ? ht / 900 : ht / 620;
  return { x: tall ? w / 2 : w * 0.72, y: ht, s };
}

/** Cyberhunk 2088: chest-up, undercut, a glowing wraparound visor, a high-collar jacket with
 * neon seams, lit cyan from one side and magenta from the other by the city. */
function runner(w: number, ht: number) {
  const { x, y, s } = stage(w, ht);
  return `<defs>
    <linearGradient id="rSkin" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#c98b6c"/><stop offset=".55" stop-color="#e7b293"/><stop offset="1" stop-color="#a86b50"/></linearGradient>
    <linearGradient id="rJacket" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2b2e3a"/><stop offset="1" stop-color="#0c0d12"/></linearGradient>
    <linearGradient id="rVisor" x1="0" y1="0" x2="1" y2="0"><stop offset="0" stop-color="#19f2ff"/><stop offset=".6" stop-color="#7d5cff"/><stop offset="1" stop-color="#ff2a6d"/></linearGradient>
    <linearGradient id="rHair" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#3a3f55"/><stop offset="1" stop-color="#101118"/></linearGradient>
    <filter id="rGlow" x="-50%" y="-50%" width="200%" height="200%"><feGaussianBlur stdDeviation="9"/></filter>
  </defs>
  <g transform="translate(${x} ${y}) scale(${s * 0.84})">
    <path d="M-260 0 C-250 -150 -205 -230 -120 -262 L-58 -282 L58 -282 L120 -262 C205 -230 250 -150 260 0 Z" fill="url(#rJacket)"/>
    <path d="M-58 -282 L0 -150 L58 -282 Z" fill="#7a1028"/>
    <path d="M-50 -270 L0 -160 L50 -270" stroke="#ff2a6d" stroke-width="3" fill="none" opacity=".7"/>
    <path d="M-120 -262 C-125 -320 -100 -360 -70 -372 L-52 -282 Z" fill="#20232d"/>
    <path d="M120 -262 C125 -320 100 -360 70 -372 L52 -282 Z" fill="#20232d"/>
    <path d="M-120 -262 C-125 -320 -100 -360 -70 -372" stroke="#19f2ff" stroke-width="5" fill="none"/>
    <path d="M120 -262 C125 -320 100 -360 70 -372" stroke="#ff2a6d" stroke-width="5" fill="none"/>
    <path d="M-185 -150 L-150 -10 M185 -150 L150 -10" stroke="#19f2ff" stroke-width="3" opacity=".45"/>
    <g transform="translate(0 -300) scale(1.32) translate(0 300)">
    <path d="M-46 -350 L-42 -280 L42 -280 L46 -350 Z" fill="#a86b50"/>
    <path d="M-78 -470 C-80 -405 -58 -352 0 -334 C58 -352 80 -405 78 -470 C76 -548 -76 -548 -78 -470 Z" fill="url(#rSkin)"/>
    <ellipse cx="-80" cy="-450" rx="10" ry="20" fill="#b9785b"/><ellipse cx="80" cy="-450" rx="10" ry="20" fill="#b9785b"/>
    <path d="M-62 -400 C-50 -360 -20 -342 0 -340 C20 -342 50 -360 62 -400 C50 -372 20 -356 0 -356 C-20 -356 -50 -372 -62 -400 Z" fill="#5a3b2e" opacity=".35"/>
    <path d="M-8 -440 L-14 -398 L4 -396" stroke="#9a5f45" stroke-width="4" fill="none" stroke-linecap="round"/>
    <path d="M-22 -377 Q0 -370 24 -379" stroke="#7d4436" stroke-width="5" fill="none" stroke-linecap="round"/>
    <path d="M-79 -470 C-82 -500 -80 -520 -70 -535 L-62 -478 Z M79 -470 C82 -500 80 -520 70 -535 L62 -478 Z" fill="#2a2d3c" opacity=".9"/>
    <path d="M-80 -505 C-92 -575 -40 -612 18 -610 C78 -608 106 -570 94 -508 C86 -536 66 -552 36 -556 C6 -560 -34 -550 -60 -526 C-70 -518 -77 -512 -80 -505 Z" fill="url(#rHair)"/>
    <path d="M-50 -570 C-10 -598 50 -600 88 -566 M-30 -585 C10 -604 52 -604 80 -586" stroke="#7d86ad" stroke-width="5" fill="none" opacity=".6" stroke-linecap="round"/>
    <path d="M-90 -470 Q0 -490 90 -470 L86 -436 Q0 -452 -86 -436 Z" fill="url(#rVisor)" filter="url(#rGlow)" opacity=".9"/>
    <path d="M-90 -470 Q0 -490 90 -470 L86 -436 Q0 -452 -86 -436 Z" fill="url(#rVisor)"/>
    <path d="M-80 -466 Q0 -482 80 -466" stroke="#ffffff" stroke-width="4" fill="none" opacity=".75"/>
    <path d="M-78 -470 C-80 -405 -58 -352 0 -334" stroke="#19f2ff" stroke-width="5" fill="none" opacity=".8"/>
    <path d="M78 -470 C80 -405 58 -352 0 -334" stroke="#ff2a6d" stroke-width="5" fill="none" opacity=".8"/>
    </g>
  </g>`;
}

/** Frostrim: a warrior in a horned helmet and a fur cloak. */
function warrior(w: number, ht: number) {
  const { x, y, s } = stage(w, ht);
  return `<g transform="translate(${x} ${y}) scale(${s})">
    <path d="M-230 0 C-215 -190 -160 -270 -100 -290 L100 -290 C160 -270 215 -190 230 0 Z" fill="#3b2f26"/>
    <path d="M-130 -285 C-90 -320 90 -320 130 -285 C100 -260 -100 -260 -130 -285 Z" fill="#8a7864"/>
    <path d="M-80 -270 L80 -270 L70 0 L-70 0 Z" fill="#59606b"/>
    <path d="M-80 -200 L80 -200 M-78 -140 L78 -140" stroke="#3e434b" stroke-width="8"/>
    <ellipse cx="0" cy="-360" rx="62" ry="74" fill="#e1b896"/>
    <path d="M-40 -330 Q0 -270 40 -330 L40 -310 Q0 -240 -40 -310 Z" fill="#b5703c"/>
    <path d="M-72 -380 C-72 -470 72 -470 72 -380 L72 -365 L-72 -365 Z" fill="#7d858f"/>
    <rect x="-8" y="-375" width="16" height="60" fill="#7d858f"/>
    <path d="M-70 -410 C-130 -420 -150 -480 -130 -520 C-120 -470 -95 -450 -62 -440 Z" fill="#efe7d6"/>
    <path d="M70 -410 C130 -420 150 -480 130 -520 C120 -470 95 -450 62 -440 Z" fill="#efe7d6"/>
  </g>`;
}

/** Stardude Valley: a friendly farmer in a woven straw hat, plaid shirt and overalls, with a
 * hoe over his shoulder. */
function farmer(w: number, ht: number) {
  const { x, y, s } = stage(w, ht);
  return `<defs>
    <pattern id="fPlaid" width="40" height="40" patternUnits="userSpaceOnUse"><rect width="40" height="40" fill="#c0392b"/><rect width="40" height="12" y="14" fill="#8e2a20" opacity=".55"/><rect width="12" height="40" x="14" fill="#8e2a20" opacity=".55"/><rect width="40" height="2" y="19" fill="#f3d6a3" opacity=".5"/></pattern>
    <pattern id="fWeave" width="16" height="16" patternUnits="userSpaceOnUse"><rect width="16" height="16" fill="#e9c46a"/><path d="M0 8 H16 M8 0 V16" stroke="#c9a24a" stroke-width="2"/></pattern>
    <linearGradient id="fDenim" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#4a7cc4"/><stop offset="1" stop-color="#2c528a"/></linearGradient>
    <radialGradient id="fSkin" cx=".45" cy=".4" r=".7"><stop offset="0" stop-color="#f6cba8"/><stop offset="1" stop-color="#d89a72"/></radialGradient>
  </defs>
  <g transform="translate(${x} ${y}) scale(${s})">
    <path d="M110 -20 L250 -600" stroke="#8b5a2b" stroke-width="16" stroke-linecap="round"/>
    <path d="M222 -610 C250 -630 300 -620 312 -590 L262 -585 Z" fill="#9aa3ab"/>
    <path d="M-235 0 C-225 -150 -185 -235 -110 -262 L110 -262 C185 -235 225 -150 235 0 Z" fill="url(#fPlaid)"/>
    <path d="M-120 0 L-110 -170 L110 -170 L120 0 Z" fill="url(#fDenim)"/>
    <path d="M-110 -170 L-95 -262 M110 -170 L95 -262" stroke="#2c528a" stroke-width="26" stroke-linecap="round"/>
    <circle cx="-100" cy="-180" r="11" fill="#f1c40f"/><circle cx="100" cy="-180" r="11" fill="#f1c40f"/>
    <rect x="-40" y="-130" width="80" height="50" rx="8" fill="#3a66a8"/>
    <path d="M60 -250 C120 -230 175 -150 160 -70" stroke="url(#fPlaid)" stroke-width="58" stroke-linecap="round" fill="none"/>
    <circle cx="160" cy="-70" r="30" fill="#e8ad86"/>
    <path d="M-42 -335 L-40 -262 L40 -262 L42 -335 Z" fill="#d89a72"/>
    <path d="M-30 -262 L0 -235 L30 -262 Z" fill="#8e2a20"/>
    <ellipse cx="0" cy="-400" rx="80" ry="86" fill="url(#fSkin)"/>
    <ellipse cx="-80" cy="-395" rx="13" ry="20" fill="#d89a72"/><ellipse cx="80" cy="-395" rx="13" ry="20" fill="#d89a72"/>
    <path d="M-70 -440 C-60 -470 -30 -470 -20 -455 M70 -440 C60 -470 30 -470 20 -455" stroke="#7a4a2a" stroke-width="8" fill="none" stroke-linecap="round"/>
    <ellipse cx="-34" cy="-418" rx="10" ry="13" fill="#3b2a20"/><ellipse cx="34" cy="-418" rx="10" ry="13" fill="#3b2a20"/>
    <circle cx="-30" cy="-422" r="4" fill="#fff"/><circle cx="38" cy="-422" r="4" fill="#fff"/>
    <ellipse cx="-50" cy="-382" rx="18" ry="11" fill="#e8786a" opacity=".55"/><ellipse cx="50" cy="-382" rx="18" ry="11" fill="#e8786a" opacity=".55"/>
    <g fill="#b06a45" opacity=".7"><circle cx="-58" cy="-395" r="3"/><circle cx="-46" cy="-400" r="2.5"/><circle cx="-52" cy="-388" r="2.5"/><circle cx="58" cy="-395" r="3"/><circle cx="46" cy="-400" r="2.5"/><circle cx="52" cy="-388" r="2.5"/></g>
    <path d="M-6 -405 Q0 -390 8 -398" stroke="#c07a55" stroke-width="5" fill="none" stroke-linecap="round"/>
    <path d="M-38 -362 Q0 -330 38 -362 Q0 -345 -38 -362 Z" fill="#7d3328"/>
    <path d="M-30 -360 Q0 -350 30 -360" stroke="#fff" stroke-width="5" fill="none"/>
    <path d="M-78 -450 C-90 -430 -88 -405 -80 -395 L-70 -440 Z M78 -450 C90 -430 88 -405 80 -395 L70 -440 Z" fill="#7a4a2a"/>
    <ellipse cx="0" cy="-462" rx="160" ry="34" fill="url(#fWeave)"/>
    <ellipse cx="0" cy="-462" rx="160" ry="34" fill="none" stroke="#b8913c" stroke-width="5"/>
    <path d="M-82 -462 C-82 -560 82 -560 82 -462 Z" fill="url(#fWeave)"/>
    <path d="M-82 -462 C-82 -560 82 -560 82 -462" fill="none" stroke="#b8913c" stroke-width="5"/>
    <path d="M-80 -478 C-40 -470 40 -470 80 -478 L82 -500 C40 -492 -40 -492 -82 -500 Z" fill="#c0392b"/>
  </g>`;
}

/** A night city: dark sky, glowing windows, neon bands. */
function city(w: number, ht: number, _h: number) {
  let out = `<rect width="${w}" height="${ht}" fill="#fdf500"/><rect y="${ht * 0.3}" width="${w}" height="${ht * 0.04}" fill="#19f2ff" opacity=".8"/><rect x="${w * 0.08}" y="${ht * 0.36}" width="${w * 0.5}" height="${ht * 0.015}" fill="#ff2a6d"/>`;
  let x = 0;
  for (let i = 0; x < w; i++) {
    const bw = 40 + ((i * 37) % 70);
    const bh = ht * (0.35 + ((i * 53) % 40) / 100);
    out += `<rect x="${x}" y="${ht - bh * 0.7}" width="${bw}" height="${bh}" fill="#20222b" opacity=".9"/>`;
    for (let wy = ht - bh * 0.7 + 14; wy < ht - 10; wy += 22) if ((i + wy) % 3) out += `<rect x="${x + 8}" y="${wy}" width="${bw - 16}" height="5" fill="${i % 2 ? "#19f2ff" : "#ff2a6d"}" opacity=".6"/>`;
    x += bw + 6;
  }
  return out;
}

/** Snowy mountains under a pale sky. */
function peaks(w: number, ht: number, h: number) {
  let out = `<defs><linearGradient id="p" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="hsl(${h} 50% 82%)"/><stop offset="1" stop-color="hsl(${h} 40% 55%)"/></linearGradient></defs><rect width="${w}" height="${ht}" fill="url(#p)"/>`;
  [0.5, 0.62, 0.75].forEach((base, i) => {
    const y = ht * base;
    const pts: string[] = [`0,${ht}`, `0,${y}`];
    for (let k = 0; k <= 6; k++) pts.push(`${(w * k) / 6},${y - (k % 2 ? ht * (0.22 - i * 0.05) : 0)}`);
    pts.push(`${w},${ht}`);
    out += `<polygon points="${pts.join(" ")}" fill="hsl(${h} 30% ${70 - i * 18}%)"/>`;
  });
  return out + `<rect y="${ht * 0.88}" width="${w}" height="${ht * 0.12}" fill="#f4f7fb"/>`;
}

/** Rolling green fields and a big sun. */
function hills(w: number, ht: number, h: number) {
  let out = `<rect width="${w}" height="${ht}" fill="hsl(${h + 100} 70% 80%)"/><circle cx="${w * 0.75}" cy="${ht * 0.28}" r="${Math.min(w, ht) * 0.12}" fill="hsl(48 95% 65%)"/>`;
  for (let i = 0; i < 4; i++) {
    const y = ht * (0.48 + i * 0.13);
    out += `<path d="M0 ${y} Q ${w * 0.3} ${y - ht * 0.16} ${w * 0.6} ${y} T ${w} ${y - ht * 0.04} V ${ht} H 0 Z" fill="hsl(${h} 50% ${46 - i * 7}%)"/>`;
  }
  return out;
}
const MB = 1_000_000;

let settings: Settings = {
  libraryDir: "/home/alex/Games/Nexus Mods",
  mode: "auto",
  maxConcurrent: 3,
  extract: true,
  keepArchives: true,
  autoRetry: true,
  verifyDownloads: true,
  speedLimit: 0,
  games: [{ domain: "stardudevalley", name: "Stardude Valley", path: "/home/alex/Games/stardude" }],
};
let hasKey = true;
let linksOurs = false;

const games: Game[] = [
  { domain: "cyberhunk2088", name: "Cyberhunk 2088", path: "/home/alex/.local/share/Steam/steamapps/common/Cyberhunk 2088", source: "steam", steamAppId: 9900001, protonPrefix: "/home/alex/.local/share/Steam/steamapps/compatdata/9900001/pfx", modFolder: "", freeSpace: 182_000_000_000, totalSpace: 1_000_000_000_000 },
  { domain: "frostrim", name: "Frostrim: The Elder Moods", path: "/mnt/games/SteamLibrary/steamapps/common/Frostrim: The Elder Moods", source: "steam", steamAppId: 9900002, protonPrefix: "/mnt/games/SteamLibrary/steamapps/compatdata/9900002/pfx", modFolder: "Data", activeProfile: "Winter evenings", freeSpace: 911_000_000_000, totalSpace: 2_000_000_000_000 },
  { domain: "stardudevalley", name: "Stardude Valley", path: "/home/alex/Games/stardude", source: "added", modFolder: "Mods", launch: "./StardudeLoader", freeSpace: 182_000_000_000, totalSpace: 1_000_000_000_000 },
];

const catalog: { mod: ModInfo; files: FileInfo[] }[] = [
  mod("frostrim", 9001, "Lanterns of the North", "Maya", "Warm lanterns along every road, with a light that follows the weather.", [
    ["Lanterns of the North", "MAIN", 48],
    ["Patch for snowy roads", "OPTIONAL", 3.2],
  ]),
  mod("frostrim", 9002, "Quiet Footsteps", "Jordan", "Softer footstep sounds on snow, stone and wood.", [["Quiet Footsteps", "MAIN", 12]]),
  mod("frostrim", 9003, "Pine Forest Textures", "Sam", "New 2K and 4K textures for pine trees and forest floors.", [
    ["4K textures", "MAIN", 1240],
    ["2K textures", "MAIN", 610],
  ]),
  mod("stardudevalley", 9101, "Bigger Barn Doors", "Alex", "Wider barn and coop doors, so animals stop getting stuck.", [["Bigger Barn Doors", "MAIN", 0.4]]),
  mod("cyberhunk2088", 9201, "Neon Sign Refresh", "Maya", "Sharper neon signs across the city.", [["Neon Sign Refresh", "MAIN", 320]]),
];

function mod(game: string, id: number, name: string, author: string, summary: string, files: [string, string, number][]) {
  return {
    mod: { modId: id, game, name, author, summary, version: "1.2.0", pictureUrl: placeholder(`${game}${id}`, name, "mod"), adult: false },
    files: files.map(([n, cat, mb], i) => ({
      fileId: id * 10 + i,
      name: n,
      fileName: `${n.replace(/\W+/g, "_")}-${id}-1-2-0.7z`,
      version: "1.2.0",
      category: cat,
      size: Math.round(mb * MB),
      uploaded: now() - 86400 * (i + 3),
      isPrimary: i === 0,
      description: null,
    })),
  };
}

let library: StoredMod[] = [
  stored(catalog[1], [0], 2),
  stored(catalog[3], [0], 5),
];

function stored(c: (typeof catalog)[number], idx: number[], daysAgo: number): StoredMod {
  const folder = `${settings.libraryDir}/${c.mod.game}/${c.mod.modId} - ${c.mod.name}`;
  return {
    game: c.mod.game,
    modId: c.mod.modId,
    name: c.mod.name,
    author: c.mod.author,
    version: c.mod.version,
    summary: c.mod.summary,
    pictureUrl: c.mod.pictureUrl,
    folder,
    files: idx.map((i) => {
      const f = c.files[i];
      return { fileId: f.fileId, name: f.name, fileName: f.fileName, version: f.version, category: f.category, size: f.size, downloadedAt: now() - daysAgo * 86400, archive: f.fileName, unpacked: settings.extract ? f.fileName.replace(/\.7z$/, "") : null };
    }),
  };
}

let jobs: Job[] = [];
const newJob = (c: (typeof catalog)[number], f: FileInfo, status: Job["status"] = "queued", received = 0): Job => ({
  id: `${c.mod.game}-${c.mod.modId}-${f.fileId}-${Math.random().toString(36).slice(2, 7)}`,
  game: c.mod.game,
  modId: c.mod.modId,
  fileId: f.fileId,
  modName: c.mod.name,
  fileName: f.fileName,
  name: f.name,
  version: f.version,
  size: f.size,
  received,
  status,
  addedAt: now(),
  speed: 0,
  retries: 0,
});
jobs = [
  newJob(catalog[0], catalog[0].files[0]),
  newJob(catalog[2], catalog[2].files[1]),
  newJob(catalog[0], catalog[0].files[1]),
  newJob(catalog[4], catalog[4].files[0]),
  { ...newJob(catalog[2], catalog[2].files[0], "needsLink"), hasLink: true, error: "The download link from Nexus Mods has run out. Click Mod manager download on the file's page again." },
];

/** The demo's account: Premium, or free with ?free in the address (to try both kinds). */
export const demoPremium = typeof location === "undefined" || !new URLSearchParams(location.search).has("free");

const running = (j: Job) => j.status === "starting" || j.status === "downloading" || j.status === "storing";

function tick() {
  const slots = settings.mode === "sequential" || (settings.mode === "auto" && !demoPremium) ? 1 : settings.maxConcurrent;
  let active = jobs.filter(running).length;
  for (const j of jobs) {
    if (active >= slots) break;
    if (j.status === "queued" && !demoPremium && !j.hasLink) {
      // A free account needs the website's click for each file.
      Object.assign(j, { status: "needsLink", error: "Nexus Mods only lets Premium members download straight from an app. Click Mod manager download on the file's page and Zom's Nexus picks it up from there." });
      emit("queue:job", { ...j });
      continue;
    }
    if (j.status === "queued") {
      j.status = "starting";
      active++;
      emit("queue:job", { ...j });
    }
  }
  for (const j of jobs) {
    if (j.status === "starting") j.status = "downloading";
    else if (j.status === "downloading") {
      // Shared pretend line speed: about 40 MB/s split between what's running.
      const speed = Math.round(((demoPremium ? 40 : 3) * MB) / Math.max(1, jobs.filter((x) => x.status === "downloading").length) * (0.85 + Math.random() * 0.3));
      j.speed = speed;
      j.received = Math.min(j.size, j.received + speed / 2);
      if (j.received >= j.size) {
        j.status = "storing";
        j.speed = 0;
      }
    } else if (j.status === "storing") {
      j.status = "done";
      j.finishedAt = now();
      const c = catalog.find((c) => c.mod.modId === j.modId)!;
      const fi = c.files.findIndex((f) => f.fileId === j.fileId);
      const have = library.find((m) => m.modId === j.modId);
      const s = stored(c, [fi], 0);
      if (have) have.files = [...have.files.filter((f) => f.fileId !== j.fileId), ...s.files];
      else library.unshift(s);
      j.path = s.folder;
      emit("library:changed", null);
    } else continue;
    emit("queue:job", { ...j });
  }
}
setInterval(tick, 500);

const all = () => emit("queue:all", jobs.map((j) => ({ ...j })));
const find = (id: unknown) => jobs.find((j) => j.id === id);

const overview = (): Overview => ({
  settings: { ...settings },
  hasKey,
  user: hasKey ? { userId: 1, name: "Alex", isPremium: demoPremium } : null,
  limits: { dailyRemaining: 19874, hourlyRemaining: 498 },
  version: __APP_VERSION__,
  freeSpace: 412_000_000_000,
  totalSpace: 1_000_000_000_000,
});

function lookup(address: string) {
  const m = address.match(/nexusmods\.com\/(?:games\/)?([\w-]+)\/mods\/(\d+)/);
  if (!m) throw new Error("Paste a mod page address from nexusmods.com.");
  const c = catalog.find((c) => c.mod.modId === Number(m[2])) ?? catalog[2];
  return c;
}

const notes: Record<string, string> = { "frostrim:9002": "Keep below Lanterns of the North, or the snow sounds come back." };

let profiles: Profile[] = [
  { id: "p1", name: "Winter evenings", game: "frostrim", mods: [9001, 9002, 9003].map((m) => ({ modId: m, fileId: m * 10, enabled: true })) },
  { id: "p2", name: "Farm life", game: "stardudevalley", mods: [{ modId: 9101, fileId: 91010, enabled: true }] },
];

const demoCheck: CheckReport = {
  conflicts: [{ winner: "Pine Forest Textures", losers: ["Lanterns of the North"], files: 3, examples: ["data/textures/landscape/trees/pine01.dds", "data/textures/landscape/trees/pine02.dds", "data/textures/landscape/snow.dds"] }],
  plugins: [{ plugin: "LanternsSnowPatch.esp", modName: "Lanterns of the North", master: "SnowyRoads.esm", kind: "missing" }],
  loadOrder: ["LanternsOfTheNorth.esp", "LanternsSnowPatch.esp", "QuietFootsteps.esp"],
  missing: [],
  installers: [],
  files: 214,
  at: now(),
  needs: [
    { name: "Snowy Roads", modId: 9004, kind: "notDownloaded", neededBy: ["Lanterns of the North"] },
    { name: "A shader tool", modId: 0, kind: "external", url: "https://example.com/shader-tool", notes: "Version 2 or newer", neededBy: ["Pine Forest Textures"] },
  ],
  compat: [{ modId: 9002, modName: "Quiet Footsteps", kind: "notUpdated", fileDate: now() - 90 * 86400, modUpdated: now() - 90 * 86400 }],
  requirementsChecked: true,
  programs: [{ modName: "Pine Forest Textures", files: ["Tools/TextureOptimizer.exe"], count: 1 }],
  gameUpdated: now() - 20 * 86400,
};

export async function call(cmd: string, a: Record<string, unknown> = {}): Promise<unknown> {
  await new Promise((r) => setTimeout(r, 60));
  switch (cmd) {
    case "get_overview":
    case "refresh_account":
      return overview();
    case "save_settings":
      settings = { ...(a.settings as Settings), games: settings.games };
      return overview();
    case "set_api_key":
      if (!String(a.key).trim()) throw new Error("Paste your API key from nexusmods.com > Settings > API keys.");
      hasKey = true;
      return overview();
    case "forget_api_key":
      hasKey = false;
      return overview();
    case "lookup_mod": {
      const c = lookup(String(a.address));
      return { mod: c.mod, files: c.files };
    }
    case "queue_list":
      return jobs.map((j) => ({ ...j }));
    case "queue_add_link": {
      const m = String(a.link).match(/^nxm:\/\/[\w-]+\/mods\/(\d+)\/files\/(\d+)/);
      if (!m) throw new Error("That isn't a Nexus Mods download link (it should start with nxm://).");
      const c = catalog.find((c) => c.mod.modId === Number(m[1])) ?? catalog[1];
      const j = newJob(c, c.files[0]);
      jobs.push(j);
      all();
      return j;
    }
    case "queue_add_files": {
      const c = lookup(String(a.address));
      const added = c.files.filter((f) => (a.fileIds as number[]).includes(f.fileId)).map((f) => newJob(c, f));
      jobs.push(...added);
      all();
      return added;
    }
    case "queue_pause": {
      const j = find(a.id);
      if (j && ["queued", "starting", "downloading", "waiting"].includes(j.status)) Object.assign(j, { status: "paused", speed: 0 });
      all();
      return;
    }
    case "queue_resume": {
      const j = find(a.id);
      if (j && ["paused", "failed", "needsLink", "waiting"].includes(j.status)) Object.assign(j, { status: "queued", error: null });
      all();
      return;
    }
    case "queue_remove":
      jobs = jobs.filter((j) => j.id !== a.id);
      all();
      return;
    case "queue_pause_all":
      jobs.forEach((j) => ["queued", "starting", "downloading", "waiting"].includes(j.status) && Object.assign(j, { status: "paused", speed: 0 }));
      all();
      return;
    case "queue_resume_all":
      jobs.forEach((j) => j.status === "paused" && (j.status = "queued"));
      all();
      return;
    case "queue_now": {
      const i = jobs.findIndex((j) => j.id === a.id);
      if (i >= 0) {
        const [j] = jobs.splice(i, 1);
        if (["paused", "failed", "waiting"].includes(j.status) || (j.status === "needsLink" && j.hasLink)) Object.assign(j, { status: "queued", error: null });
        jobs.unshift(j);
      }
      all();
      return;
    }
    case "queue_sort": {
      const movable = (s: string) => ["queued", "paused", "waiting", "needsLink", "failed"].includes(s);
      const at = jobs.map((j, i) => (movable(j.status) ? i : -1)).filter((i) => i >= 0);
      const sorted = at.map((i) => jobs[i]).sort((x, y) => (x.size === 0 ? 1 : 0) - (y.size === 0 ? 1 : 0) || (a.smallestFirst ? x.size - y.size : y.size - x.size));
      at.forEach((i, k) => (jobs[i] = sorted[k]));
      all();
      return;
    }
    case "queue_retry_failed":
      jobs.forEach((j) => j.status === "failed" && Object.assign(j, { status: "queued", error: null }));
      all();
      return;
    case "mod_requirements":
      return (a.modIds as number[]).map((id) => ({
        modId: id,
        game: String(a.game),
        name: catalog.find((c) => c.mod.modId === id)?.mod.name ?? `Mod ${id}`,
        status: "published",
        updatedAt: now() - 30 * 86400,
        requires: id === 9003 ? [{ modId: 9001, name: "Lanterns of the North", otherGame: false, notes: "Min version 1.2" }, { modId: 9004, name: "Snowy Roads", otherGame: false, notes: null }, { modId: 0, name: "A shader tool", url: "https://example.com/shader-tool", otherGame: false, notes: "Version 2 or newer" }] : [],
        dlc: [],
      }));
    case "queue_requirements":
      return { queued: (a.modIds as number[]).map((id) => (id === 9004 ? "Snowy Roads" : `Mod ${id}`)), skipped: [] };
    case "profile_resolve": {
      const p = profiles.find((x) => x.id === a.id)!;
      return { profile: structuredClone(p), added: [], switchedOn: [], moved: 0, missing: p.game === "frostrim" ? [[9004, "Snowy Roads"]] : [] };
    }
    case "profile_updates":
      return { updates: [], checked: 3, problems: [], at: now() };
    case "mod_notes":
      return { ...notes };
    case "set_mod_note":
      if (String(a.note).trim()) notes[`${a.game}:${a.modId}`] = String(a.note).trim();
      else delete notes[`${a.game}:${a.modId}`];
      return;
    case "diagnostics":
      return;
    case "queue_clear_finished":
      jobs = jobs.filter((j) => j.status !== "done");
      all();
      return;
    case "queue_reorder": {
      const i = jobs.findIndex((j) => j.id === a.id);
      if (i >= 0) {
        const [j] = jobs.splice(i, 1);
        jobs.splice(Math.min(Number(a.to), jobs.length), 0, j);
      }
      all();
      return;
    }
    case "library_list":
      return structuredClone(library);
    case "library_unpack":
      library.forEach((m) => m.files.forEach((f) => f.fileId === a.fileId && (f.unpacked = f.fileName.replace(/\.7z$/, ""))));
      return;
    case "library_remove_file":
      library.forEach((m) => (m.files = m.files.filter((f) => f.fileId !== a.fileId)));
      library = library.filter((m) => m.files.length);
      return;
    case "library_remove_mod":
      library = library.filter((m) => m.modId !== a.modId);
      return;
    case "library_usage":
      return library.flatMap((m) => m.files.map((f) => ({ game: m.game, modId: m.modId, fileId: f.fileId, archive: f.archive ? f.size : 0, unpacked: f.unpacked ? Math.round(f.size * 1.6) : 0 })));
    case "library_drop_archive": {
      let freed = 0;
      library.forEach((m) => m.files.forEach((f) => f.fileId === a.fileId && f.unpacked && f.archive && ((freed = f.size), (f.archive = null))));
      return freed;
    }
    case "library_import": {
      const file = String(a.path).split("/").pop() ?? "mod.zip";
      const id = 1_000_000_000_000 + now();
      const m: StoredMod = {
        game: String(a.game),
        modId: id,
        name: file.replace(/\.[^.]+$/, "").replace(/[_-]+/g, " "),
        files: [{ fileId: id, name: file, fileName: file, size: 2 * MB, downloadedAt: now(), archive: file, unpacked: settings.extract ? file.replace(/\.[^.]+$/, "") : null }],
        local: true,
        folder: "",
      };
      library.unshift(m);
      return structuredClone(m);
    }
    case "open_folder":
      return;
    case "games_list":
      return structuredClone(games);
    case "steam_libraries":
      return [
        { path: "/home/alex/.local/share/Steam", installed: 6, nexusGames: ["Cyberhunk 2088"], freeSpace: 182_000_000_000, totalSpace: 1_000_000_000_000 },
        { path: "/mnt/games/SteamLibrary", installed: 41, nexusGames: ["Frostrim: The Elder Moods"], freeSpace: 911_000_000_000, totalSpace: 2_000_000_000_000 },
      ];
    case "games_add": {
      const domain = String(a.domain).toLowerCase();
      if (!/^[\w-]+$/.test(domain)) throw new Error("Type the game's name as it's written in its Nexus Mods address, like frostrim.");
      games.push({ domain, name: String(a.name || domain), path: (a.path as string) || null, source: "added" });
      return;
    }
    case "games_remove": {
      const i = games.findIndex((g) => g.domain === a.domain && g.source === "added");
      if (i >= 0) games.splice(i, 1);
      return;
    }
    case "nxm_status":
      return { ours: linksOurs, other: linksOurs ? null : "other-mod-manager.desktop" } satisfies LinkStatus;
    case "game_art": {
      // Real pictures dropped into public/demo-art/ win; the drawn ones fill in the rest.
      const found = await demoArt(`${a.domain}-${a.kind}`);
      if (found) return found;
      const g = games.find((x) => x.domain === a.domain);
      return placeholder(String(a.domain), g?.name ?? String(a.domain), a.kind as "cover" | "hero" | "logo");
    }
    case "system_check":
      return {
        distro: "Demo Linux",
        items: [
          { what: "Library folder", ok: true, detail: "Mods are kept in /home/alex/Games/Nexus Mods." },
          { what: "Steam", ok: true, detail: "2 Steam library folders searched for games." },
          { what: "Nexus Mods links", ok: linksOurs, detail: linksOurs ? "The website's Mod manager download button opens Zom's Nexus." : "Not set up yet: Settings > Nexus Mods links > Open them with Zom's Nexus." },
          { what: "Tray icon", ok: true, detail: "Closing the window while downloading keeps it going in the tray." },
          { what: "Notifications", ok: true, detail: "Tells you when downloads finish in the background." },
          { what: "Rar files", ok: false, detail: "Mods packed as .rar can't be unpacked until bsdtar or unrar is installed (zip and 7z always can).", fix: "sudo apt install libarchive-tools" },
        ],
      };
    case "game_launch":
      return;
    case "library_check_updates":
    case "library_updates":
      return {
        updates: [{ game: "frostrim", modId: 9002, modName: "Quiet Footsteps", fileId: 90020, newer: { ...catalog[1].files[0], fileId: 90021, name: "Quiet Footsteps", version: "1.3.0" }, outdated: false }],
        checked: library.length,
        problems: [],
        at: now(),
      };
    case "queue_add": {
      const c = catalog.find((c) => c.mod.modId === a.modId) ?? catalog[0];
      const j = newJob(c, c.files[0]);
      jobs.push(j);
      all();
      return j;
    }
    case "browse":
      return catalog.filter((c) => c.mod.game === a.game).map((c, i) => ({ ...c.mod, endorsements: 1200 - i * 310, downloads: 48000 - i * 9000, updated: now() - i * 86400 }));
    case "mod_files": {
      const c = catalog.find((c) => c.mod.modId === a.modId) ?? catalog[0];
      return { mod: c.mod, files: c.files };
    }
    case "lookup_collection":
      if (!String(a.address).includes("/collections/")) throw new Error("That address isn't a collection.");
      return {
        game: "frostrim",
        slug: "snowy1",
        revision: 4,
        name: "Snowy North",
        author: "Maya",
        summary: "A quiet winter look: lanterns, pines and softer footsteps.",
        files: catalog.filter((c) => c.mod.game === "frostrim").flatMap((c) => c.files.slice(0, 1).map((f, i) => ({ game: c.mod.game, modId: c.mod.modId, fileId: f.fileId, modName: c.mod.name, name: f.name, version: f.version, size: f.size, optional: c.mod.modId === 9002 && i === 0 }))),
      };
    case "queue_add_collection": {
      const p: Profile = { id: `p${Date.now()}`, name: "Snowy North (revision 4)", game: "frostrim", mods: [9001, 9003].map((m) => ({ modId: m, fileId: m * 10, enabled: true })), collection: "snowy1" };
      profiles.push(p);
      catalog.filter((c) => c.mod.game === "frostrim").forEach((c) => jobs.push(newJob(c, c.files[0])));
      all();
      return { queued: 3, skipped: 0, profile: p };
    }
    case "profiles_list":
      return structuredClone(profiles);
    case "profile_save": {
      const p = { ...(a.profile as Profile) };
      if (!p.name.trim()) throw new Error("Give the profile a name.");
      if (!p.id) p.id = `p${Date.now()}`;
      profiles = [...profiles.filter((x) => x.id !== p.id), p];
      return p;
    }
    case "profile_delete":
      profiles = profiles.filter((p) => p.id !== a.id);
      return;
    case "profile_check":
      return { ...demoCheck, at: now() };
    case "profile_checks":
      return {};
    case "profile_apply":
    case "profile_play": {
      const p = profiles.find((p) => p.id === a.id)!;
      const g = games.find((g) => g.domain === p.game);
      if (g) g.activeProfile = p.name;
      return { files: 214, mods: p.mods.length, backedUp: 2, plugins: 3, installers: [], missing: [], clashes: 3 };
    }
    case "profile_clear": {
      const g = games.find((g) => g.domain === a.game);
      if (g) g.activeProfile = null;
      return;
    }
    case "profile_export":
    case "profile_import":
      throw new Error("Mod packs are saved as files, so they only work in the desktop app.");
    case "nxm_register":
      linksOurs = true;
      return { ours: true } satisfies LinkStatus;
  }
  throw new Error(`The demo doesn't do ${cmd}.`);
}
