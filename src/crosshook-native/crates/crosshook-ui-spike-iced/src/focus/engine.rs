//! Pure focus engine: zones with memory, linear (Tab) and spatial (D-pad) traversal, and the
//! overlay trap. It knows nothing about iced widgets; it works on `(Target, Rectangle)` lists
//! collected from the widget tree by `collect::Collect`.

use std::collections::HashMap;

use iced::Rectangle;

use super::target::{Target, Zone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Default)]
pub struct FocusState {
    pub focused: Option<Target>,
    memory: HashMap<Zone, Target>,
    /// Set while an overlay (modal / palette) is open: focus is trapped in `Zone::Overlay`.
    pub overlay_open: bool,
}

pub const CYCLE: [Zone; 3] = [Zone::Sidebar, Zone::Content, Zone::Inspector];

impl FocusState {
    pub fn set(&mut self, target: Target) {
        self.memory.insert(target.zone(), target.clone());
        self.focused = Some(target);
    }

    pub fn zone(&self) -> Option<Zone> {
        self.focused.as_ref().map(Target::zone)
    }

    /// Drops the overlay memory and returns to the zone focused before the overlay opened.
    pub fn close_overlay(&mut self, restore: Option<Target>) {
        self.overlay_open = false;
        self.memory.remove(&Zone::Overlay);
        self.focused = restore;
        if let Some(t) = self.focused.clone() {
            self.memory.insert(t.zone(), t);
        }
    }

    /// Items reachable right now: the overlay only, while it is open.
    fn reachable<'a>(&self, items: &'a [(Target, Rectangle)]) -> Vec<&'a (Target, Rectangle)> {
        items
            .iter()
            .filter(|(t, _)| (t.zone() == Zone::Overlay) == self.overlay_open)
            .collect()
    }

    /// Tab / Shift+Tab in tree order, wrapping, inside the reachable set.
    pub fn linear(&mut self, items: &[(Target, Rectangle)], forward: bool) -> Option<Target> {
        let reachable = self.reachable(items);
        if reachable.is_empty() {
            return None;
        }
        let pos = self
            .focused
            .as_ref()
            .and_then(|f| reachable.iter().position(|(t, _)| t == f));
        let n = reachable.len();
        let next = match (pos, forward) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(p), true) => (p + 1) % n,
            (Some(p), false) => (p + n - 1) % n,
        };
        let target = reachable[next].0.clone();
        self.set(target.clone());
        Some(target)
    }

    /// LB/RB, F6/Shift+F6: next zone in `CYCLE`, landing on its remembered target or its first
    /// item. Disabled while an overlay is open (the trap holds).
    pub fn cycle_zone(&mut self, items: &[(Target, Rectangle)], forward: bool) -> Option<Target> {
        if self.overlay_open {
            return None;
        }
        let current = self.zone().and_then(|z| CYCLE.iter().position(|c| *c == z));
        for step in 1..=CYCLE.len() {
            let idx = match current {
                Some(c) if forward => (c + step) % CYCLE.len(),
                Some(c) => (c + CYCLE.len() * 2 - step) % CYCLE.len(),
                None => step - 1,
            };
            let zone = CYCLE[idx];
            let in_zone: Vec<_> = items.iter().filter(|(t, _)| t.zone() == zone).collect();
            if in_zone.is_empty() {
                continue;
            }
            let target = self
                .memory
                .get(&zone)
                .filter(|m| in_zone.iter().any(|(t, _)| t == *m))
                .cloned()
                .unwrap_or_else(|| in_zone[0].0.clone());
            self.set(target.clone());
            return Some(target);
        }
        None
    }

    /// D-pad / arrows: nearest item in `dir` within the current zone (overlay when open).
    pub fn spatial(&mut self, items: &[(Target, Rectangle)], dir: Dir) -> Option<Target> {
        let reachable = self.reachable(items);
        let Some(current) = self.focused.clone() else {
            return self.linear(items, true);
        };
        let Some((_, from)) = reachable.iter().find(|(t, _)| *t == current).copied() else {
            return self.linear(items, true);
        };
        let zone = current.zone();
        let best = reachable
            .iter()
            .filter(|(t, _)| *t != current && t.zone() == zone)
            .filter_map(|(t, r)| score(*from, *r, dir).map(|s| (s, t)))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, t)| t.clone())?;
        self.set(best.clone());
        Some(best)
    }
}

/// Distance score for a candidate in direction `dir`, or `None` if it is not in that half-plane.
/// Off-axis distance is weighted x3 so a card in the same row beats a closer one diagonally.
pub fn score(from: Rectangle, to: Rectangle, dir: Dir) -> Option<f32> {
    let a = from.center();
    let b = to.center();
    let (along, across) = match dir {
        Dir::Right => (b.x - a.x, b.y - a.y),
        Dir::Left => (a.x - b.x, b.y - a.y),
        Dir::Down => (b.y - a.y, b.x - a.x),
        Dir::Up => (a.y - b.y, b.x - a.x),
    };
    let edge_gap = match dir {
        Dir::Right => to.x - (from.x + from.width),
        Dir::Left => from.x - (to.x + to.width),
        Dir::Down => to.y - (from.y + from.height),
        Dir::Up => from.y - (to.y + to.height),
    };
    if along <= 1.0 || edge_gap < -from.width.min(from.height) / 2.0 {
        return None;
    }
    Some(along + across.abs() * 3.0)
}
