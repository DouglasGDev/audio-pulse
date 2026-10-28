//! Bass enhancement: low shelf plus a band-limited harmonic synthesizer.
//!
//! The shelf provides the physical low-frequency boost. The harmonic stage
//! extracts the sub-bass with a 4th-order low-pass, saturates it asymmetrically
//! and removes the original fundamental again. Small drivers cannot reproduce
//! 40–80 Hz, but they can reproduce the generated 100–240 Hz harmonics, which
//! the ear uses to infer the missing fundamental (the "missing fundamental"
//! phenomenon). This is the same family of techniques used by consumer bass
//! enhancement DSPs, implemented here without clipping the main path.

use crate::biquad::{Biquad, OnePole};
use crate::params::BassParams;

pub struct BassStage {
    sample_rate: f32,
    enabled: bool,
    intensity: f32,
    harmonics: f32,
    shelf_l: Biquad,
    shelf_r: Biquad,
    lp_a_l: Biquad,
    lp_b_l: Biquad,
    lp_a_r: Biquad,
    lp_b_r: Biquad,
    hp_l: Biquad,
    hp_r: Biquad,
    drive_l: OnePole,
    drive_r: OnePole,
    dc_l: Biquad,
    dc_r: Biquad,
    last: BassParams,
}

impl BassStage {
    pub fn new(sample_rate: f32) -> Self {
        let mut stage = Self {
            sample_rate,
            enabled: false,
            intensity: 0.0,
            harmonics: 0.0,
            shelf_l: Biquad::identity(),
            shelf_r: Biquad::identity(),
            lp_a_l: Biquad::identity(),
            lp_b_l: Biquad::identity(),
            lp_a_r: Biquad::identity(),
            lp_b_r: Biquad::identity(),
            hp_l: Biquad::identity(),
            hp_r: Biquad::identity(),
            drive_l: OnePole::default(),
            drive_r: OnePole::default(),
            dc_l: Biquad::identity(),
            dc_r: Biquad::identity(),
            last: BassParams::default(),
        };
        stage.set_params(BassParams::default(), sample_rate);
        stage
    }

    pub fn set_params(&mut self, params: BassParams, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.enabled = params.enabled;
        self.intensity = params.intensity;
        self.harmonics = params.harmonics;

        if !params.enabled || params.intensity <= 0.0001 {
            self.last = params;
            return;
        }

        // Rebuild only when values changed enough to matter.
        let changed = (self.last.intensity - params.intensity).abs() > 1e-4
            || (self.last.cutoff_hz - params.cutoff_hz).abs() > 0.5
            || (self.last.harmonics - params.harmonics).abs() > 1e-4
            || !self.last.enabled;
        self.last = params.clone();
        if !changed {
            return;
        }

        let cutoff = params.cutoff_hz.clamp(30.0, 250.0);
        let shelf_gain = 15.0 * params.intensity;
        self.shelf_l = Biquad::low_shelf(sample_rate, cutoff, shelf_gain, 0.707);
        self.shelf_r = Biquad::low_shelf(sample_rate, cutoff, shelf_gain, 0.707);

        // 4th order (two cascaded 2nd order) bass extraction at ~1.6x cutoff.
        let lp_freq = (cutoff * 1.6).clamp(45.0, 380.0);
        self.lp_a_l = Biquad::low_pass(sample_rate, lp_freq, 0.54);
        self.lp_b_l = Biquad::low_pass(sample_rate, lp_freq, 1.31);
        self.lp_a_r = Biquad::low_pass(sample_rate, lp_freq, 0.54);
        self.lp_b_r = Biquad::low_pass(sample_rate, lp_freq, 1.31);

        // Keep harmonics above the fundamental region and remove DC.
        self.hp_l = Biquad::high_pass(sample_rate, (cutoff * 1.35).clamp(60.0, 360.0), 0.707);
        self.hp_r = Biquad::high_pass(sample_rate, (cutoff * 1.35).clamp(60.0, 360.0), 0.707);
        self.dc_l = Biquad::high_pass(sample_rate, 20.0, 0.707);
        self.dc_r = Biquad::high_pass(sample_rate, 20.0, 0.707);
        self.drive_l = OnePole::low_pass(sample_rate, 220.0);
        self.drive_r = OnePole::low_pass(sample_rate, 220.0);
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.enabled || self.intensity <= 0.0001 {
            return (l, r);
        }

        let boosted_l = self.shelf_l.process(l);
        let boosted_r = self.shelf_r.process(r);

        if self.harmonics <= 0.0001 {
            return (boosted_l, boosted_r);
        }

        let bass_l = self.lp_b_l.process(self.lp_a_l.process(l));
        let bass_r = self.lp_b_r.process(self.lp_a_r.process(r));

        let drive = 1.5 + self.intensity * 5.0;
        let shaped_l = (bass_l * drive).tanh() / drive;
        let shaped_r = (bass_r * drive).tanh() / drive;
        let residual_l = shaped_l - bass_l;
        let residual_r = shaped_r - bass_r;

        let harm_l = self.dc_l.process(self.hp_l.process(residual_l));
        let harm_r = self.dc_r.process(self.hp_r.process(residual_r));

        // A gentle low-pass keeps the synthesized layer from sounding gritty.
        let harm_l = self.drive_l.process(harm_l);
        let harm_r = self.drive_r.process(harm_r);

        let mix = 0.9 * self.harmonics * self.intensity;
        (boosted_l + harm_l * mix, boosted_r + harm_r * mix)
    }

    pub fn reset(&mut self) {
        for f in [
            &mut self.shelf_l,
            &mut self.shelf_r,
            &mut self.lp_a_l,
            &mut self.lp_b_l,
            &mut self.lp_a_r,
            &mut self.lp_b_r,
            &mut self.hp_l,
            &mut self.hp_r,
            &mut self.dc_l,
            &mut self.dc_r,
        ] {
            f.reset();
        }
        self.drive_l.reset();
        self.drive_r.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, sample_rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| {
                0.2 * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin()
            })
            .collect()
    }

    #[test]
    fn adds_energy_below_cutoff() {
        let sr = 48_000.0;
        let mut stage = BassStage::new(sr);
        stage.set_params(
            BassParams {
                enabled: true,
                intensity: 0.8,
                cutoff_hz: 80.0,
                harmonics: 0.0,
            },
            sr,
        );
        let input = tone(50.0, sr, 48_000);
        let mut output = Vec::new();
        for &x in &input {
            output.push(stage.process(x, x).0);
        }
        let in_rms = (input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32).sqrt();
        let out_rms =
            (output[8_000..].iter().map(|x| x * x).sum::<f32>() / 40_000.0).sqrt();
        assert!(out_rms > in_rms * 1.5, "in {in_rms} out {out_rms}");
    }
}