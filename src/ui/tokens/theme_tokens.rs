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
    fn default() -> Self {
        Self::light()
    }
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
            crate::ui::tokens::color::Color::from_hex(0x121216), // s50: page (lifted from near-black so the empty workspace doesn't read as pitch black)
            crate::ui::tokens::color::Color::from_hex(0x1A1A1E), // s100
            crate::ui::tokens::color::Color::from_hex(0x262626), // s200: surface
            crate::ui::tokens::color::Color::from_hex(0x404040), // s300
            crate::ui::tokens::color::Color::from_hex(0x525252), // s400
            crate::ui::tokens::color::Color::from_hex(0x737373), // s500
            crate::ui::tokens::color::Color::from_hex(0xA3A3A3), // s600
            crate::ui::tokens::color::Color::from_hex(0xD4D4D4), // s700
            crate::ui::tokens::color::Color::from_hex(0xE5E5E5), // s800
            crate::ui::tokens::color::Color::from_hex(0xFAFAFA), // s900: text
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
            crate::ui::tokens::color::Color::from_hex(0x232A2E), // s50: page
            crate::ui::tokens::color::Color::from_hex(0x2D353B), // s100
            crate::ui::tokens::color::Color::from_hex(0x343F44), // s200: surface
            crate::ui::tokens::color::Color::from_hex(0x3D484D), // s300
            crate::ui::tokens::color::Color::from_hex(0x475258), // s400
            crate::ui::tokens::color::Color::from_hex(0x859289), // s500
            crate::ui::tokens::color::Color::from_hex(0x9DA9A0), // s600
            crate::ui::tokens::color::Color::from_hex(0xD3C6AA), // s700
            crate::ui::tokens::color::Color::from_hex(0xE7DFCF), // s800
            crate::ui::tokens::color::Color::from_hex(0xF3EFDF), // s900: text
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

    /// GitHub dark default: near-black blue-tinted canvas with the blue
    /// accent (Primer dark: canvas #0D1117, subtle #161B22, border #30363D,
    /// fg #E6EDF3, accent #2F81F7/#1F6FEB).
    pub fn github_dark() -> Self {
        let mut t = Self::light();
        t.colors.neutral = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x0D1117), // s50: page (canvas.default)
            crate::ui::tokens::color::Color::from_hex(0x12171E), // s100: chrome
            crate::ui::tokens::color::Color::from_hex(0x161B22), // s200: surface (canvas.subtle)
            crate::ui::tokens::color::Color::from_hex(0x30363D), // s300 (border.default)
            crate::ui::tokens::color::Color::from_hex(0x444C56), // s400
            crate::ui::tokens::color::Color::from_hex(0x656C76), // s500
            crate::ui::tokens::color::Color::from_hex(0x7D8590), // s600 (fg.muted)
            crate::ui::tokens::color::Color::from_hex(0x9EA7B3), // s700
            crate::ui::tokens::color::Color::from_hex(0xC9D1D9), // s800
            crate::ui::tokens::color::Color::from_hex(0xE6EDF3), // s900: text (fg.default)
        );
        t.colors.primary = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x0A1F42),
            crate::ui::tokens::color::Color::from_hex(0x0F2D57),
            crate::ui::tokens::color::Color::from_hex(0x143D7A),
            crate::ui::tokens::color::Color::from_hex(0x1A56AD),
            crate::ui::tokens::color::Color::from_hex(0x1F6FEB), // s400 (btn.primary.bg)
            crate::ui::tokens::color::Color::from_hex(0x2F81F7), // s500: accent
            crate::ui::tokens::color::Color::from_hex(0x4493F8), // s600 (accent.fg)
            crate::ui::tokens::color::Color::from_hex(0x6CAEFF),
            crate::ui::tokens::color::Color::from_hex(0x9CCBFF),
            crate::ui::tokens::color::Color::from_hex(0xCCE5FF),
        );
        t.colors.semantic.success = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x0D2417),
            crate::ui::tokens::color::Color::from_hex(0x123321),
            crate::ui::tokens::color::Color::from_hex(0x17462D),
            crate::ui::tokens::color::Color::from_hex(0x238636), // s300 (success.emphasis)
            crate::ui::tokens::color::Color::from_hex(0x2EA043),
            crate::ui::tokens::color::Color::from_hex(0x3FB950), // s500: success.fg
            crate::ui::tokens::color::Color::from_hex(0x56D364),
            crate::ui::tokens::color::Color::from_hex(0x7EE787),
            crate::ui::tokens::color::Color::from_hex(0xA4F0B0),
            crate::ui::tokens::color::Color::from_hex(0xCCF5DB),
        );
        t.colors.semantic.warning = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x221A0B),
            crate::ui::tokens::color::Color::from_hex(0x34280F),
            crate::ui::tokens::color::Color::from_hex(0x4D3A14),
            crate::ui::tokens::color::Color::from_hex(0x9E6A03), // s300 (attention.emphasis)
            crate::ui::tokens::color::Color::from_hex(0xBB8009),
            crate::ui::tokens::color::Color::from_hex(0xD29922), // s500: attention.fg
            crate::ui::tokens::color::Color::from_hex(0xE3B341),
            crate::ui::tokens::color::Color::from_hex(0xEAC54F),
            crate::ui::tokens::color::Color::from_hex(0xF2D887),
            crate::ui::tokens::color::Color::from_hex(0xF9E6B3),
        );
        t.colors.semantic.destructive = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x2D1212),
            crate::ui::tokens::color::Color::from_hex(0x401B1A),
            crate::ui::tokens::color::Color::from_hex(0x5C2624),
            crate::ui::tokens::color::Color::from_hex(0x8E2E2B),
            crate::ui::tokens::color::Color::from_hex(0xDA3633), // s400 (danger.emphasis)
            crate::ui::tokens::color::Color::from_hex(0xF85149), // s500: danger.fg
            crate::ui::tokens::color::Color::from_hex(0xFF7B72),
            crate::ui::tokens::color::Color::from_hex(0xFFA198),
            crate::ui::tokens::color::Color::from_hex(0xFFC1BA),
            crate::ui::tokens::color::Color::from_hex(0xFFE0DB),
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

    /// Ayu Dark (ayutheme.com) adjusted toward a deep terracotta accent on a
    /// near-black navy canvas: bg #0B0E14, panel #131722, border #1E2430,
    /// fg #E9EAEE with warm-grey muted #8A919E, accent #F29A4B (Ayu's
    /// #FF9940 pulled slightly earthier), Ayu greens/yellows/reds/cyans for
    /// the semantic scales.
    pub fn ayu_dark() -> Self {
        let mut t = Self::light();
        t.colors.neutral = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x0B0E14), // s50: page (Ayu bg)
            crate::ui::tokens::color::Color::from_hex(0x0F1219), // s100: chrome
            crate::ui::tokens::color::Color::from_hex(0x131722), // s200: surface (panel)
            crate::ui::tokens::color::Color::from_hex(0x1E2430), // s300 (border)
            crate::ui::tokens::color::Color::from_hex(0x2A3140), // s400
            crate::ui::tokens::color::Color::from_hex(0x565F6E), // s500
            crate::ui::tokens::color::Color::from_hex(0x8A919E), // s600 (muted fg)
            crate::ui::tokens::color::Color::from_hex(0xA9AFBA), // s700
            crate::ui::tokens::color::Color::from_hex(0xC6CAD3), // s800
            crate::ui::tokens::color::Color::from_hex(0xE9EAEE), // s900: text
        );
        t.colors.primary = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x33200D),
            crate::ui::tokens::color::Color::from_hex(0x55361A),
            crate::ui::tokens::color::Color::from_hex(0x7A4C22),
            crate::ui::tokens::color::Color::from_hex(0xB06A2E),
            crate::ui::tokens::color::Color::from_hex(0xE0863C), // s400
            crate::ui::tokens::color::Color::from_hex(0xF29A4B), // s500: accent
            crate::ui::tokens::color::Color::from_hex(0xFFAB60), // s600 (hover)
            crate::ui::tokens::color::Color::from_hex(0xFFB878), // s700 (pressed)
            crate::ui::tokens::color::Color::from_hex(0xFFCD9F),
            crate::ui::tokens::color::Color::from_hex(0xFFE3C7),
        );
        t.colors.semantic.success = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x1E2A12),
            crate::ui::tokens::color::Color::from_hex(0x2B3B19),
            crate::ui::tokens::color::Color::from_hex(0x3B501F),
            crate::ui::tokens::color::Color::from_hex(0x77A433),
            crate::ui::tokens::color::Color::from_hex(0x97C53F),
            crate::ui::tokens::color::Color::from_hex(0xAAD94C), // s500: Ayu green
            crate::ui::tokens::color::Color::from_hex(0xBCE272),
            crate::ui::tokens::color::Color::from_hex(0xCDEA97),
            crate::ui::tokens::color::Color::from_hex(0xDEF1BD),
            crate::ui::tokens::color::Color::from_hex(0xF0F8E2),
        );
        t.colors.semantic.warning = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x2C2410),
            crate::ui::tokens::color::Color::from_hex(0x3E3317),
            crate::ui::tokens::color::Color::from_hex(0x55451D),
            crate::ui::tokens::color::Color::from_hex(0x9C7B2C),
            crate::ui::tokens::color::Color::from_hex(0xC99C3D),
            crate::ui::tokens::color::Color::from_hex(0xE6B450), // s500: Ayu yellow
            crate::ui::tokens::color::Color::from_hex(0xEDC46E),
            crate::ui::tokens::color::Color::from_hex(0xF3D28B),
            crate::ui::tokens::color::Color::from_hex(0xF9E2A9),
            crate::ui::tokens::color::Color::from_hex(0xFDF1C8),
        );
        t.colors.semantic.destructive = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x2E1417),
            crate::ui::tokens::color::Color::from_hex(0x411A1E),
            crate::ui::tokens::color::Color::from_hex(0x592226),
            crate::ui::tokens::color::Color::from_hex(0xA53E44),
            crate::ui::tokens::color::Color::from_hex(0xD95F65),
            crate::ui::tokens::color::Color::from_hex(0xF07178), // s500: Ayu red
            crate::ui::tokens::color::Color::from_hex(0xF48B91),
            crate::ui::tokens::color::Color::from_hex(0xF7A5AB),
            crate::ui::tokens::color::Color::from_hex(0xFBBFC4),
            crate::ui::tokens::color::Color::from_hex(0xFED9DD),
        );
        t.colors.semantic.info = crate::ui::tokens::color::ColorScale::new(
            crate::ui::tokens::color::Color::from_hex(0x0F2430),
            crate::ui::tokens::color::Color::from_hex(0x153341),
            crate::ui::tokens::color::Color::from_hex(0x1B4556),
            crate::ui::tokens::color::Color::from_hex(0x27799A),
            crate::ui::tokens::color::Color::from_hex(0x2FA2C8),
            crate::ui::tokens::color::Color::from_hex(0x39BAE6), // s500: Ayu cyan
            crate::ui::tokens::color::Color::from_hex(0x5FC9EC),
            crate::ui::tokens::color::Color::from_hex(0x86D7F2),
            crate::ui::tokens::color::Color::from_hex(0xACE5F7),
            crate::ui::tokens::color::Color::from_hex(0xD2F2FB),
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
    pub fn colors(&self) -> &ColorPalette {
        &self.colors
    }
    #[inline]
    pub fn typography(&self) -> &TypographyScale {
        &self.typography
    }
    #[inline]
    pub fn spacing(&self) -> &SpacingScale {
        &self.spacing
    }
    #[inline]
    pub fn radius(&self) -> &RadiusScale {
        &self.radius
    }
    #[inline]
    pub fn elevation(&self) -> &ElevationScale {
        &self.elevation
    }
    #[inline]
    pub fn motion(&self) -> &MotionScale {
        &self.motion
    }
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
