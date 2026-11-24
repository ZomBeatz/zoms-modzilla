# Zom's Modzilla for Decky Loader

A [Decky Loader](https://decky.xyz) plugin that puts your Zom's Modzilla profiles in the Steam
Deck's quick access menu (the **…** button): switch a game's profile, or take its mods out,
without leaving Game Mode. Then start the game from your library as usual.

It only reads Zom's Modzilla's profile list. Switching is done by Zom's Modzilla itself
(`zoms-modzilla --profile <id>` or `--no-mods <game>`), so the same checks run as in the app.

## Build

```bash
npm install
npm run build
```

Copy `plugin.json`, `main.py`, `package.json` and `dist/` into
`~/homebrew/plugins/zoms-modzilla/` on the Deck, then restart Decky (or use Decky's developer
mode to install the folder as a zip).

Open Zom's Modzilla once on the Deck first, so the plugin knows how to start it.
