# Zom's Nexus on Flathub

The files Flathub needs: the build manifest (`com.zombeatz.zoms-nexus.yml`), the desktop file,
and the AppStream metainfo. Flathub builds from source without network access, so the Rust
and npm dependencies are listed ahead of time.

## Before each release

1. Tag the release in `ZomBeatz/zoms-nexus` (`v0.1.0`) and set the same `tag:` in the manifest.
2. Add the release to `<releases>` in the metainfo.
3. Make the offline dependency lists (from [flatpak-builder-tools](https://github.com/flatpak/flatpak-builder-tools)):

```bash
python3 flatpak-builder-tools/cargo/flatpak-cargo-generator.py ../../src-tauri/Cargo.lock -o cargo-sources.json
```

```bash
flatpak-node-generator npm ../../package-lock.json -o node-sources.json
```

4. Check the metainfo, then build and try it:

```bash
flatpak run --command=flatpak-builder-lint org.flatpak.Builder appstream com.zombeatz.zoms-nexus.metainfo.xml
```

```bash
flatpak run org.flatpak.Builder --force-clean --user --install --install-deps-from=flathub build com.zombeatz.zoms-nexus.yml
```

## Submitting

Fork `flathub/flathub`, add these files (and the two generated lists) on a branch from
`new-pr`, and open a pull request. Screenshots for the store come from the metainfo once
there's a public URL for them.

## Why each permission

- `--filesystem=home`, `/run/media`, `/media`, `/mnt`: mods go into games' own folders, which
  live anywhere (Steam libraries on other drives, the Steam Deck's SD card).
- `--device=all`: the GPU, and controllers for controller navigation.
- `--talk-name=org.freedesktop.Flatpak`: games, Steam, Heroic and Lutris run on the computer,
  so starting them, telling when they're running and closing them goes through
  `flatpak-spawn --host` (`src-tauri/src/host.rs`).
- `org.freedesktop.secrets`, `Notifications`, `StatusNotifierWatcher`: the API key in the
  keyring, finished-download notices, and the tray icon.
