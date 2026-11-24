// The requests left with Nexus Mods, and a guide to what uses them.
//
// Nexus Mods gives each account a daily allowance of API requests. Only once that's used up does
// an hourly allowance apply. So the daily number is the one that matters; the hourly one is shown
// only when the day's are gone.

import { useState } from "react";
import { Info } from "lucide-react";
import type { Overview } from "./api";
import { Dialog } from "./ui";

const USES: [string, string][] = [
  ["Signing in", "1 when the app starts, and 1 for Check again in Settings."],
  ["Each download", "Up to 3 when it's added or starts: the file's details, the mod's details the first time, and the download link. A checksum check adds 1 when it finishes (Settings > Downloads)."],
  ["Download links (free accounts)", "None: the Slow download button on the website gives the link."],
  ["Browse", "1 per list (Trending, New, Just updated) per game, kept for 10 minutes."],
  ["Add mod…", "2 to look up a mod's address (its details and its files). 1 to look up a collection."],
  ["Requirements and name lookups", "1 for up to 40 mods at once, kept for a day. The automatic conflict check only asks about mods it hasn't seen today."],
  ["Download the missing mods", "1 per mod, for its file list, only when you click it."],
  ["Check for updates (Library)", "1 per mod in the library. Update this profile: 1 per mod in the profile."],
  ["Game covers", "Only for games Steam has no art for: 1 per game, once, then kept."],
  ["Add a game", "1 for the list of every game on Nexus Mods, kept for a week."],
  ["Everything else", "None: profiles, Play, the conflict check (apart from requirements), Review space, the library and settings work on your computer only."],
];

export function Requests({ limits }: { limits: Overview["limits"] }) {
  const [open, setOpen] = useState(false);
  const day = limits.dailyRemaining;
  const hour = limits.hourlyRemaining;
  if (day == null && hour == null) return null;
  const dayGone = day === 0;
  const low = dayGone ? (hour ?? 0) < 20 : (day ?? 0) < 500;
  return (
    <>
      <button className={`link-name req-line${low ? " warn-text" : ""}`} title="What uses requests, and what the numbers mean" onClick={() => setOpen(true)}>
        {dayGone ? `${(hour ?? 0).toLocaleString()} requests left this hour (today's are used up)` : `${(day ?? 0).toLocaleString()} requests left today`}
        <Info size={11} />
      </button>
      {open && (
        <Dialog title="Nexus Mods requests" onClose={() => setOpen(false)}>
          <p style={{ marginTop: 0 }}>
            Zom's Modzilla asks Nexus Mods for mod details, file lists and download links through their API, with your key. Nexus Mods gives each account a <b>daily allowance</b> of these requests. Once the day's are used up, a smaller <b>hourly allowance</b> applies until the day resets.
          </p>
          <p className="muted small">
            Right now: {day != null ? `${day.toLocaleString()} left today` : "today's count isn't known yet"}
            {hour != null ? `, ${hour.toLocaleString()} for the hour after that` : ""}. The numbers come with every answer from Nexus Mods, so they're always current, and checking them costs nothing.
          </p>
          <h3 style={{ marginBottom: 6 }}>What uses them</h3>
          <table className="table req-table">
            <tbody>
              {USES.map(([what, cost]) => (
                <tr key={what}>
                  <td>
                    <b>{what}</b>
                  </td>
                  <td className="muted">{cost}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="muted small">
            When they run out, nothing breaks: downloads wait and carry on by themselves when requests are available again, and pages show what they already have. The downloading itself (the file's bytes) never counts.
          </p>
        </Dialog>
      )}
    </>
  );
}
