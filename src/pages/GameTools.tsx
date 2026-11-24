import { useEffect, useState } from "react";
import { Check, Play, Plus, Trash2, Wrench } from "lucide-react";
import { api, isDesktop, modUrl, on, pickFile, type Extender, type Game, type LoaderOffer, type PrefixStatus, type Profile, type Tool, type ToolDone } from "../api";
import { Dialog } from "../ui";

/** Loaders on Nexus Mods worth having for a game: (mod number, name, what it's for). */
const NEXUS_LOADERS: Record<string, [number, string, string][]> = {
  cyberpunk2077: [
    [107, "Cyber Engine Tweaks", "Lua mods and the in-game mod menu"],
    [1511, "redscript", "Script mods"],
    [2380, "RED4ext", "Plugin mods"],
    [4198, "ArchiveXL", "New items and clothes"],
    [4197, "TweakXL", "Tweak mods"],
  ],
  skyrimspecialedition: [
    [30379, "Skyrim Script Extender (SKSE64)", "Needed by most big mods"],
    [32444, "Address Library for SKSE Plugins", "Needed by most SKSE plugins"],
  ],
  fallout4: [[42147, "Fallout 4 Script Extender (F4SE)", "Needed by most big mods"]],
};

const viaName = (v: PrefixStatus["via"]) => (v === "protontricks" ? "Protontricks" : v === "protontricksFlatpak" ? "Protontricks (Flatpak)" : v === "winetricks" ? "winetricks" : "");

/** Game tools: Steam starting the script extender, what the Proton prefix has, modding tools
 * run in the prefix, and one-click loaders. */
