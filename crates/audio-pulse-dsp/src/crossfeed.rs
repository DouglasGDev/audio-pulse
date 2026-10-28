//! Analog-style crossfeed (bs2b family) for headphone listening.
//!
//! A delayed, low-passed copy of the opposite channel is mixed into each ear.
//! This reduces the unnatural hard separation of stereo recordings and
//! centralizes low frequencies, at the cost of a slightly narrower image.

use crate::biquad::Biquad;
use crate::delay::FractionalDelay;
use crate::params::CrossfeedParams;

pub struct CrossfeedStage {
    sample_rate: f32,
    enabled: bool,
    amount: f32,
    delay_samples: f32,
    lp_l: Biquad,
    lp_r: Biquad,
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
        self.delay_samples = (params.delay_ms.clamp(0.05, 1.0) * sample_rate / 1_000.0)
            .clamp(1.0, 480.0);
        self.lp_l = Biquad::low_pass(sample_rate, 700.0, 0.707);
        self.lp_r = Biquad::low_pass(sample_rate, 700.0, 0.707);
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
        let compensation = 1.0 / (1.0 + g * 0.55);
        (
            (l + delayed_l * g) * compensation,
            (r + delayed_r * g) * compensation,
        )
    }

    pub fn reset(&mut self) {
        self.lp_l.reset();
        self.lp_r.reset();
        self.delay_l.reset();
        self.delay_r.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crossfeed_reduces_side_energy_of_a_low_tone() {
        let sr = 48_000.0;
        let mut stage = CrossfeedStage::new(sr);
        stage.set_params(
            CrossfeedParams {
                enabled: true,
                amount: 0.9,
                delay_ms: 0.3,
            },
            sr,
        );
        let mut side_in = 0.0f32;
        let mut side_out = 0.0f32;
        let n = 24_000;
        for i in 0..n {
            let x = 0.2 * (2.0 * std::f32::consts::PI * 120.0 * i as f32 / sr).sin();
            let (ol, or) = stage.process(x, 0.0);
            if i > 4_000 {
                side_in += (x - 0.0f32).abs();
                side_out += (ol - or).abs();
            }
        }
        assert!(side_out < side_in * 0.75, "in {side_in} out {side_out}");
    }
}