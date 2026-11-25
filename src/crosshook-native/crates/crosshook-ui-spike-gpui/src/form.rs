//! Gamescope form on the real core `GamescopeConfig` (re-exported by `core_bridge`): a text draft for the
//! numeric fields, pure validation, and conversion back to the core type for save.

use crate::core_bridge::{CoreGamescope as GamescopeConfig, GamescopeFilter};

/// Numeric fields, in display order. Indices are the `Target::Field(n)` numbers (0..=5 are
/// text inputs).
pub const NUMERIC: [&str; 6] = [
    "Internal width",
    "Internal height",
    "Output width",
    "Output height",
    "Frame-rate limit",
    "FSR sharpness",
];
pub const FILTERS: [Filter; 5] = [
    Filter(GamescopeFilter::Fsr),
    Filter(GamescopeFilter::Nis),
    Filter(GamescopeFilter::Linear),
    Filter(GamescopeFilter::Nearest),
    Filter(GamescopeFilter::Pixel),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Filter(pub GamescopeFilter);

impl std::fmt::Display for Filter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self.0 {
            GamescopeFilter::Fsr => "FSR",
            GamescopeFilter::Nis => "NIS",
            GamescopeFilter::Linear => "Linear",
            GamescopeFilter::Nearest => "Nearest",
            GamescopeFilter::Pixel => "Pixel",
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Draft {
    pub base: GamescopeConfig,
    pub numbers: [String; 6],
    pub optimization: bool,
}

fn opt(v: Option<impl ToString>) -> String {
    v.map(|v| v.to_string()).unwrap_or_default()
}

impl Draft {
    pub fn from_config(base: GamescopeConfig) -> Self {
        let numbers = [
            opt(base.internal_width),
            opt(base.internal_height),
            opt(base.output_width),
            opt(base.output_height),
            opt(base.frame_rate_limit),
            opt(base.fsr_sharpness),
        ];
        Self {
            base,
            numbers,
            optimization: false,
        }
    }

    /// Per-field error, index-aligned with `NUMERIC`.
    pub fn errors(&self) -> [Option<String>; 6] {
        let mut out: [Option<String>; 6] = Default::default();
        let ranges = [
            (1, 16_384),
            (1, 16_384),
            (1, 16_384),
            (1, 16_384),
            (1, 1000),
            (0, 20),
        ];
        for (i, raw) in self.numbers.iter().enumerate() {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            let (lo, hi) = ranges[i];
            out[i] = match raw.parse::<u32>() {
                Ok(v) if (lo..=hi).contains(&v) => None,
                Ok(_) => Some(format!("Must be {lo}–{hi}")),
                Err(_) => Some("Whole number only".into()),
            };
        }
        for (w, h) in [(0usize, 1usize), (2, 3)] {
            let (we, he) = (
                self.numbers[w].trim().is_empty(),
                self.numbers[h].trim().is_empty(),
            );
            if we != he && out[w].is_none() && out[h].is_none() {
                out[if we { w } else { h }] = Some("Width and height go together".into());
            }
        }
        out
    }

    pub fn is_valid(&self) -> bool {
        self.errors().iter().all(Option::is_none)
    }

    pub fn to_config(&self) -> Option<GamescopeConfig> {
        if !self.is_valid() {
            return None;
        }
        let num = |i: usize| self.numbers[i].trim().parse::<u32>().ok();
        let mut cfg = self.base.clone();
        cfg.internal_width = num(0);
        cfg.internal_height = num(1);
        cfg.output_width = num(2);
        cfg.output_height = num(3);
        cfg.frame_rate_limit = num(4);
        cfg.fsr_sharpness = num(5).and_then(|v| u8::try_from(v).ok());
        Some(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_ranges_and_pairs() {
        let mut d = Draft::from_config(GamescopeConfig::default());
        assert!(d.is_valid());
        d.numbers[0] = "1920".into();
        assert_eq!(
            d.errors()[1].as_deref(),
            Some("Width and height go together")
        );
        d.numbers[1] = "1080".into();
        assert!(d.is_valid());
        d.numbers[4] = "0".into();
        assert_eq!(d.errors()[4].as_deref(), Some("Must be 1–1000"));
        d.numbers[4] = "abc".into();
        assert_eq!(d.errors()[4].as_deref(), Some("Whole number only"));
        assert!(d.to_config().is_none());
    }

    #[test]
    fn round_trips_through_core_type() {
        let base = GamescopeConfig {
            internal_width: Some(1280),
            internal_height: Some(800),
            fsr_sharpness: Some(5),
            ..GamescopeConfig::default()
        };
        let d = Draft::from_config(base.clone());
        assert_eq!(d.to_config(), Some(base));
    }
}
