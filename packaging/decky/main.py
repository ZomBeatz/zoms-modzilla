"""Zom's Modzilla in the Steam Deck's quick access menu.

Reads Zom's Modzilla's profiles and which one is in each game (never writes them), and asks the app
to switch: `zoms-modzilla --profile <id>` or `--no-mods <game>`. A running app takes the request;
otherwise it starts and does it.
"""

import json
import os
import subprocess

import decky

APP = "com.zombeatz.zoms-modzilla"


def _home():
    return decky.DECKY_USER_HOME


def _dirs():
    """(config, data) folders: the Flatpak's when it's installed that way."""
    home = _home()
    flat = os.path.join(home, ".var", "app", APP)
    if os.path.isdir(flat):
        return os.path.join(flat, "config", APP), os.path.join(flat, "data", APP)
    return os.path.join(home, ".config", APP), os.path.join(home, ".local", "share", APP)


def _read(path, default):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def _safe(name):
    return "".join(c if c.isalnum() or c in "-_ ." else "_" for c in name).strip(" .") or "_"


class Plugin:
    async def profiles(self):
        config, data = _dirs()
        out = {}
        for p in _read(os.path.join(config, "profiles.json"), []):
            game = p.get("game", "")
            g = out.setdefault(game, {"game": game, "active": None, "profiles": []})
            g["profiles"].append({"id": p.get("id"), "name": p.get("name"), "mods": sum(1 for m in p.get("mods", []) if m.get("enabled", True))})
        for game, g in out.items():
            g["active"] = _read(os.path.join(data, "deployed", _safe(game) + ".json"), {}).get("profileId")
        return sorted(out.values(), key=lambda g: g["game"])

    async def use(self, profile_id: str):
        return self._run(["--profile", str(profile_id)])

    async def no_mods(self, game: str):
        return self._run(["--no-mods", str(game)])

    def _run(self, args):
        _, data = _dirs()
        launch = _read(os.path.join(data, "launch.json"), {})
        if launch.get("flatpak"):
            cmd = ["flatpak", "run", APP] + args
        elif launch.get("exe") and os.path.exists(launch["exe"]):
            cmd = [launch["exe"]] + args
        else:
            return "Open Zom's Modzilla once on this Deck first."
        env = dict(os.environ, HOME=_home(), DISPLAY=os.environ.get("DISPLAY", ":0"))
        env.setdefault("XDG_RUNTIME_DIR", "/run/user/%d" % os.getuid())
        try:
            subprocess.Popen(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        except OSError as e:
            return "Couldn't start Zom's Modzilla: %s" % e
        return ""

    async def _main(self):
        decky.logger.info("Zom's Modzilla plugin ready")

    async def _unload(self):
        pass
