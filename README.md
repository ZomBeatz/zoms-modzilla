<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="branding/nexus-logo-dark.png">
    <img src="branding/nexus-logo.png" alt="Zom's Nexus by ZomBeatz Media" width="460">
  </picture>
</p>

<p align="center">
  <b>Every mod you download, kept in order.</b><br>
  A free mod manager for Linux and the Steam Deck: Nexus Mods and Thunderstore, collections, profiles per game with a conflict check, troubleshooting, and Play.
</p>

## Quick start

1. **Install it.** Download the file for your Linux from the latest release:

   | Your Linux | File | How |
   |---|---|---|
   | Ubuntu, Linux Mint, Pop!_OS, Debian, Zorin | `.deb` | Open it with your software installer, or `sudo apt install ./Zom*Nexus*.deb` |
   | Fedora, Nobara, openSUSE | `.rpm` | Open it with your software installer, or `sudo dnf install ./Zom*Nexus*.rpm` (`sudo zypper install` on openSUSE) |
   | Arch, CachyOS, Manjaro, EndeavourOS, SteamOS, anything else | `.AppImage` | Right-click > **Properties** > allow running it as a program, then open it |

2. **Follow "Getting started"** at the top of the app. It walks you through:
   - **Connect your Nexus Mods account:** paste your personal API key from nexusmods.com > **Settings > API keys**. It stays on your computer.
   - **Let the website's download buttons open Zom's Nexus**, so **Mod manager download** (and **Vortex**) buttons send files to the app.
   - **Find your games:** Steam games are found by themselves; add others by hand.
   - **Get your first mod.**
3. **Check Settings > This computer.** It shows what's set up and, for anything missing, the exact command to install it on your Linux.

### Optional extras

Zom's Nexus runs without these, and says what each adds:

| For | Arch-based | Ubuntu, Debian, Mint | Fedora | openSUSE |
|---|---|---|---|---|
| A tray icon (keep downloading with the window closed) | `libayatana-appindicator` | `libayatana-appindicator3-1` | `libayatana-appindicator-gtk3` | `libayatana-appindicator3-1` |
| Notifications | `libnotify` | `libnotify-bin` | `libnotify` | `libnotify-tools` |
| Unpacking `.rar` mods (zip and 7z always work) | `libarchive` | `libarchive-tools` | `bsdtar` | `bsdtar` |
| Opening Nexus Mods links | `xdg-utils` | `xdg-utils` | `xdg-utils` | `xdg-utils` |

### If the window opens blank

On some NVIDIA setups the window can stay white. Zom's Nexus already turns off the WebKit setting that usually causes it; if it still happens, start it from a terminal with `WEBKIT_DISABLE_COMPOSITING_MODE=1 zoms-nexus`.

## What it does

