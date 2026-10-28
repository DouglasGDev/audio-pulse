//! Graphic 10-band equalizer and parametric AutoEQ correction.

use crate::biquad::Biquad;
use crate::params::{AutoEqFilter, FilterKind, EQ_BAND_COUNT, EQ_FREQUENCIES, MAX_AUTOEQ_FILTERS};

const EQ_Q: f32 = 1.41;

pub struct GraphicEq {
    left: [Biquad; EQ_BAND_COUNT],
    right: [Biquad; EQ_BAND_COUNT],
    gains: [f32; EQ_BAND_COUNT],
}

impl GraphicEq {
    pub fn new(sample_rate: f32) -> Self {
        let mut eq = Self {
            left: [Biquad::identity(); EQ_BAND_COUNT],
            right: [Biquad::identity(); EQ_BAND_COUNT],
            gains: [0.0; EQ_BAND_COUNT],
        };
        eq.set_gains(&[0.0; EQ_BAND_COUNT], sample_rate);
        eq
    }

    pub fn set_gains(&mut self, gains: &[f32; EQ_BAND_COUNT], sample_rate: f32) {
        self.gains = *gains;
        for (i, &freq) in EQ_FREQUENCIES.iter().enumerate() {
            self.left[i] = Biquad::peaking(sample_rate, freq, gains[i], EQ_Q);
            self.right[i] = Biquad::peaking(sample_rate, freq, gains[i], EQ_Q);
        }
    }

    #[inline]
    pub fn process(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        for i in 0..EQ_BAND_COUNT {
            l = self.left[i].process(l);
            r = self.right[i].process(r);
        }
        (l, r)
    }

    pub fn reset(&mut self) {
        for f in &mut self.left {
            f.reset();
        }
        for f in &mut self.right {
            f.reset();
        }
    }
}

pub struct ParametricEq {
    left: [Biquad; MAX_AUTOEQ_FILTERS],
    right: [Biquad; MAX_AUTOEQ_FILTERS],
    count: usize,
}

impl ParametricEq {
    pub fn new() -> Self {
        Self {
            left: [Biquad::identity(); MAX_AUTOEQ_FILTERS],
            right: [Biquad::identity(); MAX_AUTOEQ_FILTERS],
            count: 0,
        }
    }

    pub fn set_filters(&mut self, filters: &[AutoEqFilter], sample_rate: f32) {
        self.count = filters.len().min(MAX_AUTOEQ_FILTERS);
        for (i, filter) in filters.iter().take(self.count).enumerate() {
            let make = || match filter.kind {
                FilterKind::Peaking => {
                    Biquad::peaking(sample_rate, filter.freq, filter.gain_db, filter.q)
                }
                FilterKind::Lowshelf => {
                    Biquad::low_shelf(sample_rate, filter.freq, filter.gain_db, filter.q)
                }
                FilterKind::Highshelf => {
                    Biquad::high_shelf(sample_rate, filter.freq, filter.gain_db, filter.q)
                }
            };
            self.left[i] = make();
            self.right[i] = make();
        }
        for i in self.count..MAX_AUTOEQ_FILTERS {
            self.left[i] = Biquad::identity();
            self.right[i] = Biquad::identity();
        }
    }

    #[inline]
    pub fn process(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        for i in 0..self.count {
            l = self.left[i].process(l);
            r = self.right[i].process(r);
        }
        (l, r)
    }

    pub fn reset(&mut self) {
        for f in &mut self.left {
            f.reset();
        }
        for f in &mut self.right {
            f.reset();
        }
    }
}

impl Default for ParametricEq {
    fn default() -> Self {
        Self::new()
    }
}

/// Mid and treble tone controls. Mid is a broad peaking filter; treble is a
/// high shelf, matching the labels in the UI.
pub struct ToneStage {
    mid_l: Biquad,
    mid_r: Biquad,
    high_l: Biquad,
    high_r: Biquad,
}

impl ToneStage {
    pub fn new(sample_rate: f32) -> Self {
        let mut stage = Self {
            mid_l: Biquad::identity(),
            mid_r: Biquad::identity(),
            high_l: Biquad::identity(),
            high_r: Biquad::identity(),
        };
        stage.set_params(sample_rate, 0.0, 0.0);
        stage
    }

    pub fn set_params(&mut self, sample_rate: f32, mid_db: f32, treble_db: f32) {
        self.mid_l = Biquad::peaking(sample_rate, 1_500.0, mid_db, 0.75);
        self.mid_r = Biquad::peaking(sample_rate, 1_500.0, mid_db, 0.75);
        self.high_l = Biquad::high_shelf(sample_rate, 6_000.0, treble_db, 0.707);
        self.high_r = Biquad::high_shelf(sample_rate, 6_000.0, treble_db, 0.707);
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        (
            self.high_l.process(self.mid_l.process(l)),
            self.high_r.process(self.mid_r.process(r)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn graphic_eq_boosts_selected_band() {
        let sr = 48_000.0;
        let mut eq = GraphicEq::new(sr);
        let mut gains = [0.0; 10];
        gains[5] = 6.0; // 1 kHz
        eq.set_gains(&gains, sr);
        let mut out = Vec::new();
        for i in 0..48_000 {
            let phase = 2.0 * std::f32::consts::PI * 1_000.0 * i as f32 / sr;
            let (l, _) = eq.process(phase.sin() * 0.1, 0.0);
            if i > 4_000 {
                out.push(l);
            }
        }
        let gain = 20.0 * (rms(&out) / 0.1 * 2.0f32.sqrt()).log10();
        assert!(gain > 4.0, "expected > 4 dB, measured {gain}");
    }
}