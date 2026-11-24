//! Battery and sleep, for portable PCs like the Steam Deck: downloads can wait while the battery
//! is low, unpacking can wait for the charger, and downloads interrupted by sleep start again
//! right away on waking (picking up where they stopped) instead of waiting out a timeout.

use std::{fs, path::Path};

/// The battery's charge and whether it's charging (or on the charger), when there is one.
pub fn battery() -> Option<(u8, bool)> {
    battery_in(Path::new("/sys/class/power_supply"))
}

pub fn battery_in(dir: &Path) -> Option<(u8, bool)> {
    let mut bat = None;
    let mut ac = false;
    for e in fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        let read = |n: &str| fs::read_to_string(p.join(n)).map(|s| s.trim().to_string()).unwrap_or_default();
        match read("type").as_str() {
            "Battery" if read("scope") != "Device" => {
                let cap: u8 = read("capacity").parse().ok()?;
                bat = Some((cap.min(100), matches!(read("status").as_str(), "Charging" | "Full" | "Not charging")));
            }
            "Mains" | "USB" if read("online") == "1" => ac = true,
            _ => {}
        }
    }
    bat.map(|(c, charging)| (c, charging || ac))
}

/// Running on battery (no charger).
pub fn on_battery() -> bool {
    battery().is_some_and(|(_, charging)| !charging)
}

/// On battery and below `floor` percent (0 means never).
pub fn low(floor: u8) -> Option<u8> {
    let (cap, charging) = battery()?;
    (floor > 0 && !charging && cap < floor).then_some(cap)
}

/// Time spent asleep since the computer started, in seconds (boot time minus awake time).
pub fn slept() -> f64 {
    let read = |c: libc::clockid_t| {
        let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: clock_gettime only writes into the given timespec.
        unsafe { libc::clock_gettime(c, &mut t) };
        t.tv_sec as f64 + t.tv_nsec as f64 / 1e9
    };
    let awake = read(libc::CLOCK_MONOTONIC);
    (read(libc::CLOCK_BOOTTIME) - awake).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_battery() {
        let t = tempfile::tempdir().unwrap();
        let w = |d: &str, f: &str, v: &str| {
            fs::create_dir_all(t.path().join(d)).unwrap();
            fs::write(t.path().join(d).join(f), v).unwrap();
        };
        assert_eq!(battery_in(t.path()), None, "a desktop has none");
        w("BAT1", "type", "Battery\n");
        w("BAT1", "capacity", "18\n");
        w("BAT1", "status", "Discharging\n");
        w("ACAD", "type", "Mains\n");
        w("ACAD", "online", "0\n");
        // A controller's battery isn't the computer's.
        w("hid-pad", "type", "Battery");
        w("hid-pad", "scope", "Device");
        w("hid-pad", "capacity", "90");
        assert_eq!(battery_in(t.path()), Some((18, false)));
        w("ACAD", "online", "1\n");
        assert_eq!(battery_in(t.path()), Some((18, true)));
        assert!(slept() >= 0.0);
    }
}
