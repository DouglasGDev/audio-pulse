//! Graphic 10-band equalizer and parametric AutoEQ correction.

use crate::biquad::Biquad;
use crate::params::{AutoEqFilter, FilterKind, EQ_BAND_COUNT, EQ_FREQUENCIES, MAX_AUTOEQ_FILTERS};

/// Bandwidth of the graphic EQ bands. Presets evaluate the same cascade when
/// computing their static headroom, so the two must not drift apart.
pub const EQ_Q: f32 = 1.41;

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
            self.left[i].set_peaking(sample_rate, freq, gains[i], EQ_Q);
            self.right[i].set_peaking(sample_rate, freq, gains[i], EQ_Q);
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
            // Retuning keeps the delay memory so moving a slider or swapping
            // a correction curve does not restart the filter from silence.
            match filter.kind {
                FilterKind::Peaking => {
                    self.left[i].set_peaking(sample_rate, filter.freq, filter.gain_db, filter.q);
                    self.right[i].set_peaking(sample_rate, filter.freq, filter.gain_db, filter.q);
                }
                FilterKind::Lowshelf => {
                    self.left[i].set_low_shelf(sample_rate, filter.freq, filter.gain_db, filter.q);
                    self.right[i].set_low_shelf(sample_rate, filter.freq, filter.gain_db, filter.q);
                }
                FilterKind::Highshelf => {
                    self.left[i].set_high_shelf(sample_rate, filter.freq, filter.gain_db, filter.q);
                    self.right[i].set_high_shelf(
                        sample_rate,
                        filter.freq,
                        filter.gain_db,
                        filter.q,
                    );
                }
            }
        }
        // A section that leaves the set starts from an empty memory; keeping
        // stale state into the next curve would be worse than a cleared one.
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
        self.mid_l.set_peaking(sample_rate, 1_500.0, mid_db, 0.75);
        self.mid_r.set_peaking(sample_rate, 1_500.0, mid_db, 0.75);
        self.high_l
            .set_high_shelf(sample_rate, 6_000.0, treble_db, 0.707);
        self.high_r
            .set_high_shelf(sample_rate, 6_000.0, treble_db, 0.707);
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

    /// Re-applying the same settings must not touch the delay memory: an EQ
    /// that reallocates its biquads on every UI update restarts every band
    /// from silence and clicks. Comparing against an untouched twin proves
    /// the state survived sample for sample.
    #[test]
    fn retuning_keeps_the_filter_memory() {
        let sr = 48_000.0;
        let tone = |i: usize| 0.3 * (2.0 * std::f32::consts::PI * 1_000.0 * i as f32 / sr).sin();

        let gains = [2.0, 1.0, 0.0, -1.0, 0.5, 0.0, 1.5, 2.5, 1.0, -2.0];
        let mut graphic = GraphicEq::new(sr);
        let mut twin = GraphicEq::new(sr);
        graphic.set_gains(&gains, sr);
        twin.set_gains(&gains, sr);
        for i in 0..4_800 {
            let x = tone(i);
            graphic.process(x, x);
            twin.process(x, x);
        }
        graphic.set_gains(&gains, sr);
        for i in 4_800..4_812 {
            let x = tone(i);
            assert_eq!(
                graphic.process(x, x),
                twin.process(x, x),
                "graphic EQ dropped its state at sample {i}"
            );
        }

        let filters = vec![
            AutoEqFilter {
                kind: FilterKind::Peaking,
                freq: 3_000.0,
                gain_db: 4.0,
                q: 1.2,
            },
            AutoEqFilter {
                kind: FilterKind::Lowshelf,
                freq: 100.0,
                gain_db: -3.0,
                q: 0.707,
            },
        ];
        let mut parametric = ParametricEq::new();
        let mut twin = ParametricEq::new();
        parametric.set_filters(&filters, sr);
        twin.set_filters(&filters, sr);
        for i in 0..4_800 {
            let x = tone(i);
            parametric.process(x, x);
            twin.process(x, x);
        }
        parametric.set_filters(&filters, sr);
        for i in 4_800..4_812 {
            let x = tone(i);
            assert_eq!(
                parametric.process(x, x),
                twin.process(x, x),
                "parametric EQ dropped its state at sample {i}"
            );
        }

        let mut tone_stage = ToneStage::new(sr);
        let mut twin = ToneStage::new(sr);
        tone_stage.set_params(sr, 3.0, -2.0);
        twin.set_params(sr, 3.0, -2.0);
        for i in 0..4_800 {
            let x = tone(i);
            tone_stage.process(x, x);
            twin.process(x, x);
        }
        tone_stage.set_params(sr, 3.0, -2.0);
        for i in 4_800..4_812 {
            let x = tone(i);
            assert_eq!(
                tone_stage.process(x, x),
                twin.process(x, x),
                "tone stage dropped its state at sample {i}"
            );
        }
    }
}
