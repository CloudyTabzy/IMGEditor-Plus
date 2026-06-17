//! Motion and animation timing tokens.

/// Named duration presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DurationPreset {
    Instant,
    Fast,
    Normal,
    Slow,
    Slower,
}

impl DurationPreset {
    #[inline]
    pub const fn ms(self) -> u32 {
        match self {
            Self::Instant => 0,
            Self::Fast => 100,
            Self::Normal => 200,
            Self::Slow => 300,
            Self::Slower => 500,
        }
    }

    #[inline]
    pub const fn seconds(self) -> f32 { self.ms() as f32 / 1000.0 }
}

/// Easing function identifiers (for use in cubic-bezier strings).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    CubicBezier(f32, f32, f32, f32),
}

impl Easing {
    pub const STANDARD: Self = Self::CubicBezier(0.4, 0.0, 0.2, 1.0);
    pub const DECELERATE: Self = Self::CubicBezier(0.0, 0.0, 0.2, 1.0);
    pub const ACCELERATE: Self = Self::CubicBezier(0.4, 0.0, 1.0, 1.0);
    pub const SHARP: Self = Self::CubicBezier(0.4, 0.0, 0.6, 1.0);
}

impl Default for Easing {
    fn default() -> Self { Self::STANDARD }
}

/// A complete motion definition combining duration and easing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motion {
    pub duration_ms: u32,
    pub easing: Easing,
}

impl Motion {
    #[inline]
    pub const fn new(duration_ms: u32, easing: Easing) -> Self {
        Self { duration_ms, easing }
    }

    #[inline]
    pub const fn from_preset(preset: DurationPreset, easing: Easing) -> Self {
        Self::new(preset.ms(), easing)
    }

    #[inline]
    pub const fn duration_seconds(&self) -> f32 { self.duration_ms as f32 / 1000.0 }

    pub const NONE: Self = Self::new(0, Easing::Linear);
}

impl Default for Motion {
    fn default() -> Self { Self::new(200, Easing::STANDARD) }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionScale {
    pub instant: Motion,
    pub fast: Motion,
    pub normal: Motion,
    pub slow: Motion,
    pub slower: Motion,
}

impl MotionScale {
    #[inline]
    pub const fn get(&self, preset: DurationPreset) -> Motion {
        match preset {
            DurationPreset::Instant => self.instant,
            DurationPreset::Fast => self.fast,
            DurationPreset::Normal => self.normal,
            DurationPreset::Slow => self.slow,
            DurationPreset::Slower => self.slower,
        }
    }
}

impl Default for MotionScale {
    fn default() -> Self {
        Self {
            instant: Motion::NONE,
            fast: Motion::new(100, Easing::SHARP),
            normal: Motion::new(200, Easing::STANDARD),
            slow: Motion::new(300, Easing::STANDARD),
            slower: Motion::new(500, Easing::DECELERATE),
        }
    }
}
