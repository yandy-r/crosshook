//! Cover art: off-thread JPEG decode + downscale, dominant colour, and a byte-capped LRU of
//! decoded `image::Handle`s.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use iced::widget::image::Handle;
use lru::LruCache;

/// Decoded thumbnails are 200x300 RGBA (240 KB each); 64 MiB holds ~270 covers.
pub const BUDGET_BYTES: usize = 64 * 1024 * 1024;
pub const THUMB_W: u32 = 200;
pub const THUMB_H: u32 = 300;

#[derive(Debug, Clone)]
pub struct Decoded {
    pub handle: Handle,
    pub bytes: usize,
    pub dominant: [u8; 3],
}

/// Byte-bounded LRU. The entry count bound is only a backstop.
pub struct CoverCache {
    lru: LruCache<String, Decoded>,
    bytes: usize,
    budget: usize,
    pub evictions: u64,
}

impl CoverCache {
    pub fn new(budget: usize) -> Self {
        Self {
            lru: LruCache::new(NonZeroUsize::new(100_000).expect("nonzero")),
            bytes: 0,
            budget,
            evictions: 0,
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&Decoded> {
        self.lru.get(key)
    }

    /// Read without touching recency (for `view`, which only has `&self`).
    pub fn peek(&self, key: &str) -> Option<&Decoded> {
        self.lru.peek(key)
    }

    #[cfg(test)]
    pub fn contains(&self, key: &str) -> bool {
        self.lru.contains(key)
    }

    pub fn insert(&mut self, key: String, value: Decoded) {
        self.bytes += value.bytes;
        if let Some(old) = self.lru.put(key, value) {
            self.bytes -= old.bytes;
        }
        while self.bytes > self.budget {
            let Some((_, evicted)) = self.lru.pop_lru() else {
                break;
            };
            self.bytes -= evicted.bytes;
            self.evictions += 1;
        }
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn len(&self) -> usize {
        self.lru.len()
    }
}

pub fn cover_path(data_home: &Path, app_id: &str) -> PathBuf {
    data_home
        .join("crosshook/cache/images")
        .join(app_id)
        .join("portrait_steam_cdn.jpg")
}

/// Blocking: decode, downscale and sample. Run on the blocking pool only.
pub fn decode(path: &Path) -> Result<Decoded, String> {
    let img = image::open(path).map_err(|e| format!("decode {}: {e}", path.display()))?;
    let thumb = img
        .resize_to_fill(THUMB_W, THUMB_H, image::imageops::FilterType::Triangle)
        .into_rgba8();
    let dominant = dominant_colour(thumb.as_raw());
    let (w, h) = thumb.dimensions();
    let raw = thumb.into_raw();
    let bytes = raw.len();
    Ok(Decoded {
        handle: Handle::from_rgba(w, h, raw),
        bytes,
        dominant,
    })
}

/// Average of a sparse sample of RGBA pixels (every 16th pixel).
pub fn dominant_colour(rgba: &[u8]) -> [u8; 3] {
    let (mut r, mut g, mut b, mut n) = (0u64, 0u64, 0u64, 0u64);
    for px in rgba.chunks_exact(4).step_by(16) {
        r += u64::from(px[0]);
        g += u64::from(px[1]);
        b += u64::from(px[2]);
        n += 1;
    }
    if n == 0 {
        return [0, 0, 0];
    }
    [(r / n) as u8, (g / n) as u8, (b / n) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(bytes: usize) -> Decoded {
        Decoded {
            handle: Handle::from_rgba(1, 1, vec![0u8; 4]),
            bytes,
            dominant: [0; 3],
        }
    }

    #[test]
    fn evicts_by_bytes_lru_first() {
        let mut cache = CoverCache::new(250);
        cache.insert("a".into(), entry(100));
        cache.insert("b".into(), entry(100));
        assert!(cache.get("a").is_some()); // a is now most recent
        cache.insert("c".into(), entry(100));
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
        assert!(cache.contains("c"));
        assert_eq!(cache.bytes(), 200);
        assert_eq!(cache.evictions, 1);
    }

    #[test]
    fn dominant_of_solid_colour() {
        let px: Vec<u8> = std::iter::repeat_n([10u8, 20, 30, 255], 64)
            .flatten()
            .collect();
        assert_eq!(dominant_colour(&px), [10, 20, 30]);
    }
}
