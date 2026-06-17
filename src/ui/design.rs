//! Bridge between vendored design tokens and Iced 0.14.
//!
//! Provides:
//! - `DesignSystem` — the per-window design system wrapping a `ThemeTokens`
//!   (selected from the active Iced `Theme`) and exposing semantic helpers
//!   like `surface()`, `accent()`, `destructive()`, etc.
//! - `Design` — a lightweight value struct the view layer can hold and
//!   pass around to build consistent styles.
//!
//! Unlike `iced_plus_theme`, we do not duplicate Iced's style Catalog
//! traits; we expose small helper functions (`surface_container`,
//! `text_color`, `shadow_for`) that produce the right `iced::Color`,
//! `iced::Border`, `iced::Shadow`, and `iced::Background` values for
//! the existing widget style functions.

use iced::Color;

use crate::ui::tokens::{
    radius::RadiusScale, spacing::SpacingScale, Color as TokenColor, ColorPalette, Elevation,
    Shade, Shadow, ThemeTokens,
};

/// Convert a design-token [`TokenColor`] to an [`iced::Color`].
#[inline]
pub fn to_iced(c: TokenColor) -> Color {
    Color::from_rgba(c.r, c.g, c.b, c.a)
}

/// Convert with alpha override.
#[inline]
pub fn to_iced_alpha(c: TokenColor, a: f32) -> Color {
    Color::from_rgba(c.r, c.g, c.b, a)
}

/// Lightweight design-system view over a frozen `ThemeTokens`.
#[derive(Debug, Clone)]
pub struct Design {
    pub tokens: ThemeTokens,
    pub is_dark: bool,
}

impl Design {
    /// Construct from a frozen `ThemeTokens`. `is_dark` tells the bridge
    /// whether to bias neutrals toward dark (true) or light (false).
    pub fn from_tokens(tokens: ThemeTokens, is_dark: bool) -> Self {
        Self { tokens, is_dark }
    }

    /// Light variant.
    pub fn light() -> Self { Self::from_tokens(ThemeTokens::light(), false) }
    /// Dark variant.
    pub fn dark() -> Self { Self::from_tokens(ThemeTokens::dark(), true) }

    // ----- Colors -------------------------------------------------------

    pub fn palette(&self) -> &ColorPalette { &self.tokens.colors }
    pub fn spacing(&self) -> &SpacingScale { &self.tokens.spacing }
    pub fn radius(&self) -> &RadiusScale { &self.tokens.radius }

    /// Page background — neutral 50 (light) or 50 (dark).
    pub fn page(&self) -> Color {
        to_iced(self.palette().neutral.get(Shade::S50))
    }

    /// Surface (cards, panes) — one step above `page`.
    pub fn surface(&self) -> Color {
        if self.is_dark {
            to_iced(self.palette().neutral.get(Shade::S200))
        } else {
            TokenColor::WHITE.into_iced()
        }
    }

    /// Subtle surface variant (for list rows, table headers, etc.).
    pub fn surface_subtle(&self) -> Color {
        if self.is_dark {
            to_iced(self.palette().neutral.get(Shade::S300))
        } else {
            to_iced(self.palette().neutral.get(Shade::S50))
        }
    }

    /// Border / divider color.
    pub fn border(&self) -> Color {
        if self.is_dark {
            to_iced(self.palette().neutral.get(Shade::S300))
        } else {
            to_iced(self.palette().neutral.get(Shade::S200))
        }
    }

    /// Primary text color.
    pub fn text(&self) -> Color {
        if self.is_dark {
            to_iced(self.palette().neutral.get(Shade::S900))
        } else {
            to_iced(self.palette().neutral.get(Shade::S900))
        }
    }

    /// Muted text.
    pub fn text_muted(&self) -> Color {
        if self.is_dark {
            to_iced(self.palette().neutral.get(Shade::S600))
        } else {
            to_iced(self.palette().neutral.get(Shade::S500))
        }
    }

    /// Strong accent (menubar hover, active tab, primary button).
    pub fn accent(&self) -> Color { to_iced(self.palette().primary.get(Shade::S500)) }
    pub fn accent_hover(&self) -> Color { to_iced(self.palette().primary.get(Shade::S600)) }
    pub fn accent_pressed(&self) -> Color { to_iced(self.palette().primary.get(Shade::S700)) }
    pub fn accent_weak(&self) -> Color { to_iced(self.palette().primary.get(Shade::S100)) }
    pub fn accent_text(&self) -> Color {
        // Text-on-accent should be near-white in both modes.
        TokenColor::WHITE.into_iced()
    }

