import { useState, type ReactNode } from "react";
import type { Ctx } from "../App";
import { PhoneSendButton } from "../PhoneSend";

/** The guides, also named in the AI help's instructions (ai.rs) so it can point to them. */
export const GUIDES: { id: string; title: string; body: (go: Ctx["go"]) => ReactNode }[] = [
  {
    id: "deck-start",
    title: "Getting started on a Steam Deck",
    body: (go) => (
      <>
        <ol>
          <li>
            In Desktop Mode, open Zom's Nexus once and press <b>Add to Steam</b> (Settings › Steam Deck). From then on it's in your library in Game Mode, with its artwork.
          </li>
          <li>Sign in: <b>Sign in with your phone</b> scans a QR code, so you paste your API key on your phone instead of typing it.</li>
          <li>Pick a game, get mods (Browse, or a collection), and make a profile. <b>Play</b> puts the profile's mods in and starts the game.</li>
          <li>Games on the SD card: keep their mods on the card too (Games › Keep its mods on its drive), so they're linked, not copied.</li>
        </ol>
        <button className="btn sm" onClick={() => go("settings")}>
          Open Settings
        </button>
      </>
    ),
  },
  {
    id: "controls",
    title: "Controller, back buttons and the trackpads",
    body: () => (
      <>
        <p>
          The D-pad or left stick moves between buttons, <b>A</b> presses, <b>B</b> goes back, <b>LB/RB</b> change page, <b>Y</b> plays, <b>X</b> opens a thing's menu, and <b>Menu</b> finds anything.
        </p>
        <p>
          In Game Mode, press the controller button on Zom's Nexus' page in your library and pick the <b>Gamepad with Mouse Trackpad</b> layout: the right trackpad then works as a mouse. The back buttons can be set to keys there (Edit layout › Back grips). Zom's Nexus listens for these:
        </p>
        <ul>
          <li>
            <b>F1</b> this help · <b>F2</b> find anything · <b>F3</b> Downloads · <b>F4</b> send from your phone
          </li>
        </ul>
      </>
    ),
  },
  {
    id: "typing",
    title: "Typing in Game Mode",
    body: () => (
      <>
        <p>
          Choosing a text box with <b>A</b>, or tapping it, opens Steam's keyboard (<b>STEAM + X</b> opens it any time). For anything long, like your API key or a download link, use your phone:
        </p>
        <PhoneSendButton />
      </>
    ),
  },
  {
    id: "not-showing",
    title: "My mods don't show up in the game",
    body: (go) => (
      <ul>
        <li>Press <b>Play</b> in Zom's Nexus (or Put mods in game on the profile) before starting from Steam: that's when mods go in.</li>
        <li>
          Run <b>Check for conflicts</b> on the profile: it finds mods packed a folder too deep, missing requirements and plugins, and loaders that need a Steam launch option under Proton.
        </li>
        <li>Script extender mods (SKSE, F4SE…) need the extender started: see the next guide.</li>
        <li>
          <button className="btn sm ghost" onClick={() => go("profiles")}>
            Open Profiles
          </button>
        </li>
      </ul>
    ),
  },
  {
    id: "extenders",
    title: "SKSE, F4SE and other script extenders from Game Mode",
    body: (go) => (
      <>
        <p>
          Steam starts the game's launcher, never the script extender. On the Games page, open the game's <b>Game tools</b> (the wrench) and switch on <b>Steam's Play starts SKSE</b>. Putting a profile in then puts the extender's loader where the launcher was; the launcher is kept aside and comes back with Take mods out.
        </p>
        <button className="btn sm" onClick={() => go("games")}>
          Open Games
        </button>
      </>
    ),
  },
  {
    id: "prefix",
    title: "A mod or tool needs Visual C++, .NET or DirectX",
    body: () => (
      <p>
        Open the game's <b>Game tools</b> on the Games page: <b>Windows parts in the prefix</b> installs them with Protontricks (on a Deck, install Protontricks from Discover in Desktop Mode first). When a loader's log says a part is missing, it's marked there.
      </p>
    ),
  },
  {
    id: "tools",
    title: "xEdit, BodySlide, Nemesis, Pandora and DynDOLOD",
    body: () => (
      <p>
        Add them under <b>Modding tools</b> in the game's Game tools and press Run: they start in the game's prefix and see the mods of the profile that's in. When a tool makes new files (Nemesis' behaviours, DynDOLOD's output), Zom's Nexus offers to keep them as a mod of your own, so they go in and out with the profile.
      </p>
    ),
  },
  {
    id: "cloud",
    title: "Steam Cloud keeps undoing my saves",
    body: () => (
      <p>
        A profile that keeps its own saves empties the save folder for the others, and Steam Cloud fills it again. In Steam, turn off Steam Cloud for that game (Properties › General), or let the profile share the usual saves. <b>Save snapshots</b> keep copies either way.
      </p>
    ),
  },
  {
    id: "sd",
    title: "Mods and the SD card",
    body: () => (
      <p>
        Mods are linked into a game when the library is on the same drive, which takes no extra space. A game on the SD card with the library on the internal drive gets copies instead: twice the space, and slower. On the Games page, <b>Keep its mods on its drive</b> moves that game's mods to the card.
      </p>
    ),
  },
  {
    id: "battery",
    title: "Battery and sleep",
    body: () => (
      <p>
        Downloads interrupted by sleep start again on waking, from where they stopped. In Settings › Downloads you can make new downloads wait below a battery level, and leave unpacking for when you're on the charger.
      </p>
    ),
  },
  {
    id: "offline",
    title: "Playing offline",
    body: () => <p>Without a connection your library, profiles and Play all work, with the art already seen. Browse, update checks and downloads wait until you're back online.</p>,
  },
  {
    id: "decky",
    title: "Profiles from the quick access menu (Decky Loader)",
    body: () => (
      <p>
        With Decky Loader, the Zom's Nexus plugin lists your profiles in the <b>…</b> menu: switch a game's profile or take its mods out without leaving Game Mode. It's in the app's source under <code>packaging/decky</code>.
      </p>
    ),
  },
];

export function Guide({ go }: Ctx) {
  const [open, setOpen] = useState<string>(GUIDES[0].id);
  return (
    <div className="page">
      <div className="page-head">
        <h1>Help</h1>
      </div>
      <p className="lead">Short guides for the questions people ask most, with the Steam Deck in mind.</p>
      <div className="guide">
        {GUIDES.map((g) => (
          <section key={g.id} className={`panel guide-item${open === g.id ? " open" : ""}`}>
            <button className="guide-title" aria-expanded={open === g.id} onClick={() => setOpen(open === g.id ? "" : g.id)}>
              {g.title}
            </button>
            {open === g.id && <div className="guide-body">{g.body(go)}</div>}
          </section>
        ))}
      </div>
    </div>
  );
}
