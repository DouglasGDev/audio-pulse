//! Volume-dependent loudness compensation based on ISO 226:2003.
//!
//! At low listening levels the ear loses sensitivity at the frequency extremes
//! much faster than in the midrange. We compute the difference between the
//! equal-loudness contour at a 80 phon reference and the contour at the level
//! implied by the current master volume, normalize it at 1 kHz and implement
//! the resulting curve with ten peaking filters.
//!
//! The curve is a *relative* contour, so it would stack up to +18 dB of boost
//! on top of everything else at low volumes. To keep the chain headroom-safe,
//! the stage subtracts the curve's largest boost from the whole signal: the
//! tonal balance is preserved, but the output never exceeds the input peak.
//! The level trim is smoothed over 20 ms so moving the volume slider does not
//! step the waveform.

use crate::biquad::Biquad;
use crate::params::{EQ_BAND_COUNT, EQ_FREQUENCIES};

const FREQUENCIES: [f64; 29] = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
    500.0, 630.0, 800.0, 1_000.0, 1_250.0, 1_600.0, 2_000.0, 2_500.0, 3_150.0, 4_000.0, 5_000.0,
    6_300.0, 8_000.0, 10_000.0, 12_500.0,
];

const ALPHA: [f64; 29] = [
    0.532, 0.506, 0.480, 0.455, 0.432, 0.409, 0.387, 0.367, 0.349, 0.330, 0.315, 0.301, 0.288,
    0.276, 0.267, 0.259, 0.253, 0.250, 0.246, 0.244, 0.243, 0.243, 0.243, 0.242, 0.242, 0.245,
    0.254, 0.271, 0.301,
];

const L_U: [f64; 29] = [
    -31.6, -27.2, -23.0, -19.1, -15.9, -13.0, -10.3, -8.1, -6.2, -4.5, -3.1, -2.0, -1.1, -0.4, 0.0,
    0.3, 0.5, 0.0, -2.7, -4.1, -1.0, 1.7, 2.5, 1.2, -2.1, -7.1, -11.2, -10.7, -3.1,
];

const T_F: [f64; 29] = [
    78.5, 68.7, 59.5, 51.1, 44.0, 37.5, 31.5, 26.5, 22.1, 17.9, 14.4, 11.4, 8.6, 6.2, 4.4, 3.0,
    2.2, 2.4, 3.5, 1.7, -1.3, -4.2, -6.0, -5.4, -1.5, 6.0, 12.6, 13.9, 12.3,
];

fn interpolate(table: &[f64; 29], freq: f64) -> f64 {
    let f = freq.clamp(20.0, 20_000.0);
    // The standard is defined up to 12.5 kHz. Above it the 20 kHz values are
    // reused, following the reference implementation of ISO 226.
    if f > 12_500.0 {
        let t = (f / 12_500.0).ln() / (20_000.0f64 / 12_500.0).ln();
        return table[0] + (table[28] - table[0]) * t.clamp(0.0, 1.0);
    }
    let idx = FREQUENCIES.partition_point(|&x| x < f).clamp(1, 28);
    let f0 = FREQUENCIES[idx - 1];
    let f1 = FREQUENCIES[idx];
    let t = (f / f0).ln() / (f1 / f0).ln();
    table[idx - 1] + (table[idx] - table[idx - 1]) * t
}

/// Sound pressure level (dB) of a pure tone at `freq` that is perceived as
/// `phon` loud. ISO 226:2003 formula.
pub fn spl_for_phon(freq: f64, phon: f64) -> f64 {
    let phon = phon.clamp(0.0, 90.0);
    let alpha = interpolate(&ALPHA, freq);
    let lu = interpolate(&L_U, freq);
    let tf = interpolate(&T_F, freq);
    let a = 0.00447 * (10f64.powf(0.025 * phon) - 1.15)
        + (0.4 * 10f64.powf(((tf + lu) / 10.0) - 9.0)).powf(alpha);
    (10.0 / alpha) * a.max(1e-30).log10() - lu + 94.0
}

/// Relative gain in dB for each EQ band frequency at the current volume.
pub fn compensation_curve(volume: f32, intensity: f32) -> [f32; EQ_BAND_COUNT] {
    let volume = volume.clamp(0.0, 1.0) as f64;
    let intensity = intensity.clamp(0.0, 1.0);
    let reference_phon = 80.0;
    let current_phon = 40.0 + 40.0 * volume;

    let mut gains = [0.0f32; EQ_BAND_COUNT];
    if intensity <= 0.0001 || (reference_phon - current_phon).abs() < 0.01 {
        return gains;
    }
    for (i, &f) in EQ_FREQUENCIES.iter().enumerate() {
        // Excess SPL relative to 1 kHz needed for the same loudness. The
        // difference between the current and reference contours is the
        // equal-loudness correction; both terms are already normalized by
        // their phon value.
        let current_excess = spl_for_phon(f as f64, current_phon) - current_phon;
        let reference_excess = spl_for_phon(f as f64, reference_phon) - reference_phon;
        let db = (current_excess - reference_excess) * intensity as f64;
        gains[i] = (db as f32).clamp(-18.0, 18.0);
    }
    gains
}

