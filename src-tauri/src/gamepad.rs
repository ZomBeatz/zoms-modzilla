//! Controllers read by the app itself (through the system's input devices), so a Steam Deck's
//! controls, or any gamepad, work even where the page's own gamepad support doesn't reach (some
//! WebKit builds have none). Presses go to the page as `pad` events; the page moves the focus.
//! Directions from the left stick come as D-pad presses. Nothing is sent anywhere else.

use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Press {
    /// "a", "b", "x", "y", "lb", "rb", "lt", "rt", "menu", "view", "up", "down", "left", "right".
    button: &'static str,
    down: bool,
}

fn name(b: gilrs::Button) -> Option<&'static str> {
    use gilrs::Button::*;
    Some(match b {
        South => "a",
        East => "b",
        West => "x",
        North => "y",
        LeftTrigger => "lb",
        RightTrigger => "rb",
        LeftTrigger2 => "lt",
        RightTrigger2 => "rt",
        Start => "menu",
        Select => "view",
        DPadUp => "up",
        DPadDown => "down",
        DPadLeft => "left",
        DPadRight => "right",
        _ => return None,
    })
}

/// Start reading controllers in the background. Without input devices it quietly does nothing.
pub fn start(handle: AppHandle) {
    std::thread::spawn(move || {
        let Ok(mut g) = gilrs::Gilrs::new() else { return };
        // The left stick as a D-pad: which way it points now.
        let mut stick: Option<&'static str> = None;
        let mut right_y = 0.0f32;
        loop {
            // Wait for something to happen (no wake-ups while the controller rests), except
            // while the right stick scrolls, which repeats.
            let first = if right_y.abs() > 0.25 { g.next_event() } else { g.next_event_blocking(Some(Duration::from_millis(500))) };
            let mut pending = first;
            while let Some(ev) = pending.take().or_else(|| g.next_event()) {
                use gilrs::EventType::*;
                match ev.event {
                    ButtonPressed(b, _) => {
                        if let Some(n) = name(b) {
                            let _ = handle.emit("pad", Press { button: n, down: true });
                        }
                    }
                    ButtonReleased(b, _) => {
                        if let Some(n) = name(b) {
                            let _ = handle.emit("pad", Press { button: n, down: false });
                        }
                    }
                    AxisChanged(axis, v, _) => match axis {
                        gilrs::Axis::LeftStickX | gilrs::Axis::LeftStickY => {
                            let pad = g.gamepad(ev.id);
                            let x = pad.value(gilrs::Axis::LeftStickX);
                            let y = pad.value(gilrs::Axis::LeftStickY);
                            let now = if y > 0.6 { Some("up") } else if y < -0.6 { Some("down") } else if x < -0.6 { Some("left") } else if x > 0.6 { Some("right") } else { None };
                            if now != stick {
                                if let Some(old) = stick {
                                    let _ = handle.emit("pad", Press { button: old, down: false });
                                }
                                if let Some(n) = now {
                                    let _ = handle.emit("pad", Press { button: n, down: true });
                                }
                                stick = now;
                            }
                        }
                        gilrs::Axis::RightStickY => right_y = v,
                        _ => {}
                    },
                    _ => {}
                }
            }
            // The right stick scrolls while it's held.
            if right_y.abs() > 0.25 {
                let _ = handle.emit("pad-scroll", -right_y);
                std::thread::sleep(Duration::from_millis(16));
            }
        }
    });
}
