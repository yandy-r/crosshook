//! Keyboard → app messages, with parity to the gamepad `Nav` actions.
//! Keys a focused text input consumed (status `Captured`) are left alone, except Escape and
//! Tab, which always drive the focus layer.

use iced::event::{self, Event, Status};
use iced::keyboard::{self, Key, key::Named};
use iced::{Subscription, window};

use super::Message;
use crate::focus::Dir;
use crate::gamepad::Nav;

pub fn subscription() -> Subscription<Message> {
    event::listen_with(map)
}

fn map(event: Event, status: Status, _window: window::Id) -> Option<Message> {
    let Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) = event else {
        return None;
    };
    map_key(&key, modifiers, status == Status::Captured)
}

pub fn map_key(key: &Key, modifiers: keyboard::Modifiers, captured: bool) -> Option<Message> {
    match key {
        Key::Named(Named::Escape) => return Some(Message::Escape),
        Key::Named(Named::Tab) => {
            return Some(Message::Tab {
                forward: !modifiers.shift(),
            });
        }
        Key::Named(Named::F6) => {
            return Some(Message::Nav(if modifiers.shift() {
                Nav::PrevZone
            } else {
                Nav::NextZone
            }));
        }
        _ => {}
    }
    if modifiers.control() && matches!(key, Key::Character(c) if c.as_str() == "k") {
        return Some(Message::OpenPalette);
    }
    if captured {
        return None;
    }
    if modifiers.alt() {
        return match key {
            Key::Named(Named::ArrowLeft) => Some(Message::Back),
            Key::Named(Named::ArrowRight) => Some(Message::Forward),
            _ => None,
        };
    }
    if modifiers.control() {
        return None;
    }
    Some(match key {
        Key::Named(Named::ArrowUp) => Message::Nav(Nav::Move(Dir::Up)),
        Key::Named(Named::ArrowDown) => Message::Nav(Nav::Move(Dir::Down)),
        Key::Named(Named::ArrowLeft) => Message::Nav(Nav::Move(Dir::Left)),
        Key::Named(Named::ArrowRight) => Message::Nav(Nav::Move(Dir::Right)),
        Key::Named(Named::Enter | Named::Space) => Message::Nav(Nav::Confirm),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::Modifiers;

    #[test]
    fn keyboard_parity_with_gamepad() {
        let m = Modifiers::empty();
        assert!(matches!(
            map_key(&Key::Named(Named::Enter), m, false),
            Some(Message::Nav(Nav::Confirm))
        ));
        assert!(matches!(
            map_key(&Key::Named(Named::F6), m, false),
            Some(Message::Nav(Nav::NextZone))
        ));
        assert!(matches!(
            map_key(&Key::Named(Named::F6), Modifiers::SHIFT, false),
            Some(Message::Nav(Nav::PrevZone))
        ));
        assert!(matches!(
            map_key(&Key::Named(Named::ArrowRight), m, false),
            Some(Message::Nav(Nav::Move(Dir::Right)))
        ));
        assert!(matches!(
            map_key(&Key::Named(Named::Escape), m, true),
            Some(Message::Escape)
        ));
        assert!(
            map_key(&Key::Named(Named::ArrowRight), m, true).is_none(),
            "text input keeps arrows"
        );
        assert!(matches!(
            map_key(&Key::Character("k".into()), Modifiers::CTRL, false),
            Some(Message::OpenPalette)
        ));
    }
}
