//! Bounded line ring buffer for the console (10k lines).

use std::collections::VecDeque;

pub const CAPACITY: usize = 10_000;

#[derive(Debug)]
pub struct Ring {
    lines: VecDeque<String>,
    capacity: usize,
    dropped: u64,
    total: u64,
}

impl Default for Ring {
    fn default() -> Self {
        Self::with_capacity(CAPACITY)
    }
}

impl Ring {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(capacity),
            capacity,
            dropped: 0,
            total: 0,
        }
    }

    pub fn extend(&mut self, batch: impl IntoIterator<Item = String>) {
        for line in batch {
            if self.lines.len() == self.capacity {
                self.lines.pop_front();
                self.dropped += 1;
            }
            self.lines.push_back(line);
            self.total += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    #[cfg(test)]
    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn get(&self, index: usize) -> Option<&str> {
        self.lines.get(index).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_and_counts_drops() {
        let mut ring = Ring::with_capacity(3);
        ring.extend((0..5).map(|i| i.to_string()));
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.dropped(), 2);
        assert_eq!(ring.total(), 5);
        assert_eq!(ring.get(0), Some("2"));
        assert_eq!(ring.get(2), Some("4"));
    }

    #[test]
    fn default_capacity_is_10k() {
        let mut ring = Ring::default();
        ring.extend((0..50_000).map(|i| i.to_string()));
        assert_eq!(ring.len(), CAPACITY);
        assert_eq!(ring.dropped(), 40_000);
    }
}
