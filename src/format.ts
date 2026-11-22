// Sizes, speeds, times and plurals in the ZomBeatz way: decimal units with a space, at most 3
// significant digits ("1.92 GB", "512 MB", "12.4 MB"), dates in the person's own format.

const UNITS = ["B", "kB", "MB", "GB", "TB"];

export function bytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  let i = 0;
  let v = n;
  while (v >= 999.5 && i < UNITS.length - 1) {
    v /= 1000;
    i++;
  }
  const digits = i === 0 || v >= 99.95 ? 0 : v >= 9.995 ? 1 : 2;
  return `${v.toFixed(digits)} ${UNITS[i]}`;
}

export const speed = (n: number) => `${bytes(n)}/s`;

/** Time left at this speed: "12 s", "4 min", "1 h 5 min". */
export function eta(left: number, perSecond: number): string {
  if (perSecond <= 0 || left <= 0) return "";
  const s = Math.ceil(left / perSecond);
  if (s < 60) return `${s} s`;
  const m = Math.round(s / 60);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  return m % 60 ? `${h} h ${m % 60} min` : `${h} h`;
}

export const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** "Today 3:30 PM", "Oct 28", "Oct 28, 2025", in the person's own format. */
export function when(unix: number, today = new Date()): string {
  const d = new Date(unix * 1000);
  const time = d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  const sameDay = (a: Date, b: Date) => a.toDateString() === b.toDateString();
  if (sameDay(d, today)) return `Today ${time}`;
  const y = new Date(today);
  y.setDate(y.getDate() - 1);
  if (sameDay(d, y)) return `Yesterday ${time}`;
  return d.toLocaleDateString(undefined, d.getFullYear() === today.getFullYear() ? { month: "short", day: "numeric" } : { month: "short", day: "numeric", year: "numeric" });
}

export const percent = (part: number, whole: number) => (whole > 0 ? Math.min(100, Math.floor((part / whole) * 100)) : 0);
