//! Cover art: off-thread JPEG decode + downscale into a gpui `RenderImage` (BGRA), dominant
//! colour, and a byte-capped LRU. Evicted images are handed back so the caller can release
//! their GPU atlas tiles with `Window::drop_image` (gpui keeps them otherwise).

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::RenderImage;
use image::Frame;
use lru::LruCache;
use smallvec::smallvec;

/// Decoded thumbnails are 200x300 BGRA (240 KB each); 64 MiB holds ~270 covers.
pub const BUDGET_BYTES: usize = 64 * 1024 * 1024;
pub const THUMB_W: u32 = 200;
pub const THUMB_H: u32 = 300;

#[derive(Clone)]
pub struct Decoded {
    pub image: Arc<RenderImage>,
    pub bytes: usize,
    /// Backdrop colour drawn behind the cover image.
    pub dominant: [u8; 3],
}

/// Byte-bounded LRU. The entry count bound is only a backstop.
pub struct CoverCache {
    lru: LruCache<String, Decoded>,
    bytes: usize,
    budget: usize,
    tints: std::collections::HashMap<String, [u8; 3]>,
    pub evictions: u64,
}

impl CoverCache {
    pub fn new(budget: usize) -> Self {
        Self {
            lru: LruCache::new(NonZeroUsize::new(100_000).expect("nonzero")),
            bytes: 0,
            budget,
            tints: std::collections::HashMap::new(),
            evictions: 0,
        }
    }

    pub fn get(&mut self, key: &str) -> Option<&Decoded> {
        self.lru.get(key)
    }

    /// Dominant colour of a cover decoded earlier (kept after eviction for the placeholder).
    pub fn peek_dominant(&self, key: &str) -> Option<[u8; 3]> {
        self.lru
            .peek(key)
            .map(|d| d.dominant)
            .or_else(|| self.tints.get(key).copied())
    }

    pub fn contains(&self, key: &str) -> bool {
        self.lru.contains(key)
    }

    /// Inserts and returns the images evicted to stay under budget.
    pub fn insert(&mut self, key: String, value: Decoded) -> Vec<Arc<RenderImage>> {
        let mut evicted = Vec::new();
        self.bytes += value.bytes;
        self.tints.insert(key.clone(), value.dominant);
        if let Some(old) = self.lru.put(key, value) {
            self.bytes -= old.bytes;
            evicted.push(old.image);
        }
        while self.bytes > self.budget {
            let Some((_, gone)) = self.lru.pop_lru() else {
                break;
            };
            self.bytes -= gone.bytes;
            self.evictions += 1;
            evicted.push(gone.image);
        }
        evicted
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

/// Blocking: decode, downscale, sample and convert to BGRA. Runs on gpui's background executor.
pub fn decode(path: &Path) -> Result<Decoded, String> {
    let img = image::open(path).map_err(|e| format!("decode {}: {e}", path.display()))?;
    let mut thumb = img
        .resize_to_fill(THUMB_W, THUMB_H, image::imageops::FilterType::Triangle)
        .into_rgba8();
    let dominant = dominant_colour(thumb.as_raw());
    for px in thumb.pixels_mut() {
        px.0.swap(0, 2); // RGBA -> BGRA, what RenderImage expects
    }
    let bytes = thumb.as_raw().len();
    Ok(Decoded {
        image: Arc::new(RenderImage::new(smallvec![Frame::new(thumb)])),
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
    use image::RgbaImage;

    fn entry(bytes: usize) -> Decoded {
        Decoded {
            image: Arc::new(RenderImage::new(smallvec![Frame::new(RgbaImage::new(
                1, 1
            ))])),
            bytes,
            dominant: [0; 3],
        }
    }

    #[test]
    fn evicts_by_bytes_lru_first() {
        let mut cache = CoverCache::new(250);
        assert!(cache.insert("a".into(), entry(100)).is_empty());
        cache.insert("b".into(), entry(100));
        assert!(cache.get("a").is_some());
        let evicted = cache.insert("c".into(), entry(100));
        assert_eq!(evicted.len(), 1, "evicted image handed back for drop_image");
        assert!(cache.contains("a") && !cache.contains("b") && cache.contains("c"));
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
