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

/// Full-scale low-shelf gain at `intensity = 1.0`. Presets use the same
/// mapping to compute their static headroom.
pub const BASS_SHELF_MAX_DB: f32 = 15.0;

pub struct BassStage {
    sample_rate: f32,
    enabled: bool,
    intensity: f32,
    harmonics: f32,
    shelf_l: Biquad,
    shelf_r: Biquad,
    lp_a: Biquad,
    lp_b: Biquad,
    hp: Biquad,
    dc: Biquad,
    drive: OnePole,
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
            lp_a: Biquad::identity(),
            lp_b: Biquad::identity(),
            hp: Biquad::identity(),
            dc: Biquad::identity(),
            drive: OnePole::default(),
            last: BassParams::default(),
        };
        stage.set_params(BassParams::default(), sample_rate);
        stage
    }

    pub fn set_params(&mut self, params: BassParams, sample_rate: f32) {
        self.sample_rate = sample_rate;
        let was_enabled = self.last.enabled;
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
            || !was_enabled;
        self.last = params.clone();
        if !changed {
            return;
        }
        // Coming back from a pause leaves stale sample memory in the filters,
        // which would step the waveform; the continuous retunes below keep it.
        if !was_enabled {
            self.reset();
        }

        let cutoff = params.cutoff_hz.clamp(30.0, 250.0);
        let shelf_gain = BASS_SHELF_MAX_DB * params.intensity;
        self.shelf_l
            .set_low_shelf(sample_rate, cutoff, shelf_gain, 0.707);
        self.shelf_r
            .set_low_shelf(sample_rate, cutoff, shelf_gain, 0.707);

        // 4th order (two cascaded 2nd order) bass extraction at ~1.6x cutoff.
        let lp_freq = (cutoff * 1.6).clamp(45.0, 380.0);
        self.lp_a.set_low_pass(sample_rate, lp_freq, 0.54);
        self.lp_b.set_low_pass(sample_rate, lp_freq, 1.31);

        // Keep harmonics above the fundamental region and remove DC.
        self.hp
            .set_high_pass(sample_rate, (cutoff * 1.35).clamp(60.0, 360.0), 0.707);
        self.dc.set_high_pass(sample_rate, 20.0, 0.707);
        self.drive.set_freq(sample_rate, 220.0);
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

        // Bass is not localizable, and consumer playback is almost always
        // centred; deriving one harmonic layer avoids a phasey, widened low
        // end and keeps both channels coherent. Linear filtering commutes with
        // the sum, so this is exactly the same band as averaging L/R.
        let bass = self.lp_b.process(self.lp_a.process(0.5 * (l + r)));

        let drive = 1.5 + self.intensity * 5.0;
        let shaped = (bass * drive).tanh() / drive;
        let residual = shaped - bass;

        let harm = self.dc.process(self.hp.process(residual));
        // A gentle low-pass keeps the synthesized layer from sounding gritty.
        let harm = self.drive.process(harm);

        let mix = 0.9 * self.harmonics * self.intensity;
        (boosted_l + harm * mix, boosted_r + harm * mix)
    }

    pub fn reset(&mut self) {
        for f in [
            &mut self.shelf_l,
            &mut self.shelf_r,
            &mut self.lp_a,
            &mut self.lp_b,
            &mut self.hp,
            &mut self.dc,
        ] {
            f.reset();
        }
        self.drive.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(freq: f32, sample_rate: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| 0.2 * (2.0 * std::f32::consts::PI * freq * i as f32 / sample_rate).sin())
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
        let out_rms = (output[8_000..].iter().map(|x| x * x).sum::<f32>() / 40_000.0).sqrt();
        assert!(out_rms > in_rms * 1.5, "in {in_rms} out {out_rms}");
    }

    fn goertzel(samples: &[f32], freq: f32, sample_rate: f32) -> f32 {
        let w = 2.0 * std::f32::consts::PI * freq / sample_rate;
        let coeff = 2.0 * w.cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &x in samples {
            let s0 = x + coeff * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - coeff * s1 * s2).max(0.0).sqrt() / samples.len() as f32
    }

    #[test]
    fn harmonics_stay_centered() {
        let sr = 48_000.0;
        let mut stage = BassStage::new(sr);
        stage.set_params(
            BassParams {
                enabled: true,
                intensity: 0.8,
                cutoff_hz: 80.0,
                harmonics: 1.0,
            },
            sr,
        );
        let mut left = Vec::with_capacity(48_000);
        let mut right = Vec::with_capacity(48_000);
        for i in 0..48_000 {
            let x = 0.5 * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / sr).sin();
            let (l, r) = stage.process(x, 0.0);
            left.push(l);
            right.push(r);
        }
        // The shelf only boosts the left channel; the synthesized 3rd harmonic
        // must appear at the same level in both, or the bass would smear.
        let h3_l = goertzel(&left[24_000..], 150.0, sr);
        let h3_r = goertzel(&right[24_000..], 150.0, sr);
        assert!(h3_l > 1e-4, "no harmonic content was generated: {h3_l}");
        assert!(
            (h3_l - h3_r).abs() / h3_l < 0.02,
            "harmonics are not centred: L {h3_l} R {h3_r}"
        );
    }
}
