//! Content-adaptive equalization.
//!
//! Every 20 ms the stage measures five octave bands from an overlapped FFT,
//! compares their energy against a gentle program-material reference, and
//! trims each band by at most ±3 dB. The reference is not flat: it expects
//! roughly +2 dB of low-end and -2 dB of top-end energy relative to the
//! program's own median, which is where most music and cinema sit. That keeps
//! the controller from brightening every mix the way a flat target would.
//!
//! The controller is deterministic, not a neural network: a deadband avoids
//! chasing noise, an asymmetric smoothing (fast attack, slower release) keeps
//! transitions musical, and a predictive headroom rule caps positive gains by
//! the measured peak so the look-ahead limiter is never the first line of
//! defence against clipping. If the input is already at the ceiling, no band
//! is boosted at all. Silence freezes the adaptation instead of lifting the
//! noise floor.

use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use crate::biquad::Biquad;
use crate::params::{
    AdaptiveEqParams, ADAPTIVE_EQ_BANDS, ADAPTIVE_EQ_FREQUENCIES, ADAPTIVE_EQ_MAX_DB,
};

const CONTROL_SECONDS: f32 = 0.020;
/// Reference balance in dB relative to the program's own median band. Music
/// naturally carries more octave energy in the lows, so a flat target would
/// make almost every track brighter.
const TARGET_TILT_DB: [f32; ADAPTIVE_EQ_BANDS] = [2.0, 1.0, 0.0, -1.0, -2.0];
const DEADBAND_DB: f32 = 1.2;
/// Bands this far below the loudest one are treated as empty and never
/// boosted, so the controller does not pull up hiss or room noise.
const ACTIVE_RANGE_DB: f32 = 30.0;
const SILENCE_DB: f32 = -62.0;
const CEILING_DB: f32 = -1.0;
const BAND_Q: [f32; ADAPTIVE_EQ_BANDS] = [0.8, 0.9, 1.0, 1.0, 0.8];
const GAIN_SMOOTHING: f32 = 0.18;

pub struct AdaptiveEqStage {
    sample_rate: f32,
    enabled: bool,
    intensity: f32,
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    window_sum: f32,
    buffer: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    accum: Vec<f32>,
    pos: usize,
    hop: usize,
    fft_size: usize,
    band_bins: [(usize, usize); ADAPTIVE_EQ_BANDS],
    bands_db: [f32; ADAPTIVE_EQ_BANDS],
    gains_db: [f32; ADAPTIVE_EQ_BANDS],
    filters: [Biquad; ADAPTIVE_EQ_BANDS],
    peak: f32,
    has_analysis: bool,
}

