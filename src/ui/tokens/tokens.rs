//! Aggregate theme tokens structure.
use crate::ui::tokens::color::ColorPalette;
use crate::ui::tokens::elevation::ElevationScale;
use crate::ui::tokens::motion::MotionScale;
use crate::ui::tokens::radius::RadiusScale;
use crate::ui::tokens::spacing::SpacingScale;
use crate::ui::tokens::typography::TypographyScale;

/// Complete set of design tokens for a theme.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeTokens {
    pub colors: ColorPalette,
    pub typography: TypographyScale,
    pub spacing: SpacingScale,
    pub radius: RadiusScale,
    pub elevation: ElevationScale,
    pub motion: MotionScale,
}

impl Default for ThemeTokens {
    fn default() -> Self { Self::light() }
}

impl ThemeTokens {
    /// Light theme with a blue/indigo primary palette.
    pub fn light() -> Self {
        Self {
            colors: ColorPalette::default(),
            typography: TypographyScale::default(),
            spacing: SpacingScale::DEFAULT,
            radius: RadiusScale::DEFAULT,
            elevation: ElevationScale::default(),
            motion: MotionScale::default(),
        }
    }

    /// Dark theme with elevated neutrals and brighter accent.
    pub fn dark() -> Self {
        let mut t = Self::light();
        // In dark mode, the scale goes DARK → LIGHT, so s50 is the
        // page background and s900 is text on dark. The
        // `ColorScale::get` function still works the same way because
        // shades are just labels.
        t.colors.neutral = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x0A0A0A),  // s50: page
            crate::ui::tokens::color::Color::from_hex(0x171717),  // s100
            crate::ui::tokens::color::Color::from_hex(0x262626),  // s200: surface
            crate::ui::tokens::color::Color::from_hex(0x404040),  // s300
            crate::ui::tokens::color::Color::from_hex(0x525252),  // s400
            crate::ui::tokens::color::Color::from_hex(0x737373),  // s500
            crate::ui::tokens::color::Color::from_hex(0xA3A3A3),  // s600
            crate::ui::tokens::color::Color::from_hex(0xD4D4D4),  // s700
            crate::ui::tokens::color::Color::from_hex(0xE5E5E5),  // s800
            crate::ui::tokens::color::Color::from_hex(0xFAFAFA),  // s900: text
        );
        // Stronger shadows on dark mode
        t.elevation = {
            let strong = crate::ui::tokens::color::Color::new(0.0, 0.0, 0.0, 0.5);
            let soft = crate::ui::tokens::color::Color::new(0.0, 0.0, 0.0, 0.3);
            crate::ui::tokens::elevation::ElevationScale {
                flat: crate::ui::tokens::elevation::Elevation::FLAT,
                raised: crate::ui::tokens::elevation::Elevation::new(
                    crate::ui::tokens::elevation::Shadow::new(0.0, 1.0, 3.0, 0.0, soft),
                ),
                overlay: crate::ui::tokens::elevation::Elevation::new(
                    crate::ui::tokens::elevation::Shadow::new(0.0, 4.0, 6.0, -1.0, soft),
                ),
                floating: crate::ui::tokens::elevation::Elevation::new(
                    crate::ui::tokens::elevation::Shadow::new(0.0, 10.0, 15.0, -3.0, strong),
                ),
                modal: crate::ui::tokens::elevation::Elevation::new(
                    crate::ui::tokens::elevation::Shadow::new(0.0, 25.0, 50.0, -12.0, strong),
                ),
            }
        };
        t
    }

    #[inline]
    pub fn colors(&self) -> &ColorPalette { &self.colors }
    #[inline]
    pub fn typography(&self) -> &TypographyScale { &self.typography }
    #[inline]
    pub fn spacing(&self) -> &SpacingScale { &self.spacing }
    #[inline]
    pub fn radius(&self) -> &RadiusScale { &self.radius }
    #[inline]
    pub fn elevation(&self) -> &ElevationScale { &self.elevation }
    #[inline]
    pub fn motion(&self) -> &MotionScale { &self.motion }
}

/// Identifier for which preset is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemePresetKind {
    Light,
    Dark,
    Custom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ThemePreset {
    pub kind: ThemePresetKind,
    pub name: &'static str,
    pub tokens: ThemeTokens,
}

impl ThemePreset {
    pub const fn new(kind: ThemePresetKind, name: &'static str, tokens: ThemeTokens) -> Self {
        Self { kind, name, tokens }
    }
}
