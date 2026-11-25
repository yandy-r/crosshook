//! Back/forward navigation history. In the Elm model this is a plain value in app state:
//! a Vec plus a cursor. Navigating truncates the forward branch.

#[derive(Debug, Clone)]
pub struct History<T> {
    entries: Vec<T>,
    cursor: usize,
    limit: usize,
}

impl<T: Clone + PartialEq> History<T> {
    pub fn new(start: T, limit: usize) -> Self {
        Self {
            entries: vec![start],
            cursor: 0,
            limit: limit.max(2),
        }
    }

    pub fn current(&self) -> &T {
        &self.entries[self.cursor]
    }

    pub fn push(&mut self, route: T) {
        if *self.current() == route {
            return;
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push(route);
        if self.entries.len() > self.limit {
            self.entries.remove(0);
        }
        self.cursor = self.entries.len() - 1;
    }

    pub fn can_back(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_forward(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }

    pub fn back(&mut self) -> Option<&T> {
        self.can_back().then(|| {
            self.cursor -= 1;
            &self.entries[self.cursor]
        })
    }

    pub fn forward(&mut self) -> Option<&T> {
        self.can_forward().then(|| {
            self.cursor += 1;
            &self.entries[self.cursor]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_forward_and_branch_truncation() {
        let mut h = History::new(1, 50);
        h.push(2);
        h.push(3);
        assert_eq!(h.back(), Some(&2));
        assert_eq!(h.back(), Some(&1));
        assert_eq!(h.back(), None);
        assert_eq!(h.forward(), Some(&2));
        h.push(9);
        assert!(!h.can_forward());
        assert_eq!(h.back(), Some(&2));
    }

    #[test]
    fn duplicate_push_is_ignored_and_limit_holds() {
        let mut h = History::new(0, 2);
        h.push(0);
        assert!(!h.can_back());
        h.push(1);
        h.push(2);
        assert_eq!(h.back(), Some(&1));
        assert_eq!(h.back(), None);
    }
}
