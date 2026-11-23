//! Hero Detail view-model: tab ids, visibility and default tab, and the
//! hero-image precedence.
//!
//! Parity oracle: React `hero-detail-model.ts` and `AppShell.tsx` (tab
//! fallback). Runtime-only.
//!
//! ```
//! use crosshook_view_model::hero::HeroTab;
//! use crosshook_view_model::launch::LaunchMethod;
//!
//! assert_eq!(HeroTab::from_id("launch-options"), Some(HeroTab::LaunchOptions));
//! assert_eq!(HeroTab::resolve(Some("bogus"), LaunchMethod::Native, false), HeroTab::Overview);
//! ```

pub mod model;

pub use model::{
    display_path, resolve_hero_image, visible_tabs, HeroImage, HeroImageInputs, HeroTab,
    ImageSource,
};
