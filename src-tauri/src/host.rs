//! The computer outside a Flatpak sandbox. Installed from Flathub, Zom's Nexus runs in a
//! sandbox that can't see other programs or start them directly: games, Steam, Lutris and
//! Heroic run on the computer itself, so they're started and watched through `flatpak-spawn
//! --host` (allowed by the Flatpak's `--talk-name=org.freedesktop.Flatpak`). Installed any other
//! way, everything here is the plain way.

use std::{fs, path::Path, process::Command};

pub fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists()
}

/// A command run on the computer (outside the sandbox, when there is one).
pub fn command(program: &str) -> Command {
    if in_flatpak() {
        let mut c = Command::new("flatpak-spawn");
        c.arg("--host").arg(program);
        c
    } else {
        Command::new(program)
    }
}

/// The computer's own os-release (a Flatpak's /etc has its runtime's).
pub fn os_release() -> String {
    if in_flatpak() {
        if let Ok(t) = fs::read_to_string("/run/host/os-release") {
            return t;
        }
    }
    fs::read_to_string("/etc/os-release").unwrap_or_default()
}

/// A running program: its number, its name, and its command line (lowercase, `/` for `\`).
#[derive(Debug, Clone, PartialEq)]
pub struct Proc {
    pub pid: i32,
    pub comm: String,
    pub cmd: String,
}

/// `ps -eo pid=,comm=,args=` read.
pub fn parse_ps(out: &str) -> Vec<Proc> {
    out.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            let (pid, rest) = l.split_once(char::is_whitespace)?;
            let rest = rest.trim_start();
            let (comm, args) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
            Some(Proc { pid: pid.parse().ok()?, comm: comm.to_string(), cmd: args.trim().replace('\\', "/").to_lowercase() })
        })
        .collect()
}

/// Every program running on the computer (never this one).
pub fn processes() -> Vec<Proc> {
    let me = std::process::id() as i32;
    if in_flatpak() {
        return command("ps").args(["-eo", "pid=,comm=,args="]).output().map(|o| parse_ps(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default().into_iter().filter(|p| p.pid != me).collect();
    }
    fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let pid: i32 = e.file_name().to_string_lossy().parse().ok()?;
            if pid == me {
                return None;
            }
            let cmd = fs::read(e.path().join("cmdline")).map(|b| String::from_utf8_lossy(&b).replace('\0', " ")).unwrap_or_default().replace('\\', "/").to_lowercase();
            let comm = fs::read_to_string(e.path().join("comm")).unwrap_or_default().trim().to_string();
            Some(Proc { pid, comm, cmd })
        })
        .collect()
}

/// Ask a program to close (TERM), or make it (KILL).
pub fn signal(pid: i32, kill: bool) {
    if in_flatpak() {
        let _ = command("kill").args([if kill { "-KILL" } else { "-TERM" }, &pid.to_string()]).status();
    } else {
        // SAFETY: kill only sends a signal to the given process number.
        unsafe { libc::kill(pid, if kill { libc::SIGKILL } else { libc::SIGTERM }) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ps() {
        let out = "  1 systemd /sbin/init\n 4242 wine64 Z:\\mnt\\Steam Games\\steamapps\\common\\Cool Game\\game.exe -dx12\n 77 steam\n";
        let p = parse_ps(out);
        assert_eq!(p.len(), 3);
        assert_eq!(p[1].pid, 4242);
        assert!(p[1].cmd.contains("/steamapps/common/cool game/game.exe"));
        assert_eq!((p[2].comm.as_str(), p[2].cmd.as_str()), ("steam", ""));
        assert!(processes().iter().all(|x| x.pid != std::process::id() as i32));
    }
}
