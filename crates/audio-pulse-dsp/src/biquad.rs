//! Fundamental biquad filter implementation used by every stage.
//!
//! Coefficients follow the RBJ Audio EQ Cookbook, computed in `f64` and stored
//! as `f32` for the real-time path. Processing uses the transposed direct form
//! II topology, which has good numerical behaviour for time-varying filters.

use std::f64::consts::PI;

#[derive(Clone, Copy, Debug)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Default for Biquad {
    fn default() -> Self {
        Self::identity()
    }
}

impl Biquad {
    pub fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }

    fn from_coeffs(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Self {
        if !a0.is_finite() || a0.abs() < 1e-12 {
            return Self::identity();
        }
        let inv = 1.0 / a0;
        let mut filter = Self {
            b0: (b0 * inv) as f32,
            b1: (b1 * inv) as f32,
            b2: (b2 * inv) as f32,
            a1: (a1 * inv) as f32,
            a2: (a2 * inv) as f32,
            z1: 0.0,
            z2: 0.0,
        };
        // Guard against denormal coefficients from bad parameters.
        if !filter.is_finite() {
            filter = Self::identity();
        }
        filter
    }

    fn is_finite(&self) -> bool {
        self.b0.is_finite()
            && self.b1.is_finite()
            && self.b2.is_finite()
            && self.a1.is_finite()
            && self.a2.is_finite()
    }

    fn omega(sample_rate: f32, freq: f32) -> f64 {
        let sr = sample_rate.max(8_000.0) as f64;
        let f = (freq as f64).clamp(10.0, sr * 0.495);
        2.0 * PI * f / sr
    }

    pub fn peaking(sample_rate: f32, freq: f32, gain_db: f32, q: f32) -> Self {
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let w0 = Self::omega(sample_rate, freq);
        let cos = w0.cos();
        let alpha = w0.sin() / (2.0 * q.max(0.05) as f64);
        Self::from_coeffs(
            1.0 + alpha * a,
            -2.0 * cos,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos,
            1.0 - alpha / a,
        )
    }

    /// Replaces only the coefficients, keeping the filter memory. Used when a
    /// source moves (dynamic HRTF) or the adaptive EQ retunes: rebuilding the
    /// biquad from scratch would zero `z1`/`z2` and produce a click.
    pub fn set_peaking(&mut self, sample_rate: f32, freq: f32, gain_db: f32, q: f32) {
        let retuned = Self::peaking(sample_rate, freq, gain_db, q);
        self.assign(&retuned);
    }

    pub fn set_low_shelf(&mut self, sample_rate: f32, freq: f32, gain_db: f32, q: f32) {
        let retuned = Self::low_shelf(sample_rate, freq, gain_db, q);
        self.assign(&retuned);
    }

    pub fn set_high_shelf(&mut self, sample_rate: f32, freq: f32, gain_db: f32, q: f32) {
        let retuned = Self::high_shelf(sample_rate, freq, gain_db, q);
        self.assign(&retuned);
    }

    pub fn set_low_pass(&mut self, sample_rate: f32, freq: f32, q: f32) {
        let retuned = Self::low_pass(sample_rate, freq, q);
        self.assign(&retuned);
    }

    pub fn set_high_pass(&mut self, sample_rate: f32, freq: f32, q: f32) {
        let retuned = Self::high_pass(sample_rate, freq, q);
        self.assign(&retuned);
    }

    /// Swaps the coefficients but not the delay memory, so retuning a stage is
    /// click-free even when the filter type changes.
    pub fn assign(&mut self, other: &Self) {
        self.b0 = other.b0;
        self.b1 = other.b1;
        self.b2 = other.b2;
        self.a1 = other.a1;
        self.a2 = other.a2;
    }

    pub fn low_shelf(sample_rate: f32, freq: f32, gain_db: f32, q: f32) -> Self {
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let w0 = Self::omega(sample_rate, freq);
        let cos = w0.cos();
        let sin = w0.sin();
        let alpha = sin / (2.0 * q.max(0.05) as f64);
        let sqrt_a = a.sqrt();
        let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
        Self::from_coeffs(
            a * ((a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
            a * ((a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha),
            (a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos),
            (a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha,
        )
    }

    pub fn high_shelf(sample_rate: f32, freq: f32, gain_db: f32, q: f32) -> Self {
        let a = 10f64.powf(gain_db as f64 / 40.0);
        let w0 = Self::omega(sample_rate, freq);
        let cos = w0.cos();
        let sin = w0.sin();
        let alpha = sin / (2.0 * q.max(0.05) as f64);
        let sqrt_a = a.sqrt();
        let two_sqrt_a_alpha = 2.0 * sqrt_a * alpha;
        Self::from_coeffs(
            a * ((a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
            a * ((a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha),
            (a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha,
            2.0 * ((a - 1.0) - (a + 1.0) * cos),
            (a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha,
        )
    }

    pub fn low_pass(sample_rate: f32, freq: f32, q: f32) -> Self {
        let w0 = Self::omega(sample_rate, freq);
        let cos = w0.cos();
        let alpha = w0.sin() / (2.0 * q.max(0.05) as f64);
        let b = (1.0 - cos) * 0.5;
        Self::from_coeffs(b, 1.0 - cos, b, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
    }

    pub fn high_pass(sample_rate: f32, freq: f32, q: f32) -> Self {
        let w0 = Self::omega(sample_rate, freq);
        let cos = w0.cos();
        let alpha = w0.sin() / (2.0 * q.max(0.05) as f64);
        let b = (1.0 + cos) * 0.5;
        Self::from_coeffs(b, -(1.0 + cos), b, 1.0 + alpha, -2.0 * cos, 1.0 - alpha)
    }

    pub fn all_pass(sample_rate: f32, freq: f32, q: f32) -> Self {
        let w0 = Self::omega(sample_rate, freq);
        let cos = w0.cos();
        let alpha = w0.sin() / (2.0 * q.max(0.05) as f64);
        Self::from_coeffs(
            1.0 - alpha,
            -2.0 * cos,
            1.0 + alpha,
            1.0 + alpha,
            -2.0 * cos,
            1.0 - alpha,
        )
    }

    /// Magnitude response in dB at `freq`, useful for verification and for the
    /// UI's response overlay (the UI mirrors the same formulas).
    pub fn magnitude_db(&self, freq: f32, sample_rate: f32) -> f32 {
        let w = Self::omega(sample_rate, freq);
        let (s1, c1) = (w.sin(), w.cos());
        let (s2, c2) = ((2.0 * w).sin(), (2.0 * w).cos());
        let num_re = self.b0 as f64 + self.b1 as f64 * c1 + self.b2 as f64 * c2;
        let num_im = -(self.b1 as f64 * s1 + self.b2 as f64 * s2);
        let den_re = 1.0 + self.a1 as f64 * c1 + self.a2 as f64 * c2;
        let den_im = -(self.a1 as f64 * s1 + self.a2 as f64 * s2);
        let num = (num_re * num_re + num_im * num_im).sqrt();
        let den = (den_re * den_re + den_im * den_im).sqrt().max(1e-12);
        20.0 * (num / den).max(1e-12).log10() as f32
    }
}

/// One-pole low-pass used for head shadow and damping.
#[derive(Clone, Copy, Debug, Default)]
pub struct OnePole {
    a: f32,
    z: f32,
}

impl OnePole {
    pub fn low_pass(sample_rate: f32, freq: f32) -> Self {
        let sr = sample_rate.max(8_000.0);
        let f = freq.clamp(10.0, sr * 0.49);
        let a = (-2.0f32 * std::f32::consts::PI * f / sr).exp();
        Self { a, z: 0.0 }
    }

    /// Retunes the pole without dropping the integrator, so a moving head
    /// shadow filter does not restart from silence. `reset()` is the explicit
    /// way to clear the state.
    pub fn set_freq(&mut self, sample_rate: f32, freq: f32) {
        let sr = sample_rate.max(8_000.0);
        let f = freq.clamp(10.0, sr * 0.49);
        self.a = (-2.0f32 * std::f32::consts::PI * f / sr).exp();
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        self.z = x * (1.0 - self.a) + self.z * self.a;
        self.z
    }

    pub fn reset(&mut self) {
        self.z = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peaking_has_expected_gain_at_center() {
        let sr = 48_000.0;
        let filter = Biquad::peaking(sr, 1_000.0, 6.0, 1.4);
        let gain = filter.magnitude_db(1_000.0, sr);
        assert!((gain - 6.0).abs() < 0.15, "gain {gain}");
    }

    #[test]
    fn shelves_touch_dc_and_nyquist() {
        let sr = 48_000.0;
        let low = Biquad::low_shelf(sr, 120.0, 8.0, 0.707);
        assert!((low.magnitude_db(20.0, sr) - 8.0).abs() < 0.5);
        let high = Biquad::high_shelf(sr, 6_000.0, -6.0, 0.707);
        assert!((high.magnitude_db(16_000.0, sr) + 6.0).abs() < 1.0);
    }

    #[test]
    fn filter_remains_finite_after_impulse() {
        let mut f = Biquad::peaking(48_000.0, 60.0, 12.0, 6.0);
        let mut max = 0.0f32;
        for i in 0..48_000 {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let y = f.process(x);
            assert!(y.is_finite());
            max = max.max(y.abs());
        }
        assert!(max < 100.0);
    }
}
