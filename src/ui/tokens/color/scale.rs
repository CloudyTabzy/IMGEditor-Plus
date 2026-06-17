//! Color scale with typed shade access.

/// An RGBA color with f32 components in the range 0.0..=1.0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    #[inline]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self::new(r, g, b, 1.0)
    }

    #[inline]
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
    }

    #[inline]
    pub const fn from_hex(hex: u32) -> Self {
        Self::from_rgb8(
            ((hex >> 16) & 0xFF) as u8,
            ((hex >> 8) & 0xFF) as u8,
            (hex & 0xFF) as u8,
        )
    }

    #[inline]
    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// Linear interpolation between two colors.
    #[inline]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Multiply alpha (for hover/focus overlays).
    #[inline]
    pub fn fade(self, factor: f32) -> Self {
        Self { a: (self.a * factor).clamp(0.0, 1.0), ..self }
    }

    pub const TRANSPARENT: Self = Self::new(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Self = Self::rgb(0.0, 0.0, 0.0);
    pub const WHITE: Self = Self::rgb(1.0, 1.0, 1.0);
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

/// Shade level for color scales (50-900).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Shade {
    S50 = 0,
    S100 = 1,
    S200 = 2,
    S300 = 3,
    S400 = 4,
    S500 = 5,
    S600 = 6,
    S700 = 7,
    S800 = 8,
    S900 = 9,
}

impl Shade {
    pub const ALL: [Self; 10] = [
        Self::S50, Self::S100, Self::S200, Self::S300, Self::S400,
        Self::S500, Self::S600, Self::S700, Self::S800, Self::S900,
    ];

    #[inline]
    pub const fn value(self) -> u16 {
        match self {
            Self::S50 => 50, Self::S100 => 100, Self::S200 => 200, Self::S300 => 300,
            Self::S400 => 400, Self::S500 => 500, Self::S600 => 600, Self::S700 => 700,
            Self::S800 => 800, Self::S900 => 900,
        }
    }

    #[inline]
    pub const fn index(self) -> usize { self as usize }
}

/// A 10-step color scale from light (50) to dark (900).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorScale {
    pub s50: Color,
    pub s100: Color,
    pub s200: Color,
    pub s300: Color,
    pub s400: Color,
    pub s500: Color,
    pub s600: Color,
    pub s700: Color,
    pub s800: Color,
    pub s900: Color,
}

impl ColorScale {
    #[inline]
    pub const fn new(
        s50: Color, s100: Color, s200: Color, s300: Color, s400: Color,
        s500: Color, s600: Color, s700: Color, s800: Color, s900: Color,
    ) -> Self {
        Self { s50, s100, s200, s300, s400, s500, s600, s700, s800, s900 }
    }

    #[inline]
    pub const fn from_array(colors: [Color; 10]) -> Self {
        Self {
            s50: colors[0], s100: colors[1], s200: colors[2], s300: colors[3], s400: colors[4],
            s500: colors[5], s600: colors[6], s700: colors[7], s800: colors[8], s900: colors[9],
        }
    }

    #[inline]
    pub const fn get(&self, shade: Shade) -> Color {
        match shade {
            Shade::S50 => self.s50, Shade::S100 => self.s100, Shade::S200 => self.s200,
            Shade::S300 => self.s300, Shade::S400 => self.s400, Shade::S500 => self.s500,
            Shade::S600 => self.s600, Shade::S700 => self.s700, Shade::S800 => self.s800,
            Shade::S900 => self.s900,
        }
    }

    #[inline]
    pub const fn base(&self) -> Color { self.s500 }
    #[inline]
    pub const fn light(&self) -> Color { self.s100 }
    #[inline]
    pub const fn dark(&self) -> Color { self.s700 }

    #[inline]
    pub const fn to_array(&self) -> [Color; 10] {
        [self.s50, self.s100, self.s200, self.s300, self.s400,
         self.s500, self.s600, self.s700, self.s800, self.s900]
    }
}

impl Default for ColorScale {
    fn default() -> Self {
        Self::new(
            Color::from_hex(0xFAFAFA), Color::from_hex(0xF5F5F5),
            Color::from_hex(0xE5E5E5), Color::from_hex(0xD4D4D4),
            Color::from_hex(0xA3A3A3), Color::from_hex(0x737373),
            Color::from_hex(0x525252), Color::from_hex(0x404040),
            Color::from_hex(0x262626), Color::from_hex(0x171717),
        )
    }
}
