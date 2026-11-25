//! Grid geometry (ported from the iced spike): column count, row count and D-pad steps.
//! Culling itself is Kit's `VirtualList` here, so the iced culling helpers were dropped.

use crate::focus::Dir;

pub const CARD_W: f32 = 160.0;
pub const CARD_H: f32 = 270.0;
pub const GAP: f32 = 12.0;
pub const PAD: f32 = 12.0;
pub const OVERSCAN_ROWS: usize = 1;

#[derive(Debug, Clone, Copy)]
pub struct Grid {
    pub cols: usize,
    pub items: usize,
}

impl Grid {
    pub fn new(width: f32, items: usize) -> Self {
        let usable = (width - PAD * 2.0 + GAP).max(CARD_W + GAP);
        Self {
            cols: ((usable / (CARD_W + GAP)).floor() as usize).max(1),
            items,
        }
    }

    pub fn rows(&self) -> usize {
        self.items.div_ceil(self.cols)
    }

    /// One grid row: a card plus the gap below it (the virtual list's row size).
    pub fn row_h() -> f32 {
        CARD_H + GAP
    }

    pub fn step(&self, index: usize, dir: Dir) -> Option<usize> {
        let (row, col) = (index / self.cols, index % self.cols);
        let next = match dir {
            Dir::Left if col > 0 => index - 1,
            Dir::Right if col + 1 < self.cols && index + 1 < self.items => index + 1,
            Dir::Up if row > 0 => index - self.cols,
            Dir::Down if index + self.cols < self.items => index + self.cols,
            Dir::Down if row + 1 < self.rows() => self.items - 1,
            _ => return None,
        };
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_and_edges() {
        let g = Grid::new(820.0, 10); // 4 cols, rows: 4,4,2
        assert_eq!(g.step(0, Dir::Right), Some(1));
        assert_eq!(g.step(3, Dir::Right), None);
        assert_eq!(g.step(5, Dir::Up), Some(1));
        assert_eq!(
            g.step(7, Dir::Down),
            Some(9),
            "short last row clamps to last item"
        );
        assert_eq!(g.step(0, Dir::Left), None);
    }
}
