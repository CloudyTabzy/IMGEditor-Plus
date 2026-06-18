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

    /// Everforest dark theme: muted green-grey neutrals with sage accents.
    pub fn everforest() -> Self {
        let mut t = Self::light();
        t.colors.neutral = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x232A2E),  // s50: page
            crate::ui::tokens::color::Color::from_hex(0x2D353B),  // s100
            crate::ui::tokens::color::Color::from_hex(0x343F44),  // s200: surface
            crate::ui::tokens::color::Color::from_hex(0x3D484D),  // s300
            crate::ui::tokens::color::Color::from_hex(0x475258),  // s400
            crate::ui::tokens::color::Color::from_hex(0x859289),  // s500
            crate::ui::tokens::color::Color::from_hex(0x9DA9A0),  // s600
            crate::ui::tokens::color::Color::from_hex(0xD3C6AA),  // s700
            crate::ui::tokens::color::Color::from_hex(0xE7DFCF),  // s800
            crate::ui::tokens::color::Color::from_hex(0xF3EFDF),  // s900: text
        );
        t.colors.primary = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x3C4841),
            crate::ui::tokens::color::Color::from_hex(0x4A574D),
            crate::ui::tokens::color::Color::from_hex(0x5E6E60),
            crate::ui::tokens::color::Color::from_hex(0x7A8E74),
            crate::ui::tokens::color::Color::from_hex(0xA7C080),
            crate::ui::tokens::color::Color::from_hex(0xB6CC94),
            crate::ui::tokens::color::Color::from_hex(0xC5D9A8),
            crate::ui::tokens::color::Color::from_hex(0xD4E6BC),
            crate::ui::tokens::color::Color::from_hex(0xE3F3D0),
            crate::ui::tokens::color::Color::from_hex(0xF2FFE4),
        );
        t.colors.semantic.success = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x2B3F36),
            crate::ui::tokens::color::Color::from_hex(0x355244),
            crate::ui::tokens::color::Color::from_hex(0x426652),
            crate::ui::tokens::color::Color::from_hex(0x587C66),
            crate::ui::tokens::color::Color::from_hex(0x83C092),
            crate::ui::tokens::color::Color::from_hex(0x96CFA5),
            crate::ui::tokens::color::Color::from_hex(0xA9DDB8),
            crate::ui::tokens::color::Color::from_hex(0xBCEBCB),
            crate::ui::tokens::color::Color::from_hex(0xCFF9DE),
            crate::ui::tokens::color::Color::from_hex(0xE2FFF1),
        );
        t.colors.semantic.warning = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x443C2E),
            crate::ui::tokens::color::Color::from_hex(0x554A36),
            crate::ui::tokens::color::Color::from_hex(0x66583E),
            crate::ui::tokens::color::Color::from_hex(0x8A7A5A),
            crate::ui::tokens::color::Color::from_hex(0xDBBC7F),
            crate::ui::tokens::color::Color::from_hex(0xE5CC99),
            crate::ui::tokens::color::Color::from_hex(0xEFDDB3),
            crate::ui::tokens::color::Color::from_hex(0xF9EDCD),
            crate::ui::tokens::color::Color::from_hex(0xFFFBE7),
            crate::ui::tokens::color::Color::from_hex(0xFFFFFF),
        );
        t.colors.semantic.destructive = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x4A3436),
            crate::ui::tokens::color::Color::from_hex(0x5E3F41),
            crate::ui::tokens::color::Color::from_hex(0x724A4C),
            crate::ui::tokens::color::Color::from_hex(0x9A6868),
            crate::ui::tokens::color::Color::from_hex(0xE67E80),
            crate::ui::tokens::color::Color::from_hex(0xED9798),
            crate::ui::tokens::color::Color::from_hex(0xF4B0B0),
            crate::ui::tokens::color::Color::from_hex(0xFBC9C9),
            crate::ui::tokens::color::Color::from_hex(0xFFE2E2),
            crate::ui::tokens::color::Color::from_hex(0xFFFFFF),
        );
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