    pub fn success(&self) -> Color { to_iced(self.palette().semantic.success.get(Shade::S500)) }
    pub fn warning(&self) -> Color { to_iced(self.palette().semantic.warning.get(Shade::S500)) }
    pub fn destructive(&self) -> Color { to_iced(self.palette().semantic.destructive.get(Shade::S500)) }
    pub fn info(&self) -> Color { to_iced(self.palette().semantic.info.get(Shade::S500)) }

    /// Selection row background (translucent accent).
    pub fn selection_bg(&self) -> Color {
        let base = self.palette().primary.get(Shade::S500);
        to_iced(base.fade(0.18))
    }

    /// Hover overlay (sits on top of any surface).
    pub fn hover_overlay(&self) -> Color {
        let shade = if self.is_dark { Shade::S900 } else { Shade::S900 };
        let accent = self.palette().neutral.get(shade);
        to_iced(accent.fade(0.06))
    }

    // ----- Gradients ---------------------------------------------------

    /// Two-stop gradient for the menubar / toolbar background.
    pub fn menubar_gradient(&self) -> (Color, Color) {
        let a = if self.is_dark {
            to_iced(self.palette().neutral.get(Shade::S800))
        } else {
            to_iced(self.palette().neutral.get(Shade::S50))
        };
        let b = if self.is_dark {
            to_iced(self.palette().neutral.get(Shade::S700))
        } else {
            to_iced(self.palette().neutral.get(Shade::S100))
        };
        (a, b)
    }

    /// Accent gradient for the active tab / progress bar.
    pub fn accent_gradient(&self) -> (Color, Color) {
        (
            to_iced(self.palette().primary.get(Shade::S400)),
            to_iced(self.palette().primary.get(Shade::S600)),
        )
    }

    /// Status-bar gradient on save (success tint).
    pub fn success_gradient(&self) -> (Color, Color) {
        (
            to_iced(self.palette().semantic.success.get(Shade::S400)),
            to_iced(self.palette().semantic.success.get(Shade::S600)),
        )
    }

    // ----- Spacing / Radius --------------------------------------------

    /// Standard component padding in Iced's `Padding` form.
    pub fn padding(&self, size: f32) -> iced::Padding {
        iced::Padding {
            top: size,
            bottom: size,
            left: size * 1.5,
            right: size * 1.5,
        }
    }

    // ----- Elevation ---------------------------------------------------

    /// Convert our `Shadow` to Iced's `iced::Shadow`.
    pub fn iced_shadow(&self, e: &Elevation) -> iced::Shadow {
        let s: Shadow = e.shadow;
        iced::Shadow {
            color: to_iced_alpha(s.color, s.color.a),
            offset: iced::Vector::new(s.offset_x, s.offset_y),
            blur_radius: s.blur,
        }
    }

    /// Border for a given elevation.
    pub fn iced_border(&self, e: &Elevation) -> iced::Border {
        iced::Border {
            color: to_iced(e.border_color),
            width: e.border_width,
            radius: self.radius().lg().into(),
        }
    }
}

// ----- TokenColor -> iced::Color convenience impl -----------------------

impl TokenColor {
    /// Convert to `iced::Color` via the bridge.
    #[inline]
    pub fn into_iced(self) -> Color { to_iced(self) }
}

// ----- Convenience: pick design from an iced Theme ----------------------

/// Build a `Design` whose token palette matches the active Iced `Theme`.
pub fn design_for_theme(theme: &iced::Theme) -> Design {
    // Detect light vs dark by reading the palette's `is_dark()`.
    let is_dark = theme.extended_palette().is_dark;
    if is_dark {
        Design::dark()
    } else {
        Design::light()
    }
}

// ----- Test-only helpers ------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_dark_constructors() {
        let l = Design::light();
        let d = Design::dark();
        // Surface is near-white in light, near-black in dark.
        assert!(l.surface().r > 0.95);
        assert!(d.surface().r < 0.30);
    }

    #[test]
    fn accent_text_is_white_in_both_modes() {
        let l = Design::light();
        let d = Design::dark();
        assert_eq!(l.accent_text(), d.accent_text());
        assert!(l.accent_text().r > 0.99);
    }

    #[test]
    fn spacing_and_radius_default() {
        let d = Design::light();
        assert!((d.spacing().lg() - 16.0).abs() < f32::EPSILON);
        assert!((d.radius().md() - 6.0).abs() < f32::EPSILON);
    }

    #[test]
    fn to_iced_round_trip() {
        let c = TokenColor::from_hex(0x3366FF);
        let iced = to_iced(c);
        assert!((iced.r - 0x33 as f32 / 255.0).abs() < 0.01);
        assert!((iced.g - 0x66 as f32 / 255.0).abs() < 0.01);
        assert!((iced.b - 1.0).abs() < 0.01);
        assert!((iced.a - 1.0).abs() < 0.01);
    }
}
