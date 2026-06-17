//! Elevation and shadow tokens.
use crate::ui::tokens::color::Color;

/// Shadow definition for elevation effects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

impl Shadow {
    #[inline]
    pub const fn new(offset_x: f32, offset_y: f32, blur: f32, spread: f32, color: Color) -> Self {
        Self { offset_x, offset_y, blur, spread, color }
    }

    pub const NONE: Self = Self::new(0.0, 0.0, 0.0, 0.0, Color::TRANSPARENT);
}

impl Default for Shadow {
    fn default() -> Self { Self::NONE }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ElevationLevel {
    Flat = 0,
    Raised = 1,
    Overlay = 2,
    Floating = 3,
    Modal = 4,
}

impl ElevationLevel {
    #[inline]
    pub const fn index(self) -> usize { self as usize }

    #[inline]
    pub const fn z_index(self) -> u32 {
        match self {
            Self::Flat => 0,
            Self::Raised => 10,
            Self::Overlay => 20,
            Self::Floating => 30,
            Self::Modal => 40,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Elevation {
    pub shadow: Shadow,
    pub shadow_secondary: Option<Shadow>,
    pub border_width: f32,
    pub border_color: Color,
}

impl Elevation {
    #[inline]
    pub const fn new(shadow: Shadow) -> Self {
        Self { shadow, shadow_secondary: None, border_width: 0.0, border_color: Color::TRANSPARENT }
    }

    #[inline]
    pub const fn with_border(border_width: f32, border_color: Color) -> Self {
        Self { shadow: Shadow::NONE, shadow_secondary: None, border_width, border_color }
    }

    pub const FLAT: Self = Self::new(Shadow::NONE);
}

impl Default for Elevation {
    fn default() -> Self { Self::FLAT }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElevationScale {
    pub flat: Elevation,
    pub raised: Elevation,
    pub overlay: Elevation,
    pub floating: Elevation,
    pub modal: Elevation,
}

impl ElevationScale {
    #[inline]
    pub fn get(&self, level: ElevationLevel) -> &Elevation {
        match level {
            ElevationLevel::Flat => &self.flat,
            ElevationLevel::Raised => &self.raised,
            ElevationLevel::Overlay => &self.overlay,
            ElevationLevel::Floating => &self.floating,
            ElevationLevel::Modal => &self.modal,
        }
    }
}

impl Default for ElevationScale {
    fn default() -> Self {
        let shadow_color = Color::new(0.0, 0.0, 0.0, 0.1);
        let shadow_color_strong = Color::new(0.0, 0.0, 0.0, 0.15);
        Self {
            flat: Elevation::FLAT,
            raised: Elevation::new(Shadow::new(0.0, 1.0, 3.0, 0.0, shadow_color)),
            overlay: Elevation::new(Shadow::new(0.0, 4.0, 6.0, -1.0, shadow_color)),
            floating: Elevation::new(Shadow::new(0.0, 10.0, 15.0, -3.0, shadow_color_strong)),
            modal: Elevation::new(Shadow::new(0.0, 25.0, 50.0, -12.0, shadow_color_strong)),
        }
    }
}