pub struct LoudnessStage {
    left: [Biquad; EQ_BAND_COUNT],
    right: [Biquad; EQ_BAND_COUNT],
    enabled: bool,
    volume: f32,
    intensity: f32,
    trim_target: f32,
    trim_current: f32,
    trim_coeff: f32,
}

impl LoudnessStage {
    pub fn new(sample_rate: f32) -> Self {
        let mut stage = Self {
            left: [Biquad::identity(); EQ_BAND_COUNT],
            right: [Biquad::identity(); EQ_BAND_COUNT],
            enabled: false,
            volume: 1.0,
            intensity: 0.0,
            trim_target: 1.0,
            trim_current: 1.0,
            trim_coeff: 1.0 - (-1.0 / (0.020 * sample_rate.max(8_000.0))).exp(),
        };
        stage.set_params(sample_rate, 1.0, 0.0, false);
        stage
    }

    pub fn set_params(&mut self, sample_rate: f32, volume: f32, intensity: f32, enabled: bool) {
        let changed = (self.volume - volume).abs() > 0.002
            || (self.intensity - intensity).abs() > 0.002
            || self.enabled != enabled;
        self.volume = volume;
        self.intensity = intensity;
        self.enabled = enabled;
        if !changed {
            return;
        }
        if !enabled {
            for i in 0..EQ_BAND_COUNT {
                self.left[i] = Biquad::identity();
                self.right[i] = Biquad::identity();
            }
            self.trim_target = 1.0;
            self.trim_current = 1.0;
            return;
        }
        let gains = compensation_curve(volume, intensity);
        let max_boost = gains.iter().cloned().fold(0.0f32, f32::max);
        self.trim_target = 10f32.powf(-max_boost.max(0.0) / 20.0);
        for i in 0..EQ_BAND_COUNT {
            // Broad overlapping filters approximate the smooth ISO contour.
            // Retuning keeps the filter memory, so volume changes do not
            // restart the state and click.
            let q = 1.15;
            self.left[i].set_peaking(sample_rate, EQ_FREQUENCIES[i], gains[i], q);
            self.right[i].set_peaking(sample_rate, EQ_FREQUENCIES[i], gains[i], q);
        }
    }

    #[inline]
    pub fn process(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        if !self.enabled {
            return (l, r);
        }
        for i in 0..EQ_BAND_COUNT {
            l = self.left[i].process(l);
            r = self.right[i].process(r);
        }
        self.trim_current += (self.trim_target - self.trim_current) * self.trim_coeff;
        (l * self.trim_current, r * self.trim_current)
    }

    /// Linear trim currently applied after the contour. Exposed for tests.
    pub fn current_trim(&self) -> f32 {
        self.trim_current
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_khz_equals_phon() {
        for phon in [20.0, 40.0, 60.0, 80.0] {
            let spl = spl_for_phon(1_000.0, phon);
            assert!((spl - phon).abs() < 0.1, "phon {phon} spl {spl}");
        }
    }

    #[test]
    fn compensation_is_flat_at_unity_and_boosts_extremes_when_quiet() {
        let unity = compensation_curve(1.0, 1.0);
        assert!(unity.iter().all(|g| g.abs() < 1e-3));
        let quiet = compensation_curve(0.3, 1.0);
        assert!(quiet[0] > 2.0, "low boost {:?}", quiet);
        assert!(quiet[EQ_BAND_COUNT - 1] > 1.0, "high boost {:?}", quiet);
        assert!(quiet[5].abs() < 1.0, "mid should stay flat {:?}", quiet);
    }

    fn stage_output_peak(volume: f32, freq: f32, amp: f32) -> (f32, f32) {
        let sr = 48_000.0f32;
        let mut stage = LoudnessStage::new(sr);
        stage.set_params(sr, volume, 1.0, true);
        let mut peak = 0.0f32;
        let mut rms = 0.0f64;
        let mut count = 0usize;
        let frames = 48_000;
        for i in 0..frames {
            let x = amp * (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin();
            let (l, _) = stage.process(x, x);
            if i >= frames / 2 {
                peak = peak.max(l.abs());
                rms += (l as f64) * (l as f64);
                count += 1;
            }
        }
        (peak, (rms / count as f64).sqrt() as f32)
    }

    #[test]
    fn quiet_playback_never_exceeds_the_input_peak() {
        let (peak, _) = stage_output_peak(0.25, 1_000.0, 0.99);
        assert!(
            peak <= 1.0,
            "loudness stage overshot with full-scale input: {peak}"
        );
    }

    #[test]
    fn quiet_playback_keeps_the_relative_bass_boost() {
        let sr = 48_000.0f32;
        let mut stage = LoudnessStage::new(sr);
        stage.set_params(sr, 0.25, 1.0, true);
        for _ in 0..(sr as usize / 2) {
            stage.process(0.0, 0.0);
        }
        let trim_db = 20.0 * stage.current_trim().log10();
        let gains = compensation_curve(0.25, 1.0);
        let peak_boost = gains.iter().cloned().fold(0.0f32, f32::max);
        assert!(
            (trim_db + peak_boost).abs() < 0.05,
            "trim {trim_db} dB does not match the curve peak +{peak_boost} dB"
        );
    }
}