export function GameTools({ game, onClose, toast, openAdd }: { game: Game; onClose: () => void; toast: (t: string) => void; openAdd: (address?: string) => void }) {
  const [ext, setExt] = useState<Extender | null>(null);
  const [pfx, setPfx] = useState<PrefixStatus | null>(null);
  const [tools, setTools] = useState<Tool[]>([]);
  const [offers, setOffers] = useState<LoaderOffer[]>([]);
  const [busy, setBusy] = useState("");
  const [said, setSaid] = useState("");
  const load = () => {
    void api.extender(game.domain).then(setExt, () => setExt(null));
    void api.prefixStatus(game.domain).then(setPfx, () => setPfx(null));
    void api.tools(game.domain).then(setTools, () => {});
    void api.loaderOffers(game.domain).then(setOffers, () => {});
  };
  useEffect(load, [game.domain]); // eslint-disable-line react-hooks/exhaustive-deps
  const run = async (what: string, f: () => Promise<unknown>) => {
    setBusy(what);
    try {
      await f();
    } catch (e) {
      toast((e as Error).message);
    } finally {
      setBusy("");
    }
  };
  const nexus = NEXUS_LOADERS[game.domain] ?? [];
  const proton = !!game.protonPrefix;
  return (
    <Dialog title={`${game.name}: game tools`} onClose={onClose} wide foot={<button className="btn" onClick={onClose}>Close</button>}>
      {ext && (
        <section className="tools-sec">
          <h3>Start {ext.name} from Steam</h3>
          <p className="muted small">
            Steam's Play button (and Game Mode on a Steam Deck) starts {ext.launcher}, so {ext.name} never runs. With this on, putting a profile in also puts {ext.loader} in the launcher's place. The launcher is kept aside and comes back with Take mods out, or when you switch this off.
          </p>
          <label className="row gap-sm">
            <input type="checkbox" checked={ext.on} disabled={!!busy} onChange={(e) => void run("ext", async () => setExt(await api.setExtender(game.domain, e.target.checked)))} />
            Steam's Play starts {ext.name}
          </label>
          <p className="muted small">
            {ext.swapped ? `On: Play in Steam starts ${ext.name} now.` : ext.on && !ext.installed ? `${ext.name} isn't in the game folder or a mod of the profile yet: add it below, then Play.` : ext.on ? "It takes effect the next time a profile goes in." : ""}
          </p>
        </section>
      )}

      {proton && (
        <section className="tools-sec">
          <h3>Windows parts in the prefix</h3>
          <p className="muted small">Some mods and tools need Visual C++, .NET or DirectX parts in the game's Proton prefix. Each one takes a minute or two (.NET longer); close the game first.</p>
          {pfx && !pfx.via && (
            <p className="banner warn small">
              {pfx.steam ? (
                <>
                  These install with <b>Protontricks</b>. {pfx.deck ? "On a Steam Deck: switch to Desktop Mode, open Discover, search “Protontricks” and install it." : "Install it from your app store (Flathub: Protontricks) or your distro's packages."} Then open this again.
                </>
              ) : (
                <>This game isn't in Steam, so these install with winetricks: install it from your distro's packages, then open this again.</>
              )}
            </p>
          )}
          {pfx?.via && <p className="muted small">Using {viaName(pfx.via)}.</p>}
          {pfx?.suggested.length ? <p className="banner small">A loader's log says something's missing: {pfx.suggested.join(", ")}.</p> : null}
          <div className="tools-list">
            {(pfx?.verbs ?? []).map((v) => {
              const have = pfx!.installed.includes(v.verb);
              const want = pfx!.suggested.includes(v.verb);
              return (
                <div key={v.verb} className={`tools-row${want ? " want" : ""}`}>
                  <div className="grow">
                    <b>{v.name}</b> <span className="muted small">{v.wantedBy}</span>
                  </div>
                  {have ? (
                    <span className="pill">
                      <Check size={12} /> Installed
                    </span>
                  ) : (
                    <button
                      className={`btn sm${want ? " primary" : ""}`}
                      disabled={!pfx!.via || !pfx!.hasPrefix || !!busy}
                      title={pfx!.hasPrefix ? "" : "Start the game once first, so it has a prefix"}
                      onClick={() =>
                        void run(v.verb, async () => {
                          const out = await api.prefixInstall(game.domain, v.verb);
                          setSaid(out);
                          toast(`${v.name} is installed in ${game.name}'s prefix.`);
                          load();
                        })
                      }
                    >
                      {busy === v.verb ? "Installing…" : "Install"}
                    </button>
                  )}
                </div>
              );
            })}
          </div>
          {said && <pre className="tools-out">{said}</pre>}
        </section>
      )}

      {proton && (
        <section className="tools-sec">
          <h3>Modding tools</h3>
          <p className="muted small">
            Run xEdit, BodySlide, Nemesis, Pandora, DynDOLOD and other Windows tools in the game's prefix: they see the mods of the profile that's in the game, like on Windows. New files a tool makes can be kept as a mod of your own.
            {!game.activeProfile && " Put a profile in first (Play, or Put mods in game on the Profiles page)."}
          </p>
          <div className="tools-list">
            {tools.map((t) => (
              <div key={t.id} className="tools-row">
                <Wrench size={14} />
                <div className="grow">
                  <b>{t.name}</b> <span className="muted small ellipsis">{t.exe}</span>
                </div>
                <button className="btn sm primary" disabled={!!busy} onClick={() => void run(t.id, async () => (await api.runTool(t.id), toast(`${t.name} is starting in ${game.name}'s prefix…`)))}>
                  <Play size={13} /> Run
                </button>
                <button className="icon-btn danger" title="Take off the list" aria-label={`Take ${t.name} off the list`} onClick={() => void run("del", async () => (await api.deleteTool(t.id), load()))}>
                  <Trash2 size={14} />
                </button>
              </div>
            ))}
          </div>
          <button
            className="btn sm"
            onClick={() =>
              void run("add", async () => {
                const exe = isDesktop ? await pickFile("Pick the tool's .exe", "exe") : "/home/alex/Tools/Nemesis/Nemesis Unlimited Behavior Engine.exe";
                if (!exe) return;
                await api.saveTool({ id: "", game: game.domain, name: "", exe, args: [] });
                load();
              })
            }
          >
            <Plus size={13} /> Add a tool
          </button>
        </section>
      )}

      {(offers.length > 0 || nexus.length > 0) && (
        <section className="tools-sec">
          <h3>Loaders</h3>
          <p className="muted small">What many mods for this game need. Each one becomes a mod in your library: add it to a profile like any other.</p>
          <div className="tools-list">
            {nexus.map(([id, name, what]) => (
              <div key={id} className="tools-row">
                <div className="grow">
                  <b>{name}</b> <span className="muted small">{what} · Nexus Mods</span>
                </div>
                <button className="btn sm" onClick={() => openAdd(modUrl(game.domain, id))}>
                  Get
                </button>
              </div>
            ))}
            {offers.map((o) => (
              <div key={o.id} className="tools-row">
                <div className="grow">
                  <b>{o.name}</b> <span className="muted small">{o.what}</span>
                </div>
                <button
                  className="btn sm"
                  disabled={!!busy}
                  onClick={() =>
                    void run(o.id, async () => {
                      const m = await api.installLoader(game.domain, o.id);
                      toast(`${m.name} is in the library. Add it to a profile${proton ? `; under Proton its launch options need WINEDLLOVERRIDES="${o.dll}=n,b" %command% (the conflict check says so too)` : ""}.`);
                    })
                  }
                >
                  {busy === o.id ? "Getting…" : "Get"}
                </button>
              </div>
            ))}
          </div>
        </section>
      )}
      {!ext && !proton && !offers.length && !nexus.length && <p className="muted">Nothing here for this game: it doesn't run through Proton and has no known loaders.</p>}
    </Dialog>
  );
}

/** After a modding tool closes: keep the new files it made as a mod of your own. */
export function ToolOutput({ toast }: { toast: (t: string) => void }) {
  const [done, setDone] = useState<ToolDone | null>(null);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [name, setName] = useState("");
  const [into, setInto] = useState("");
  useEffect(() => {
    const stop = on<ToolDone>("tool:done", (d) => {
      if (!d.made.length) return toast(`${d.name} closed. It didn't make any new files.`);
      setDone(d);
      setName(`${d.name} output`);
      void api.profiles().then((all) => {
        setProfiles(all);
        setInto(all.find((p) => p.game === d.game && p.name === d.game)?.id ?? all.find((p) => p.game === d.game)?.id ?? "");
      });
    });
    return () => void stop.then((f) => f());
  }, [toast]);
  if (!done) return null;
  const mine = profiles.filter((p) => p.game === done.game);
  return (
    <Dialog
      title={`${done.name} made ${done.made.length} new file${done.made.length === 1 ? "" : "s"}`}
      onClose={() => setDone(null)}
      foot={
        <>
          <button className="btn ghost" onClick={() => setDone(null)}>
            Leave them in the game
          </button>
          <button
            className="btn primary"
            onClick={() =>
              void api.keepToolFiles(done.game, name, into || null).then(
                (m) => {
                  toast(`Kept as “${m.name}”${into ? ", at the end of the profile" : ""}.`);
                  setDone(null);
                },
                (e) => toast((e as Error).message),
              )
            }
          >
            Keep as a mod
          </button>
        </>
      }
    >
      <p className="muted small">Kept as a mod, they move into your library and go in and out with a profile, instead of staying loose in the game folder. Files that were already there are never touched.</p>
      <ul className="tools-made">
        {done.made.slice(0, 12).map((f) => (
          <li key={f}>{f}</li>
        ))}
        {done.made.length > 12 && <li className="muted">and {done.made.length - 12} more</li>}
      </ul>
      <label className="field">
        Name
        <input value={name} onChange={(e) => setName(e.target.value)} />
      </label>
      <label className="field">
        Add to
        <select value={into} onChange={(e) => setInto(e.target.value)}>
          <option value="">just the library</option>
          {mine.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </label>
    </Dialog>
  );
}
