//! Easing functions for animation.
//!
//! Ported from Robert Penner's easing equations and augmented with
//! elastic, bounce, and back variants. Each function takes a normalised
//! time `t` in [0,1] and returns a normalised value in [0,1].

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    QuadIn, QuadOut, QuadInOut,
    CubicIn, CubicOut, CubicInOut,
    QuartIn, QuartOut, QuartInOut,
    QuintIn, QuintOut, QuintInOut,
    SineIn, SineOut, SineInOut,
    ExpoIn, ExpoOut, ExpoInOut,
    ElasticIn, ElasticOut, ElasticInOut,
    BounceIn, BounceOut, BounceInOut,
    BackIn, BackOut, BackInOut,
}

impl Easing {
    /// Apply the easing function to a normalised time `t` in [0, 1],
    /// returning a normalised value in [0, 1].
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::QuadIn => t * t,
            Self::QuadOut => t * (2.0 - t),
            Self::QuadInOut => {
                if t < 0.5 { 2.0 * t * t } else { -1.0 + (4.0 - 2.0 * t) * t }
            }
            Self::CubicIn => t * t * t,
            Self::CubicOut => {
                let t = t - 1.0;
                t * t * t + 1.0
            }
            Self::CubicInOut => {
                if t < 0.5 { 4.0 * t * t * t } else { let t = 2.0 * t - 2.0; 0.5 * t * t * t + 1.0 }
            }
            Self::QuartIn => t * t * t * t,
            Self::QuartOut => {
                let t = t - 1.0;
                1.0 - t * t * t * t
            }
            Self::QuartInOut => {
                if t < 0.5 { 8.0 * t * t * t * t } else { let t = t - 1.0; 1.0 - 8.0 * t * t * t * t }
            }
            Self::QuintIn => t * t * t * t * t,
            Self::QuintOut => {
                let t = t - 1.0;
                1.0 + t * t * t * t * t
            }
            Self::QuintInOut => {
                if t < 0.5 { 16.0 * t * t * t * t * t } else { let t = 2.0 * t - 2.0; 0.5 * t * t * t * t * t + 1.0 }
            }
            Self::SineIn => 1.0 - (t * std::f32::consts::FRAC_PI_2).cos(),
            Self::SineOut => (t * std::f32::consts::FRAC_PI_2).sin(),
            Self::SineInOut => 0.5 * (1.0 - (std::f32::consts::PI * t).cos()),
            Self::ExpoIn => {
                if t == 0.0 { 0.0 } else { (16.0 * t - 16.0).exp2() }
            }
            Self::ExpoOut => {
                if t == 1.0 { 1.0 } else { 1.0 - (-16.0 * t).exp2() }
            }
            Self::ExpoInOut => {
                if t == 0.0 { 0.0 }
                else if t == 1.0 { 1.0 }
                else if t < 0.5 { 0.5 * (16.0 * (2.0 * t) - 16.0).exp2() }
                else { 0.5 * (2.0 - (-16.0 * (2.0 * t - 1.0)).exp2()) }
            }
            Self::ElasticIn => {
                if t == 0.0 || t == 1.0 { t }
                else {
                    let p = 0.3;
                    let s = p / 4.0;
                    -(2.0_f32).powf(10.0 * (t - 1.0)) * ((t - 1.0 - s) * (2.0 * std::f32::consts::PI) / p).sin()
                }
            }
            Self::ElasticOut => {
                if t == 0.0 || t == 1.0 { t }
                else {
                    let p = 0.3;
                    let s = p / 4.0;
                    (2.0_f32).powf(-10.0 * t) * ((t - s) * (2.0 * std::f32::consts::PI) / p).sin() + 1.0
                }
            }
            Self::ElasticInOut => {
                if t == 0.0 || t == 1.0 { t }
                else {
                    let p = 0.3 * 1.5;
                    let s = p / 4.0;
                    if t < 0.5 {
                        -0.5 * (2.0_f32).powf(10.0 * (2.0 * t - 1.0)) * ((2.0 * t - 1.0 - s) * (2.0 * std::f32::consts::PI) / p).sin()
                    } else {
                        0.5 * (2.0_f32).powf(-10.0 * (2.0 * t - 1.0)) * ((2.0 * t - 1.0 - s) * (2.0 * std::f32::consts::PI) / p).sin() + 1.0
                    }
                }
            }
            Self::BounceIn => 1.0 - Self::BounceOut.apply(1.0 - t),
            Self::BounceOut => {
                let (n1, d1) = (7.5625, 2.75);
                if t < 1.0 / d1 { n1 * t * t }
                else if t < 2.0 / d1 { n1 * (t - 1.5 / d1) * (t - 1.5 / d1) + 0.75 }
                else if t < 2.5 / d1 { n1 * (t - 2.25 / d1) * (t - 2.25 / d1) + 0.9375 }
                else { n1 * (t - 2.625 / d1) * (t - 2.625 / d1) + 0.984375 }
            }
            Self::BounceInOut => {
                if t < 0.5 { (1.0 - Self::BounceOut.apply(1.0 - 2.0 * t)) * 0.5 }
                else { (1.0 + Self::BounceOut.apply(2.0 * t - 1.0)) * 0.5 }
            }
            Self::BackIn => {
                let s = 1.70158;
                t * t * ((s + 1.0) * t - s)
            }
            Self::BackOut => {
                let s = 1.70158;
                let t = t - 1.0;
                t * t * ((s + 1.0) * t + s) + 1.0
            }
            Self::BackInOut => {
                let s = 1.70158 * 1.525;
                if t < 0.5 {
                    let t = 2.0 * t;
                    0.5 * (t * t * ((s + 1.0) * t - s))
                } else {
                    let t = 2.0 * t - 2.0;
                    0.5 * (t * t * ((s + 1.0) * t + s) + 2.0)
                }
            }
        }
    }
}

impl Default for Easing {
    fn default() -> Self { Self::CubicOut }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_unchanged() {
        let e = Easing::Linear;
        assert!((e.apply(0.0) - 0.0).abs() < 1e-6);
        assert!((e.apply(0.5) - 0.5).abs() < 1e-6);
        assert!((e.apply(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cubic_out_smooth() {
        let e = Easing::CubicOut;
        assert!((e.apply(0.0) - 0.0).abs() < 1e-6);
        assert!((e.apply(1.0) - 1.0).abs() < 1e-6);
        // Midpoint: should be eased >0.5
        assert!(e.apply(0.5) > 0.5, "cubic out should overshoot at midpoint");
    }

    #[test]
    fn elastic_bounces() {
        let e = Easing::ElasticOut;
        assert!((e.apply(0.0) - 0.0).abs() < 1e-4);
        assert!((e.apply(1.0) - 1.0).abs() < 1e-4);
        // Elastic should briefly exceed 1.0
        let mid = e.apply(0.7);
        assert!(mid > 1.0 || mid < 0.0, "elastic should overshoot: got {mid}");
    }

    #[test]
    fn bounce_never_negative() {
        let e = Easing::BounceOut;
        for i in 0..=1000 {
            let t = i as f32 / 1000.0;
            let v = e.apply(t);
            assert!(v >= 0.0, "bounce went negative at t={t}");
            assert!(v <= 1.0, "bounce exceeded 1.0 at t={t}");
        }
    }

    #[test]
    fn back_in_overshoots_negative() {
        let e = Easing::BackIn;
        let mid = e.apply(0.5);
        assert!(mid < 0.0, "back-in should dip below 0 at midpoint: got {mid}");
    }
}