impl AdaptiveEqStage {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8_000.0);
        let hop = ((CONTROL_SECONDS * sr).round() as usize).max(64);
        let fft_size = (hop * 2).max(1_024).next_power_of_two();
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        let window: Vec<f32> = (0..fft_size)
            .map(|n| {
                0.5 - 0.5
                    * (2.0 * std::f32::consts::PI * n as f32 / (fft_size - 1) as f32).cos()
            })
            .collect();
        let window_sum = window.iter().sum::<f32>().max(1.0);
        let scratch_len = fft.get_inplace_scratch_len().max(1);
        let bin_hz = sr / fft_size as f32;
        let band_bins: [(usize, usize); ADAPTIVE_EQ_BANDS] = std::array::from_fn(|index| {
            let center = ADAPTIVE_EQ_FREQUENCIES[index];
            let low = (center * std::f32::consts::FRAC_1_SQRT_2) / bin_hz;
            let high = (center * std::f32::consts::SQRT_2) / bin_hz;
            let low_bin = (low.floor() as usize).clamp(1, fft_size / 2 - 1);
            let high_bin = (high.ceil() as usize).clamp(low_bin + 1, fft_size / 2);
            (low_bin, high_bin)
        });

        Self {
            sample_rate: sr,
            enabled: false,
            intensity: 0.6,
            fft,
            window,
            window_sum,
            buffer: vec![Complex::new(0.0, 0.0); fft_size],
            scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            accum: vec![0.0; fft_size],
            pos: 0,
            hop,
            fft_size,
            band_bins,
            bands_db: [-90.0; ADAPTIVE_EQ_BANDS],
            gains_db: [0.0; ADAPTIVE_EQ_BANDS],
            filters: [Biquad::identity(); ADAPTIVE_EQ_BANDS],
            peak: 0.0,
            has_analysis: false,
        }
    }

    pub fn set_params(&mut self, params: AdaptiveEqParams, sample_rate: f32) {
        if (sample_rate - self.sample_rate).abs() > 1.0 {
            *self = Self::new(sample_rate);
        }
        self.intensity = params.intensity.clamp(0.0, 1.0);
        if params.enabled != self.enabled {
            self.enabled = params.enabled;
            self.reset_adaptation();
        }
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.enabled || self.intensity <= 0.0001 {
            return (l, r);
        }

        self.peak = self.peak.max(l.abs()).max(r.abs());
        self.push((l + r) * 0.5);

        let mut left = l;
        let mut right = r;
        for filter in &mut self.filters {
            left = filter.process(left);
            right = filter.process(right);
        }
        (left, right)
    }

    fn push(&mut self, mono: f32) {
        self.accum[self.pos] = mono;
        self.pos += 1;
        if self.pos >= self.fft_size {
            self.analyze();
            self.accum.copy_within(self.hop..self.fft_size, 0);
            self.pos = self.fft_size - self.hop;
        }
    }

    fn analyze(&mut self) {
        for i in 0..self.fft_size {
            self.buffer[i] = Complex::new(self.accum[i] * self.window[i], 0.0);
        }
        self.fft
            .process_with_scratch(&mut self.buffer, &mut self.scratch);

        let norm = 2.0 / self.window_sum;
        let mut raw_db = [-90.0f32; ADAPTIVE_EQ_BANDS];
        for (band, &(low, high)) in self.band_bins.iter().enumerate() {
            let mut power = 0.0f32;
            for bin in low..high {
                let c = self.buffer[bin];
                power += c.re * c.re + c.im * c.im;
            }
            // Summing the bins integrates each octave band's energy, so a
            // pink-ish program reads roughly flat across the five bands.
            let amplitude = (power.sqrt() * norm).max(1e-9);
            raw_db[band] = 20.0 * amplitude.log10();
        }

        if !self.has_analysis {
            self.bands_db = raw_db;
            self.has_analysis = true;
        } else {
            for (band, &raw) in raw_db.iter().enumerate() {
                let coeff = if raw > self.bands_db[band] { 0.5 } else { 0.2 };
                self.bands_db[band] += (raw - self.bands_db[band]) * coeff;
            }
        }

        // Peak from this control window. It is the time-domain peak, not the
        // FFT magnitude, because clipping is a sample-domain event.
        let peak = self.peak.max(1e-6);
        self.peak = 0.0;
        let peak_db = 20.0 * peak.log10();

        let loudest = self
            .bands_db
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);
        if loudest < SILENCE_DB || peak < 1e-4 {
            // Freeze: do not lift the noise floor, do not move the filters.
            return;
        }

        let active: [bool; ADAPTIVE_EQ_BANDS] =
            std::array::from_fn(|band| self.bands_db[band] > loudest - ACTIVE_RANGE_DB);
        if active.iter().filter(|flag| **flag).count() < 2 {
            // A single band carries no balance information.
            return;
        }

        // Centre the target on the active bands so corrections only
        // redistribute energy; the equalizer never applies a net level change.
        let mut anchor_sum = 0.0;
        let mut anchor_count = 0.0;
        for band in 0..ADAPTIVE_EQ_BANDS {
            if active[band] {
                anchor_sum += self.bands_db[band] - TARGET_TILT_DB[band];
                anchor_count += 1.0;
            }
        }
        let anchor = anchor_sum / anchor_count;

        let mut desired = [0.0f32; ADAPTIVE_EQ_BANDS];
        for band in 0..ADAPTIVE_EQ_BANDS {
            if !active[band] {
                continue;
            }
            let target = anchor + TARGET_TILT_DB[band];
            let error = self.bands_db[band] - target;
            let correction = if error > DEADBAND_DB {
                -(error - DEADBAND_DB)
            } else if error < -DEADBAND_DB {
                -(error + DEADBAND_DB)
            } else {
                0.0
            };
            desired[band] = correction.clamp(-ADAPTIVE_EQ_MAX_DB, ADAPTIVE_EQ_MAX_DB);
        }

        // Predictive headroom: the largest positive correction is assumed to
        // act on the loudest sample. If that would cross the ceiling, scale
        // every boost down. Cuts are always allowed.
        let max_boost = desired.iter().cloned().fold(0.0f32, f32::max);
        let headroom_scale = if max_boost > 0.0 && peak_db + max_boost > CEILING_DB {
            ((CEILING_DB - peak_db) / max_boost).clamp(0.0, 1.0)
        } else {
            1.0
        };

        for band in 0..ADAPTIVE_EQ_BANDS {
            let goal = if desired[band] > 0.0 {
                desired[band] * self.intensity * headroom_scale
            } else {
                desired[band] * self.intensity
            };
            self.gains_db[band] += (goal - self.gains_db[band]) * GAIN_SMOOTHING;
            self.filters[band].set_peaking(
                self.sample_rate,
                ADAPTIVE_EQ_FREQUENCIES[band],
                self.gains_db[band],
                BAND_Q[band],
            );
        }
    }

    pub fn gains_db(&self) -> [f32; ADAPTIVE_EQ_BANDS] {
        self.gains_db
    }

    pub fn reset(&mut self) {
        self.reset_adaptation();
    }

    fn reset_adaptation(&mut self) {
        self.accum.fill(0.0);
        self.pos = 0;
        self.peak = 0.0;
        self.has_analysis = false;
        self.bands_db = [-90.0; ADAPTIVE_EQ_BANDS];
        self.gains_db = [0.0; ADAPTIVE_EQ_BANDS];
        for filter in &mut self.filters {
            *filter = Biquad::identity();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_sine(stage: &mut AdaptiveEqStage, freq: f32, amp: f32, seconds: f32) {
        let sr = 48_000.0;
        let frames = (seconds * sr) as usize;
        for i in 0..frames {
            let x = amp * (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin();
            let (l, r) = stage.process(x, x);
            assert!(l.is_finite() && r.is_finite());
        }
    }

    fn enabled_stage() -> AdaptiveEqStage {
        let mut stage = AdaptiveEqStage::new(48_000.0);
        stage.set_params(
            AdaptiveEqParams {
                enabled: true,
                intensity: 1.0,
            },
            48_000.0,
        );
        stage
    }

    #[test]
    fn silence_does_not_move_the_gains() {
        let mut stage = enabled_stage();
        for _ in 0..48_000 {
            let (l, r) = stage.process(0.0, 0.0);
            assert_eq!((l, r), (0.0, 0.0));
        }
        assert!(stage.gains_db().iter().all(|gain| *gain == 0.0));
    }

    #[test]
    fn bass_heavy_program_is_rebalanced_toward_the_target() {
        let mut stage = enabled_stage();
        // Establish the reference with a mid-only tone, then add a loud bass
        // tone: the low band now sits far above the reference tilt.
        feed_sine(&mut stage, 1_000.0, 0.12, 1.5);
        let sr = 48_000.0;
        for i in 0..48_000 * 3 {
            let bass = 0.5 * (2.0 * std::f32::consts::PI * 80.0 * i as f32 / sr).sin();
            let mid = 0.12 * (2.0 * std::f32::consts::PI * 1_000.0 * i as f32 / sr).sin();
            stage.process(bass + mid, bass + mid);
        }
        let gains = stage.gains_db();
        assert!(
            gains[0] < -1.5,
            "low band should be trimmed, got {:?}",
            gains
        );
        assert!(
            gains[2] > 0.3,
            "mid band should be lifted, got {:?}",
            gains
        );
        assert!(gains.iter().all(|gain| gain.abs() <= ADAPTIVE_EQ_MAX_DB + 0.01));
    }

    #[test]
    fn hot_signal_never_gets_a_positive_gain() {
        let mut stage = enabled_stage();
        // Near-full-scale bass with a quiet mid: the controller wants to lift
        // the mid band, but there is no headroom left to do so safely.
        let sr = 48_000.0;
        for i in 0..48_000 * 2 {
            let bass = 0.98 * (2.0 * std::f32::consts::PI * 80.0 * i as f32 / sr).sin();
            let mid = 0.2 * (2.0 * std::f32::consts::PI * 1_000.0 * i as f32 / sr).sin();
            stage.process(bass + mid, bass + mid);
        }
        let gains = stage.gains_db();
        assert!(
            gains.iter().all(|gain| *gain <= 0.05),
            "boosted a near-full-scale signal: {:?}",
            gains
        );
    }

    #[test]
    fn disabled_stage_is_a_passthrough() {
        let mut stage = AdaptiveEqStage::new(48_000.0);
        stage.set_params(AdaptiveEqParams::default(), 48_000.0);
        for i in 0..2_048 {
            let x = (i as f32 * 0.01).sin() * 0.3;
            assert_eq!(stage.process(x, x * 0.5), (x, x * 0.5));
        }
    }
}
