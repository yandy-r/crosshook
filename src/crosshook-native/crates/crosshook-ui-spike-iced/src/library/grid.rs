//! Grid geometry for viewport culling: column count, visible row range, spatial steps and
//! scroll-to-reveal. Pure; the view builds widgets only for `visible_items`.

use crate::focus::Dir;

pub const CARD_W: f32 = 160.0;
pub const CARD_H: f32 = 270.0;
pub const GAP: f32 = 12.0;
pub const PAD: f32 = 12.0;
pub const HEADER_H: f32 = 56.0;
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

    pub fn row_h() -> f32 {
        CARD_H + GAP
    }

    /// Visible rows (with overscan) as `(first_row, last_row_exclusive)`.
    pub fn visible_rows(&self, scroll_y: f32, view_h: f32) -> (usize, usize) {
        let top = (scroll_y - HEADER_H - PAD).max(0.0);
        let first = (top / Self::row_h()).floor() as usize;
        let last = ((scroll_y + view_h - HEADER_H - PAD).max(0.0) / Self::row_h()).ceil() as usize;
        (
            first.saturating_sub(OVERSCAN_ROWS),
            (last + OVERSCAN_ROWS).min(self.rows()),
        )
    }

    pub fn visible_items(&self, scroll_y: f32, view_h: f32) -> (usize, usize) {
        let (a, b) = self.visible_rows(scroll_y, view_h);
        (a * self.cols, (b * self.cols).min(self.items))
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

    /// New scroll offset if card `index` is not fully visible.
    pub fn scroll_to_reveal(&self, index: usize, scroll_y: f32, view_h: f32) -> Option<f32> {
        let top = HEADER_H + PAD + (index / self.cols) as f32 * Self::row_h();
        let bottom = top + CARD_H;
        if top < scroll_y {
            Some((top - PAD).max(0.0))
        } else if bottom > scroll_y + view_h {
            Some(bottom - view_h + PAD)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn culls_to_visible_rows() {
        let g = Grid::new(820.0, 500);
        assert_eq!(g.cols, 4);
        let (a, b) = g.visible_items(0.0, 900.0);
        assert_eq!(a, 0);
        assert!(b <= 5 * 4, "only ~4 rows + overscan, got {b}");
        let (a, _) = g.visible_items(10_000.0, 900.0);
        assert!(a > 100);
    }

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

    #[test]
    fn reveal_scrolls_only_when_needed() {
        let g = Grid::new(820.0, 500);
        assert_eq!(g.scroll_to_reveal(0, 0.0, 900.0), None);
        assert!(g.scroll_to_reveal(40, 0.0, 900.0).is_some());
    }
}
