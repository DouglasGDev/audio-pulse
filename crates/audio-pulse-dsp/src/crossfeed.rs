//! Analog-style crossfeed (bs2b family) for headphone listening.
//!
//! A delayed, low-passed copy of the opposite channel is mixed into each ear.
//! This reduces the unnatural hard separation of stereo recordings and
//! centralizes low frequencies, at the cost of a slightly narrower image.
//!
//! The bleed path is low-passed, so a fixed output trim would either boost
//! centred bass or dull the highs. A low shelf at the bleed cutoff inverses
//! exactly that: mono content below 700 Hz keeps its level, the highs stay
//! untouched, and only the inter-channel difference is narrowed.

use crate::biquad::Biquad;
use crate::delay::FractionalDelay;
use crate::params::CrossfeedParams;

const BLEED_CUTOFF_HZ: f32 = 700.0;

pub struct CrossfeedStage {
    sample_rate: f32,
    enabled: bool,
    amount: f32,
    delay_samples: f32,
    lp_l: Biquad,
    lp_r: Biquad,
    comp_l: Biquad,
    comp_r: Biquad,
    delay_l: FractionalDelay,
    delay_r: FractionalDelay,
}

impl CrossfeedStage {
    pub fn new(sample_rate: f32) -> Self {
        let mut stage = Self {
            sample_rate,
            enabled: false,
            amount: 0.0,
            delay_samples: 14.0,
            lp_l: Biquad::identity(),
            lp_r: Biquad::identity(),
            comp_l: Biquad::identity(),
            comp_r: Biquad::identity(),
            delay_l: FractionalDelay::new(512),
            delay_r: FractionalDelay::new(512),
        };
        stage.set_params(CrossfeedParams::default(), sample_rate);
        stage
    }

    pub fn set_params(&mut self, params: CrossfeedParams, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.enabled = params.enabled;
        self.amount = params.amount.clamp(0.0, 1.0);
        self.delay_samples =
            (params.delay_ms.clamp(0.05, 1.0) * sample_rate / 1_000.0).clamp(1.0, 480.0);
        let g = self.amount * 0.55;
        let trim_db = -20.0 * (1.0 + g).log10();
        self.lp_l.set_low_pass(sample_rate, BLEED_CUTOFF_HZ, 0.707);
        self.lp_r.set_low_pass(sample_rate, BLEED_CUTOFF_HZ, 0.707);
        self.comp_l
            .set_low_shelf(sample_rate, BLEED_CUTOFF_HZ, trim_db, 0.707);
        self.comp_r
            .set_low_shelf(sample_rate, BLEED_CUTOFF_HZ, trim_db, 0.707);
        if !params.enabled {
            self.reset();
        }
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.enabled || self.amount <= 0.0001 {
            return (l, r);
        }
        let bleed_l = self.lp_l.process(r);
        let bleed_r = self.lp_r.process(l);
        let delayed_l = self.delay_l.write_read(bleed_l, self.delay_samples);
        let delayed_r = self.delay_r.write_read(bleed_r, self.delay_samples);
        let g = self.amount * 0.55;
        (
            self.comp_l.process(l + delayed_l * g),
            self.comp_r.process(r + delayed_r * g),
        )
    }

    /// Delay of the bleed path in samples.
    pub fn latency_samples(&self) -> usize {
        if self.enabled && self.amount > 0.0001 {
            self.delay_samples.round() as usize
        } else {
            0
        }
    }

    pub fn reset(&mut self) {
        self.lp_l.reset();
        self.lp_r.reset();
        self.comp_l.reset();
        self.comp_r.reset();
        self.delay_l.reset();
        self.delay_r.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    fn run(amount: f32, freq: f32, left_gain: f32, right_gain: f32) -> (f64, f64) {
        let mut stage = CrossfeedStage::new(SR);
        stage.set_params(
            CrossfeedParams {
                enabled: true,
                amount,
                delay_ms: 0.3,
            },
            SR,
        );
        let mut energy_l = 0.0f64;
        let mut energy_r = 0.0f64;
        let mut energy_in = 0.0f64;
        for i in 0..48_000 {
            let phase = 2.0 * std::f32::consts::PI * freq * i as f32 / SR;
            let xl = left_gain * phase.sin();
            let xr = right_gain * phase.sin();
            let (ol, or) = stage.process(xl, xr);
            if i > 4_000 {
                energy_l += (ol as f64) * (ol as f64);
                energy_r += (or as f64) * (or as f64);
                energy_in += (xl as f64) * (xl as f64) + (xr as f64) * (xr as f64);
            }
        }
        (energy_l + energy_r, energy_in)
    }

    #[test]
    fn crossfeed_reduces_side_energy_of_a_low_tone() {
        let (out, center) = run(0.9, 120.0, 0.2, 0.0);
        assert!(out < center * 0.75, "in {center} out {out}");
    }

    #[test]
    fn mono_bass_keeps_its_level() {
        let (out, reference) = run(1.0, 100.0, 0.2, 0.2);
        let gain_db = 10.0 * (out / reference).log10();
        assert!(
            gain_db.abs() < 0.6,
            "crossfeed changed centred bass by {gain_db:+.2} dB"
        );
    }

    #[test]
    fn high_frequencies_keep_their_level() {
        let (out, reference) = run(1.0, 8_000.0, 0.2, 0.2);
        let gain_db = 10.0 * (out / reference).log10();
        assert!(
            gain_db.abs() < 0.6,
            "crossfeed trimmed the treble by {gain_db:+.2} dB"
        );
    }
}
