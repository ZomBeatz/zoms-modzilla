# Zom's Nexus for Decky Loader

A [Decky Loader](https://decky.xyz) plugin that puts your Zom's Nexus profiles in the Steam
Deck's quick access menu (the **…** button): switch a game's profile, or take its mods out,
without leaving Game Mode. Then start the game from your library as usual.

It only reads Zom's Nexus' profile list. Switching is done by Zom's Nexus itself
(`zoms-nexus --profile <id>` or `--no-mods <game>`), so the same checks run as in the app.

## Build

```bash
npm install
npm run build
```

Copy `plugin.json`, `main.py`, `package.json` and `dist/` into
`~/homebrew/plugins/zoms-nexus/` on the Deck, then restart Decky (or use Decky's developer
mode to install the folder as a zip).

Open Zom's Nexus once on the Deck first, so the plugin knows how to start it.
