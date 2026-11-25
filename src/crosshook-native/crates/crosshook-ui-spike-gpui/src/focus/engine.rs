//! Pure focus engine: zones with memory, linear traversal and spatial (D-pad) traversal, and the
//! overlay trap. Ported from the iced spike with its rectangle type swapped for a local `Rect`,
//! so it has no toolkit types at all. It works on `(Target, Rect)` lists that the gpui layer
//! collects during prepaint (`registry::FocusRegistry`).

use std::collections::HashMap;

use super::target::{Target, Zone};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

#[derive(Debug, Default)]
pub struct FocusState {
    pub focused: Option<Target>,
    memory: HashMap<Zone, Target>,
    /// Set while an overlay (dialog / palette) is open: focus is trapped in `Zone::Overlay`.
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

    /// Drops the overlay memory and returns to the target focused before the overlay opened.
    pub fn close_overlay(&mut self, restore: Option<Target>) {
        self.overlay_open = false;
        self.memory.remove(&Zone::Overlay);
        self.focused = restore;
        if let Some(t) = self.focused.clone() {
            self.memory.insert(t.zone(), t);
        }
    }

    fn reachable<'a>(&self, items: &'a [(Target, Rect)]) -> Vec<&'a (Target, Rect)> {
        items
            .iter()
            .filter(|(t, _)| (t.zone() == Zone::Overlay) == self.overlay_open)
            .collect()
    }

    /// Linear order in paint order, wrapping, inside the reachable set. gpui's own Tab handles
    /// the keyboard; this is used by the D-pad when nothing is focused yet.
    pub fn linear(&mut self, items: &[(Target, Rect)], forward: bool) -> Option<Target> {
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
    pub fn cycle_zone(&mut self, items: &[(Target, Rect)], forward: bool) -> Option<Target> {
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
    pub fn spatial(&mut self, items: &[(Target, Rect)], dir: Dir) -> Option<Target> {
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
pub fn score(from: Rect, to: Rect, dir: Dir) -> Option<f32> {
    let (ax, ay) = from.center();
    let (bx, by) = to.center();
    let (along, across) = match dir {
        Dir::Right => (bx - ax, by - ay),
        Dir::Left => (ax - bx, by - ay),
        Dir::Down => (by - ay, bx - ax),
        Dir::Up => (ay - by, bx - ax),
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
