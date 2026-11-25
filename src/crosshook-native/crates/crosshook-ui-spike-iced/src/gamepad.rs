//! gilrs 0.11 on a dedicated thread, mapped to the same `Nav` actions as the keyboard.
//! The thread blocks in `next_event_blocking` (no idle polling in the UI); the stick uses a
//! deadzone with edge-triggered steps plus a slow auto-repeat while held.

use std::time::{Duration, Instant};

use gilrs::{Axis, Button, EventType};
use iced::futures::SinkExt;
use iced::{Subscription, stream};

use crate::focus::Dir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Move(Dir),
    Confirm,
    Back,
    NextZone,
    PrevZone,
}

pub const DEADZONE: f32 = 0.5;
const REPEAT: Duration = Duration::from_millis(180);

pub fn map_button(button: Button) -> Option<Nav> {
    Some(match button {
        Button::DPadUp => Nav::Move(Dir::Up),
        Button::DPadDown => Nav::Move(Dir::Down),
        Button::DPadLeft => Nav::Move(Dir::Left),
        Button::DPadRight => Nav::Move(Dir::Right),
        Button::South => Nav::Confirm,
        Button::East => Nav::Back,
        Button::RightTrigger => Nav::NextZone,
        Button::LeftTrigger => Nav::PrevZone,
        _ => return None,
    })
}

/// Stick axis to a direction once it leaves the deadzone. gilrs: +y is up.
pub fn map_axis(axis: Axis, value: f32) -> Option<Dir> {
    if value.abs() < DEADZONE {
        return None;
    }
    match axis {
        Axis::LeftStickX if value > 0.0 => Some(Dir::Right),
        Axis::LeftStickX => Some(Dir::Left),
        Axis::LeftStickY if value > 0.0 => Some(Dir::Up),
        Axis::LeftStickY => Some(Dir::Down),
        _ => None,
    }
}

pub fn subscription() -> Subscription<Nav> {
    Subscription::run(|| {
        stream::channel(32, async |mut out| {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let spawned = std::thread::Builder::new()
                .name("spike-gilrs".into())
                .spawn(move || {
                    let Ok(mut gilrs) = gilrs::Gilrs::new() else {
                        eprintln!("spike: gilrs unavailable; gamepad disabled");
                        return;
                    };
                    let mut held: Option<(Dir, Instant)> = None;
                    loop {
                        let timeout = held.map(|_| REPEAT);
                        let Some(event) = gilrs.next_event_blocking(timeout) else {
                            if let Some((dir, at)) = held
                                && at.elapsed() >= REPEAT
                            {
                                held = Some((dir, Instant::now()));
                                if tx.send(Nav::Move(dir)).is_err() {
                                    return;
                                }
                            }
                            continue;
                        };
                        let nav = match event.event {
                            EventType::ButtonPressed(b, _) => map_button(b),
                            EventType::AxisChanged(axis, value, _) => {
                                let dir = map_axis(axis, value);
                                let edge = dir.is_some() && held.map(|(d, _)| d) != dir;
                                if dir.is_none()
                                    && matches!(axis, Axis::LeftStickX | Axis::LeftStickY)
                                {
                                    held = None;
                                }
                                if edge {
                                    held = dir.map(|d| (d, Instant::now()));
                                }
                                dir.filter(|_| edge).map(Nav::Move)
                            }
                            _ => None,
                        };
                        if let Some(nav) = nav
                            && tx.send(nav).is_err()
                        {
                            return;
                        }
                    }
                });
            if spawned.is_err() {
                return;
            }
            while let Some(nav) = rx.recv().await {
                if out.send(nav).await.is_err() {
                    break;
                }
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_map_to_keyboard_parity_actions() {
        assert_eq!(map_button(Button::South), Some(Nav::Confirm));
        assert_eq!(map_button(Button::East), Some(Nav::Back));
        assert_eq!(map_button(Button::RightTrigger), Some(Nav::NextZone));
        assert_eq!(map_button(Button::LeftTrigger), Some(Nav::PrevZone));
        assert_eq!(map_button(Button::DPadLeft), Some(Nav::Move(Dir::Left)));
        assert_eq!(map_button(Button::Start), None);
    }

    #[test]
    fn stick_respects_deadzone() {
        assert_eq!(map_axis(Axis::LeftStickX, 0.3), None);
        assert_eq!(map_axis(Axis::LeftStickX, 0.9), Some(Dir::Right));
        assert_eq!(map_axis(Axis::LeftStickY, -0.9), Some(Dir::Down));
        assert_eq!(map_axis(Axis::RightStickX, 0.9), None);
    }
}
