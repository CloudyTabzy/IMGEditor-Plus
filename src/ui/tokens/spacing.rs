//! Named spacing sizes for semantic usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SpacingSize {
    Xxs = 0,
    Xs = 1,
    Sm = 2,
    Md = 3,
    Lg = 4,
    Xl = 5,
    Xl2 = 6,
    Xl3 = 7,
    Xl4 = 8,
    Xl5 = 9,
}

impl SpacingSize {
    pub const ALL: [Self; 10] = [
        Self::Xxs, Self::Xs, Self::Sm, Self::Md, Self::Lg,
        Self::Xl, Self::Xl2, Self::Xl3, Self::Xl4, Self::Xl5,
    ];

    #[inline]
    pub const fn index(self) -> usize { self as usize }
}

/// A modular spacing scale with 10 predefined values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpacingScale {
    values: [f32; 10],
}

impl SpacingScale {
    pub const DEFAULT: Self = Self {
        values: [2.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0, 64.0, 96.0],
    };

    #[inline]
    pub const fn new(values: [f32; 10]) -> Self { Self { values } }

    #[inline]
    pub const fn get(&self, size: SpacingSize) -> f32 {
        self.values[size.index()]
    }

    #[inline] pub const fn xxs(&self) -> f32 { self.values[0] }
    #[inline] pub const fn xs(&self) -> f32 { self.values[1] }
    #[inline] pub const fn sm(&self) -> f32 { self.values[2] }
    #[inline] pub const fn md(&self) -> f32 { self.values[3] }
    #[inline] pub const fn lg(&self) -> f32 { self.values[4] }
    #[inline] pub const fn xl(&self) -> f32 { self.values[5] }
    #[inline] pub const fn xl2(&self) -> f32 { self.values[6] }
    #[inline] pub const fn xl3(&self) -> f32 { self.values[7] }
    #[inline] pub const fn xl4(&self) -> f32 { self.values[8] }
    #[inline] pub const fn xl5(&self) -> f32 { self.values[9] }

    #[inline]
    pub const fn values(&self) -> &[f32; 10] { &self.values }
}

impl Default for SpacingScale {
    fn default() -> Self { Self::DEFAULT }
}
