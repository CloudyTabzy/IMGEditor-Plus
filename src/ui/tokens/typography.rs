//! Typography tokens for text styling.

/// Font weight values following CSS font-weight specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum FontWeight {
    Thin = 100,
    ExtraLight = 200,
    Light = 300,
    Regular = 400,
    Medium = 500,
    SemiBold = 600,
    Bold = 700,
    ExtraBold = 800,
    Black = 900,
}

impl FontWeight {
    #[inline]
    pub const fn value(self) -> u16 { self as u16 }
}

impl Default for FontWeight {
    fn default() -> Self { Self::Regular }
}

/// A complete text style definition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub font_family: &'static str,
    pub size: f32,
    pub weight: FontWeight,
    pub line_height: f32,
    pub letter_spacing: f32,
}

impl TextStyle {
    #[inline]
    pub const fn new(
        font_family: &'static str,
        size: f32,
        weight: FontWeight,
        line_height: f32,
    ) -> Self {
        Self { font_family, size, weight, line_height, letter_spacing: 0.0 }
    }

    #[inline]
    pub const fn with_letter_spacing(self, letter_spacing: f32) -> Self {
        Self { letter_spacing, ..self }
    }

    #[inline]
    pub const fn line_height_px(&self) -> f32 { self.size * self.line_height }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::new("Inter", 16.0, FontWeight::Regular, 1.5)
    }
}

/// Named text style categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextStyleName {
    DisplayXl,
    DisplayLg,
    HeadingLg,
    HeadingMd,
    HeadingSm,
    BodyLg,
    BodyMd,
    BodySm,
    Code,
    Label,
    Micro,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypographyScale {
    pub display_xl: TextStyle,
    pub display_lg: TextStyle,
    pub heading_lg: TextStyle,
    pub heading_md: TextStyle,
    pub heading_sm: TextStyle,
    pub body_lg: TextStyle,
    pub body_md: TextStyle,
    pub body_sm: TextStyle,
    pub code: TextStyle,
    pub label: TextStyle,
    pub micro: TextStyle,
}

impl TypographyScale {
    #[inline]
    pub fn get(&self, name: TextStyleName) -> &TextStyle {
        match name {
            TextStyleName::DisplayXl => &self.display_xl,
            TextStyleName::DisplayLg => &self.display_lg,
            TextStyleName::HeadingLg => &self.heading_lg,
            TextStyleName::HeadingMd => &self.heading_md,
            TextStyleName::HeadingSm => &self.heading_sm,
            TextStyleName::BodyLg => &self.body_lg,
            TextStyleName::BodyMd => &self.body_md,
            TextStyleName::BodySm => &self.body_sm,
            TextStyleName::Code => &self.code,
            TextStyleName::Label => &self.label,
            TextStyleName::Micro => &self.micro,
        }
    }
}

impl Default for TypographyScale {
    fn default() -> Self {
        const FONT: &str = "Inter";
        const MONO: &str = "JetBrains Mono";
        Self {
            display_xl: TextStyle::new(FONT, 60.0, FontWeight::Bold, 1.1),
            display_lg: TextStyle::new(FONT, 48.0, FontWeight::Bold, 1.1),
            heading_lg: TextStyle::new(FONT, 32.0, FontWeight::SemiBold, 1.2),
            heading_md: TextStyle::new(FONT, 24.0, FontWeight::SemiBold, 1.3),
            heading_sm: TextStyle::new(FONT, 20.0, FontWeight::SemiBold, 1.4),
            body_lg: TextStyle::new(FONT, 18.0, FontWeight::Regular, 1.6),
            body_md: TextStyle::new(FONT, 16.0, FontWeight::Regular, 1.5),
            body_sm: TextStyle::new(FONT, 14.0, FontWeight::Regular, 1.5),
            code: TextStyle::new(MONO, 14.0, FontWeight::Regular, 1.6),
            label: TextStyle::new(FONT, 12.0, FontWeight::Medium, 1.4),
            micro: TextStyle::new(FONT, 10.0, FontWeight::Medium, 1.4),
        }
    }
}
