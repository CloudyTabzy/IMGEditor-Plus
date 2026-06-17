//! Vendored design tokens, adapted from `iced_plus_tokens`.
//!
//! Self-contained — no external Iced dependency. Provides a complete
//! design system: color palette (primary/secondary/neutral/semantic),
//! spacing scale, radius scale, motion scale, typography scale, and
//! elevation (shadow) scale.

pub mod color;
pub mod elevation;
pub mod motion;
pub mod radius;
pub mod spacing;
pub mod tokens;
pub mod typography;

#[allow(unused_imports)]
pub use color::{Color, ColorPalette, SemanticColors, Shade};
pub use elevation::{Elevation, Shadow};
pub use tokens::{ThemeTokens};
