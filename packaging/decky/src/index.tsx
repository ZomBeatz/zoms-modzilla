import { ButtonItem, PanelSection, PanelSectionRow, staticClasses } from "@decky/ui";
import { callable, definePlugin, toaster } from "@decky/api";
import { useEffect, useState } from "react";
import { FaCubes } from "react-icons/fa";

type Game = { game: string; active: string | null; profiles: { id: string; name: string; mods: number }[] };

const profiles = callable<[], Game[]>("profiles");
const use = callable<[id: string], string>("use");
const noMods = callable<[game: string], string>("no_mods");

const nice = (g: string) => g.replace(/([a-z])([0-9])/g, "$1 $2").replace(/^./, (c) => c.toUpperCase());

function Content() {
  const [games, setGames] = useState<Game[] | null>(null);
  const load = () => void profiles().then(setGames).catch(() => setGames([]));
  useEffect(load, []);
  const done = (msg: string, ok: string) => {
    toaster.toast({ title: "Zom's Nexus", body: msg || ok });
    setTimeout(load, 4000);
  };
  if (!games) return <PanelSection title="Loading…" />;
  if (!games.length)
    return (
      <PanelSection>
        <PanelSectionRow>No profiles yet. Make one in Zom's Nexus.</PanelSectionRow>
      </PanelSection>
    );
  return (
    <>
      {games.map((g) => (
        <PanelSection key={g.game} title={nice(g.game)}>
          {g.profiles.map((p) => (
            <PanelSectionRow key={p.id}>
              <ButtonItem layout="below" disabled={g.active === p.id} onClick={() => void use(p.id).then((m) => done(m, `Putting ${p.name} in…`))}>
                {g.active === p.id ? `✓ ${p.name}` : p.name} · {p.mods} mods
              </ButtonItem>
            </PanelSectionRow>
          ))}
          <PanelSectionRow>
            <ButtonItem layout="below" disabled={!g.active} onClick={() => void noMods(g.game).then((m) => done(m, "Taking the mods out…"))}>
              Play without mods
            </ButtonItem>
          </PanelSectionRow>
        </PanelSection>
      ))}
    </>
  );
}

export default definePlugin(() => ({
  name: "Zom's Nexus",
  titleView: <div className={staticClasses.Title}>Zom's Nexus</div>,
  content: <Content />,
  icon: <FaCubes />,
}));