| | |
|---|---|
| **One at a time or several at once** | Match my account (Premium several, free one), one at a time in list order, or up to 8 at once, with a speed limit for all of them together. Switch any time; it takes effect right away. |
| **One game at a time** | Pick a game at the top of the sidebar (your recent ones, or **All games…**) and every page shows just that game's downloads, mods and profiles. |
| **Collections** | Paste a collection's address (or use its button on the website) to queue every file and get a profile made from it. Files you already have aren't downloaded again. |
| **Browse** | Trending, new and just-updated mods for each game, with pictures. |
| **Profiles and Play** | Sets of mods per game, in load order. Add a finished download to one in a click. Play unpacks what's still packed, puts the mods in the game's folder (hard links, game files kept aside and put back), switches Bethesda plugins on, and starts the game. |
| **Conflict check** | Runs by itself whenever a profile changes: which mods overwrite which files, plugins missing a master or loading too early, mods missing what they need (as listed on Nexus Mods), and mods that run code but haven't been updated since the game was. |
| **Your own mod files** | Add a mod from another site, or your own, from its zip, 7z or rar file. It works like any other mod (but stays out of mod packs). |
| **Room on the drive** | Bars show how full the library's and each game's drives are. Downloads, unpacking and copying mods into a game check there's room first. |
| **Mod packs** | Share a profile as a small file listing its mods; opening one queues the downloads and makes the profile. |
| **Updates** | Finds newer versions of the files in your library. |
| **Resume** | Pause, close the app, lose the connection: downloads carry on from where they stopped. A dropped connection is tried again by itself. Closing the window while files download keeps it going in the system tray, and it can start with your computer to finish them. |
| **Download now, and in order** | Start any file straight away (the one in its slot steps aside and carries on after), sort the waiting files smallest or biggest first, and see progress in the sidebar. |
| **Free and Premium accounts** | Free accounts click **Slow download** for each file, as Nexus Mods asks; the app shows the pages one after another in its own Nexus Mods window. Premium accounts get every file without clicks, several at once. |
| **A tidy library** | `~/Games/Nexus Mods/<game>/<mod id> - <name>/`, with a `nexus.json` for each mod. Zip, 7z and rar files are unpacked when they finish (you can turn that off). |
| **Your games** | Cover art for each one. Finds Nexus Mods games in every Steam library folder (native and Proton, Flatpak and Snap), and the Games page lists the folders it searched. Add others by hand, with their mod folder and start command. |
| **Mod installers** | Mods with an installer (FOMOD) show each step with its pictures; your choices are kept for the next version, and a collection's curator choices are used by themselves. |
| **Thunderstore too** | For the 280+ BepInEx games on Thunderstore (Valheim, Lethal Company, R.E.P.O. …): search, get a mod with everything it needs, update all, and keep profiles made from mod packs up to date. No Overwolf. |
| **Bring your setup across** | Make a profile from a Mod Organizer 2 profile (nothing downloaded again, installer choices kept) or from an r2modman profile or profile code (its config files too). |
| **Each profile's own settings** | Mods' settings files (.ini, .cfg, .json …) are kept per profile, with an editor that turns them into switches and boxes. |
| **When a game won't start** | Notices a game that closes right after starting and shows what changed since it last ran fine; reads mod loaders' logs (SMAPI, script extenders, BepInEx, MelonLoader, UE4SS, REFramework, Cyber Engine Tweaks, RED4ext) and names the mod at fault; warns before Play when the same crash is coming; finds loaders a profile needs but doesn't have, mods packed a folder too deep, and the Proton launch options mod loaders need. **Play without mods** and **Find the problem mod** narrow it down. |
| **Ask AI (optional)** | Bring any AI service (Claude, OpenAI, Gemini, OpenRouter, or one on your own computer like Ollama): it gets the check, the logs and the last runs, with your home folder and names taken out, and only when you press Send. |
| **Steam Deck** | **Add to Steam** puts it in your library with artwork, so Game Mode opens it full screen; the controller moves between buttons (A, B, Y, L1/R1), text boxes open Steam's keyboard, and mods on an SD card stay on the card. |
| **More launchers** | Games from Heroic (GOG, Epic, Amazon) and Lutris as well as Steam; shows which Proton each runs with. Terraria's tModLoader mods and mod packs too. |
| **Saves per profile** | A heavily modded profile can keep its own saves; Zom's Nexus asks before the first Play and warns when Steam Cloud would copy saves back in. |
| **Privacy** | No account of its own and no tracking. Your API key stays on your computer in a file only you can read. |

Not done yet: Baldur's Gate 3's own mod list isn't managed, and Thunderstore games with loaders other than BepInEx can be browsed but not installed. Collection files hosted outside Nexus Mods have to be fetched by hand.

## Building it

Needs Node.js 20 or newer, Rust, Tauri's Linux packages (`webkit2gtk-4.1`, `libayatana-appindicator`, `librsvg`), and libudev (`systemd-libs` on Arch, `libudev-dev` on Debian and Ubuntu) for controllers.

```bash
npm install
npm run app       # the desktop app with live reload
npm run dev       # just the page, in a browser, with made-up demo data (add ?free for a free account)
npm test          # the page's tests
cargo test --manifest-path src-tauri/Cargo.toml   # the queue, library and link tests
npm run bundle    # .deb, .rpm and .AppImage in src-tauri/target/release/bundle
```

## Where things are kept

| What | Where |
|---|---|
| Settings | `~/.config/com.zombeatz.zoms-nexus/settings.json` |
| Your API key | `~/.config/com.zombeatz.zoms-nexus/api-key` (only you can read it) |
| The download list | `~/.local/share/com.zombeatz.zoms-nexus/queue.json` |
| Mods | The library folder (**Settings > Storage**), `~/Games/Nexus Mods` at first |
| Profiles | `~/.config/com.zombeatz.zoms-nexus/profiles.json` |
| What's in each game now | `~/.local/share/com.zombeatz.zoms-nexus/deployed/<game>.json`, and the game's own replaced files in `<game folder>/.zoms-nexus-backup` |
| Game art | `~/.cache/com.zombeatz.zoms-nexus/art` |
| Each profile's mod settings | `~/.local/share/com.zombeatz.zoms-nexus/profile-settings/<profile>/` |
| How each Play went | `~/.local/share/com.zombeatz.zoms-nexus/runs/<profile>.json` |
| The AI service's key (if you set one) | `~/.config/com.zombeatz.zoms-nexus/ai-key` (only you can read it) |
| Thunderstore's lists | `~/.cache/com.zombeatz.zoms-nexus/thunderstore` (fetched at most once an hour) |

## License

[MIT](LICENSE). The ZomBeatz Media and Zom's Nexus names and logos aren't covered by it. Zom's Nexus isn't made by or connected with Nexus Mods; it uses the public Nexus Mods API with your own key.
