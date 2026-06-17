//! Color palette with semantic roles.

use super::scale::{Color, ColorScale};

/// Complete color palette for a theme.
#[derive(Debug, Clone, PartialEq)]
pub struct ColorPalette {
    pub primary: ColorScale,
    pub secondary: ColorScale,
    pub neutral: ColorScale,
    pub semantic: SemanticColors,
}

impl ColorPalette {
    #[inline]
    pub const fn new(
        primary: ColorScale,
        secondary: ColorScale,
        neutral: ColorScale,
        semantic: SemanticColors,
    ) -> Self {
        Self { primary, secondary, neutral, semantic }
    }
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self {
            primary: default_primary(),
            secondary: default_secondary(),
            neutral: ColorScale::default(),
            semantic: SemanticColors::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SemanticColors {
    pub success: ColorScale,
    pub warning: ColorScale,
    pub destructive: ColorScale,
    pub info: ColorScale,
}

impl SemanticColors {
    #[inline]
    pub const fn new(
        success: ColorScale,
        warning: ColorScale,
        destructive: ColorScale,
        info: ColorScale,
    ) -> Self {
        Self { success, warning, destructive, info }
    }
}

impl Default for SemanticColors {
    fn default() -> Self {
        Self {
            success: default_success(),
            warning: default_warning(),
            destructive: default_destructive(),
            info: default_info(),
        }
    }
}

fn default_primary() -> ColorScale {
    ColorScale::new(
        Color::from_hex(0xEFF6FF), Color::from_hex(0xDBEAFE),
        Color::from_hex(0xBFDBFE), Color::from_hex(0x93C5FD),
        Color::from_hex(0x60A5FA), Color::from_hex(0x3B82F6),
        Color::from_hex(0x2563EB), Color::from_hex(0x1D4ED8),
        Color::from_hex(0x1E40AF), Color::from_hex(0x1E3A8A),
    )
}

fn default_secondary() -> ColorScale {
    ColorScale::new(
        Color::from_hex(0xF5F3FF), Color::from_hex(0xEDE9FE),
        Color::from_hex(0xDDD6FE), Color::from_hex(0xC4B5FD),
        Color::from_hex(0xA78BFA), Color::from_hex(0x8B5CF6),
        Color::from_hex(0x7C3AED), Color::from_hex(0x6D28D9),
        Color::from_hex(0x5B21B6), Color::from_hex(0x4C1D95),
    )
}

fn default_success() -> ColorScale {
    ColorScale::new(
        Color::from_hex(0xF0FDF4), Color::from_hex(0xDCFCE7),
        Color::from_hex(0xBBF7D0), Color::from_hex(0x86EFAC),
        Color::from_hex(0x4ADE80), Color::from_hex(0x22C55E),
        Color::from_hex(0x16A34A), Color::from_hex(0x15803D),
        Color::from_hex(0x166534), Color::from_hex(0x14532D),
    )
}

fn default_warning() -> ColorScale {
    ColorScale::new(
        Color::from_hex(0xFFFBEB), Color::from_hex(0xFEF3C7),
        Color::from_hex(0xFDE68A), Color::from_hex(0xFCD34D),
        Color::from_hex(0xFBBF24), Color::from_hex(0xF59E0B),
        Color::from_hex(0xD97706), Color::from_hex(0xB45309),
        Color::from_hex(0x92400E), Color::from_hex(0x78350F),
    )
}

fn default_destructive() -> ColorScale {
    ColorScale::new(
        Color::from_hex(0xFEF2F2), Color::from_hex(0xFEE2E2),
        Color::from_hex(0xFECACA), Color::from_hex(0xFCA5A5),
        Color::from_hex(0xF87171), Color::from_hex(0xEF4444),
        Color::from_hex(0xDC2626), Color::from_hex(0xB91C1C),
        Color::from_hex(0x991B1B), Color::from_hex(0x7F1D1D),
    )
}

fn default_info() -> ColorScale {
    ColorScale::new(
        Color::from_hex(0xECFEFF), Color::from_hex(0xCFFAFE),
        Color::from_hex(0xA5F3FC), Color::from_hex(0x67E8F9),
        Color::from_hex(0x22D3EE), Color::from_hex(0x06B6D4),
        Color::from_hex(0x0891B2), Color::from_hex(0x0E7490),
        Color::from_hex(0x155E75), Color::from_hex(0x164E63),
    )
}
