//! Border radius tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RadiusSize {
    None = 0,
    Xs = 1,
    Sm = 2,
    Md = 3,
    Lg = 4,
    Xl = 5,
    Xl2 = 6,
    Full = 7,
}

impl RadiusSize {
    #[inline]
    pub const fn index(self) -> usize { self as usize }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadiusScale {
    values: [f32; 8],
}

impl RadiusScale {
    pub const DEFAULT: Self = Self {
        values: [0.0, 2.0, 4.0, 6.0, 8.0, 12.0, 16.0, 9999.0],
    };

    #[inline]
    pub const fn new(values: [f32; 8]) -> Self { Self { values } }

    #[inline]
    pub const fn get(&self, size: RadiusSize) -> f32 { self.values[size.index()] }

    #[inline] pub const fn none(&self) -> f32 { self.values[0] }
    #[inline] pub const fn xs(&self) -> f32 { self.values[1] }
    #[inline] pub const fn sm(&self) -> f32 { self.values[2] }
    #[inline] pub const fn md(&self) -> f32 { self.values[3] }
    #[inline] pub const fn lg(&self) -> f32 { self.values[4] }
    #[inline] pub const fn xl(&self) -> f32 { self.values[5] }
    #[inline] pub const fn xl2(&self) -> f32 { self.values[6] }
    #[inline] pub const fn full(&self) -> f32 { self.values[7] }
}

impl Default for RadiusScale {
    fn default() -> Self { Self::DEFAULT }
}
