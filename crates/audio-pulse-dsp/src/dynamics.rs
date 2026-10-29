//! Stereo-linked feed-forward compressor and look-ahead brickwall limiter.
//!
//! The compressor detects RMS power, not sample peaks, and its sidechain is
//! high-passed at 70 Hz. A peak detector reacts to isolated transients and to
//! deep bass that the ear barely localizes, ducking the whole mix for a kick
//! drum. RMS plus a low-cut sidechain keeps the gain tied to perceived loudness
//! while the bass stays untouched; the limiter still catches single samples.

use crate::biquad::Biquad;
use crate::params::{CompressorParams, LimiterParams};

/// Sidechain low-cut. Deep bass is left out of the detector so it cannot pump
/// the midrange; peaks are still handled by the limiter.
const SIDECHAIN_HP_HZ: f32 = 70.0;

fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

fn smoothing_coeff(time_ms: f32, sample_rate: f32) -> f32 {
    let seconds = time_ms.max(0.05) / 1_000.0;
    (1.0 - (-1.0 / (seconds * sample_rate)).exp()).clamp(0.0, 1.0)
}

pub struct Compressor {
    sample_rate: f32,
    enabled: bool,
    threshold_db: f32,
    ratio: f32,
    knee_db: f32,
    makeup: f32,
    attack: f32,
    release: f32,
    gain: f32,
    gain_reduction_db: f32,
    sidechain_hpf: bool,
    sc_l: Biquad,
    sc_r: Biquad,
    rms_power: f32,
}

impl Compressor {
    pub fn new(sample_rate: f32) -> Self {
        let mut c = Self {
            sample_rate,
            enabled: false,
            threshold_db: -18.0,
            ratio: 2.0,
            knee_db: 6.0,
            makeup: 1.0,
            attack: 0.1,
            release: 0.05,
            gain: 1.0,
            gain_reduction_db: 0.0,
            sidechain_hpf: true,
            sc_l: Biquad::identity(),
            sc_r: Biquad::identity(),
            rms_power: 0.0,
        };
        c.update_sidechain();
        c.set_params(CompressorParams::default(), sample_rate);
        c
    }

    /// The musical compressor leaves deep bass out of its detector. The
    /// loud-sound protection is a safety stage and must attenuate every
    /// frequency by the same amount, so it turns the sidechain filter off.
    fn update_sidechain(&mut self) {
        if self.sidechain_hpf {
            self.sc_l
                .set_high_pass(self.sample_rate, SIDECHAIN_HP_HZ, 0.707);
            self.sc_r
                .set_high_pass(self.sample_rate, SIDECHAIN_HP_HZ, 0.707);
        } else {
            self.sc_l.assign(&Biquad::identity());
            self.sc_r.assign(&Biquad::identity());
        }
    }

    pub fn set_params(&mut self, params: CompressorParams, sample_rate: f32) {
        let was_enabled = self.enabled;
        self.enabled = params.enabled;
        self.threshold_db = params.threshold_db;
        self.ratio = params.ratio.max(1.0);
        self.knee_db = params.knee_db.max(0.0);
        self.makeup = db_to_lin(params.makeup_db);
        self.attack = smoothing_coeff(params.attack_ms, sample_rate);
        self.release = smoothing_coeff(params.release_ms, sample_rate);
        let rate_changed = (sample_rate - self.sample_rate).abs() > 1.0;
        if rate_changed {
            self.sample_rate = sample_rate;
        }
        let hpf_changed = params.sidechain_hpf != self.sidechain_hpf;
        self.sidechain_hpf = params.sidechain_hpf;
        if rate_changed || hpf_changed {
            self.update_sidechain();
        }
        // Re-enabling without a reset would reuse the sidechain filter history
        // from before the bypass; clear it together with the detector.
        if !params.enabled || !was_enabled {
            self.reset();
        }
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.enabled {
            return (l, r);
        }
        let side_l = self.sc_l.process(l);
        let side_r = self.sc_r.process(r);
        let power = 0.5 * (side_l * side_l + side_r * side_r);
        let coeff = if power > self.rms_power {
            self.attack
        } else {
            self.release
        };
        self.rms_power += (power - self.rms_power) * coeff;
        let level_db = 10.0 * self.rms_power.max(1e-12).log10();
        let over = level_db - self.threshold_db;
        let knee_half = self.knee_db * 0.5;
        let slope = 1.0 / self.ratio - 1.0;

        let reduction_db = if over <= -knee_half {
            0.0
        } else if over < knee_half && self.knee_db > 0.0 {
            slope * (over + knee_half).powi(2) / (2.0 * self.knee_db)
        } else {
            slope * over
        };

        let target = db_to_lin(reduction_db);
        let coeff = if target < self.gain {
            self.attack
        } else {
            self.release
        };
        self.gain += (target - self.gain) * coeff;
        self.gain_reduction_db = 20.0 * self.gain.max(1e-6).log10();

        let g = self.gain * self.makeup;
        (l * g, r * g)
    }

    pub fn gain_reduction_db(&self) -> f32 {
        self.gain_reduction_db.min(0.0)
    }

    pub fn reset(&mut self) {
        self.gain = 1.0;
        self.gain_reduction_db = 0.0;
        self.rms_power = 0.0;
        self.sc_l.reset();
        self.sc_r.reset();
    }
}

/// Taps in the inter-sample peak interpolator.
const TRUE_PEAK_TAPS: usize = 8;
/// Interior positions evaluated between two samples (4× oversampling).
const TRUE_PEAK_PHASES: [f32; 3] = [0.25, 0.5, 0.75];

/// Lagrange coefficients for one fractional position inside the centre segment
/// of an 8-sample window. Lagrange is exact for polynomials up to degree 7,
/// which keeps the interpolated peak within a few hundredths of a dB even for
/// tones close to Nyquist, where short cubic interpolators under-read.
fn lagrange_coeffs(t: f32) -> [f32; TRUE_PEAK_TAPS] {
    let mut coeffs = [0.0f32; TRUE_PEAK_TAPS];
    for (i, coeff) in coeffs.iter_mut().enumerate() {
        let xi = i as f64 - 3.0;
        let mut num = 1.0f64;
        let mut den = 1.0f64;
        for j in 0..TRUE_PEAK_TAPS {
            if i == j {
                continue;
            }
            let xj = j as f64 - 3.0;
            num *= t as f64 - xj;
            den *= xi - xj;
        }
        *coeff = (num / den) as f32;
    }
    coeffs
}

/// Look-ahead limiter.
///
/// A sliding-window minimum of the required gain is computed from the incoming
/// (future) samples. The audio itself is delayed by the same look-ahead, so the
/// gain applied to each sample can never exceed the gain required to keep it
/// below the ceiling. A slow release smooths the upward movement of that
/// minimum; attacks remain instantaneous, guaranteeing the ceiling.
///
/// The detector combines sample peaks with a 4× inter-sample estimate, so a
/// waveform whose samples are under the ceiling but whose reconstructed peak
/// overshoots is still caught. The estimate lags the input by four samples,
/// which stays well inside the look-ahead.
pub struct Limiter {
    sample_rate: f32,
    enabled: bool,
    ceiling: f32,
    release: f32,
    lookahead: usize,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    ring_len: usize,
    write: usize,
    gain_hold: f32,
    gain_reduction_db: f32,
    history_l: [f32; TRUE_PEAK_TAPS],
    history_r: [f32; TRUE_PEAK_TAPS],
    interp: [[f32; TRUE_PEAK_TAPS]; 3],
    // Monotonic deque of (sequence index, gain) for sliding-window minimum.
    q_index: Vec<u64>,
    q_value: Vec<f32>,
    q_head: usize,
    q_tail: usize,
    q_len: usize,
    seq: u64,
}

impl Limiter {
    pub fn new(sample_rate: f32) -> Self {
        let lookahead = ((sample_rate * 0.005).ceil() as usize).clamp(1, 4_095);
        let ring_len = lookahead + 1;
        let mut limiter = Self {
            sample_rate,
            enabled: true,
            ceiling: db_to_lin(-0.3),
            release: 0.01,
            lookahead,
            buffer_l: vec![0.0; ring_len],
            buffer_r: vec![0.0; ring_len],
            ring_len,
            write: 0,
            gain_hold: 1.0,
            gain_reduction_db: 0.0,
            history_l: [0.0; TRUE_PEAK_TAPS],
            history_r: [0.0; TRUE_PEAK_TAPS],
            interp: [
                lagrange_coeffs(TRUE_PEAK_PHASES[0]),
                lagrange_coeffs(TRUE_PEAK_PHASES[1]),
                lagrange_coeffs(TRUE_PEAK_PHASES[2]),
            ],
            q_index: vec![0; lookahead + 2],
            q_value: vec![0.0; lookahead + 2],
            q_head: 0,
            q_tail: 0,
            q_len: 0,
            seq: 0,
        };
        limiter.set_params(LimiterParams::default(), sample_rate);
        limiter
    }

    pub fn set_params(&mut self, params: LimiterParams, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.enabled = params.enabled;
        self.ceiling = db_to_lin(params.ceiling_db.clamp(-24.0, 0.0));
        self.release = smoothing_coeff(params.release_ms, sample_rate);
        if !params.enabled {
            self.reset();
        }
    }

    pub fn lookahead_samples(&self) -> usize {
        if self.enabled {
            self.lookahead
        } else {
            0
        }
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.enabled {
            return (l, r);
        }

        self.history_l.rotate_left(1);
        self.history_r.rotate_left(1);
        self.history_l[TRUE_PEAK_TAPS - 1] = l;
        self.history_r[TRUE_PEAK_TAPS - 1] = r;

        let peak = l.abs().max(r.abs()).max(self.inter_sample_peak());
        let target = if peak > self.ceiling {
            self.ceiling / peak.max(1e-9)
        } else {
            1.0
        };

        // Maintain a monotonic queue with increasing gains from front to back.
        while self.q_len > 0 {
            let back = (self.q_tail + self.q_index.len() - 1) % self.q_index.len();
            if self.q_value[back] >= target {
                self.q_tail = back;
                self.q_len -= 1;
            } else {
                break;
            }
        }
        self.q_index[self.q_tail] = self.seq;
        self.q_value[self.q_tail] = target;
        self.q_tail = (self.q_tail + 1) % self.q_index.len();
        self.q_len += 1;

        let window = (self.lookahead + 1) as u64;
        while self.q_len > 0 && self.q_index[self.q_head] + window <= self.seq {
            self.q_head = (self.q_head + 1) % self.q_index.len();
            self.q_len -= 1;
        }
        let min_gain = if self.q_len > 0 {
            self.q_value[self.q_head]
        } else {
            1.0
        };

        self.gain_hold = if min_gain < self.gain_hold {
            min_gain
        } else {
            self.gain_hold + (min_gain - self.gain_hold) * self.release
        };
        self.gain_reduction_db = 20.0 * self.gain_hold.max(1e-6).log10();

        // Delay by `lookahead` samples.
        self.buffer_l[self.write] = l;
        self.buffer_r[self.write] = r;
        let read = (self.write + 1) % self.ring_len;
        let out_l = self.buffer_l[read] * self.gain_hold;
        let out_r = self.buffer_r[read] * self.gain_hold;

        self.write = read;
        self.seq = self.seq.wrapping_add(1);
        (out_l, out_r)
    }

    /// Highest reconstructed magnitude between the two centre samples of the
    /// history, evaluated at the three interior quarter positions.
    fn inter_sample_peak(&self) -> f32 {
        let mut peak = 0.0f32;
        for coeffs in &self.interp {
            let mut l = 0.0f32;
            let mut r = 0.0f32;
            for ((&coeff, &hl), &hr) in coeffs.iter().zip(&self.history_l).zip(&self.history_r) {
                l += coeff * hl;
                r += coeff * hr;
            }
            peak = peak.max(l.abs()).max(r.abs());
        }
        peak
    }

    pub fn gain_reduction_db(&self) -> f32 {
        self.gain_reduction_db.min(0.0)
    }

    pub fn reset(&mut self) {
        self.buffer_l.fill(0.0);
        self.buffer_r.fill(0.0);
        self.write = 0;
        self.gain_hold = 1.0;
        self.gain_reduction_db = 0.0;
        self.history_l.fill(0.0);
        self.history_r.fill(0.0);
        self.q_head = 0;
        self.q_tail = 0;
        self.q_len = 0;
        self.seq = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compressor_reduces_loud_signal() {
        let sr = 48_000.0;
        let mut c = Compressor::new(sr);
        c.set_params(
            CompressorParams {
                enabled: true,
                threshold_db: -24.0,
                ratio: 4.0,
                attack_ms: 2.0,
                release_ms: 80.0,
                knee_db: 3.0,
                makeup_db: 0.0,
                ..Default::default()
            },
            sr,
        );
        let mut peak = 0.0f32;
        for i in 0..48_000 {
            let x = 0.9 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / sr).sin();
            let (l, _) = c.process(x, x);
            if i > 10_000 {
                peak = peak.max(l.abs());
            }
        }
        assert!(peak < 0.5, "compressor peak {peak}");
    }

    #[test]
    fn limiter_never_exceeds_ceiling() {
        let sr = 48_000.0;
        let mut limiter = Limiter::new(sr);
        limiter.set_params(
            LimiterParams {
                enabled: true,
                ceiling_db: -1.0,
                release_ms: 40.0,
            },
            sr,
        );
        let ceiling = db_to_lin(-1.0);
        let mut peak = 0.0f32;
        for i in 0..48_000 {
            // 6 dB above full scale.
            let x = 2.0 * (2.0 * std::f32::consts::PI * 330.0 * i as f32 / sr).sin();
            let (l, _) = limiter.process(x, x);
            if i > 1_000 {
                peak = peak.max(l.abs());
            }
        }
        assert!(peak <= ceiling + 1e-4, "peak {peak} ceiling {ceiling}");
    }

    #[test]
    fn limiter_catches_inter_sample_peaks() {
        let sr = 48_000.0;
        let ceiling_db = -1.0;
        let ceiling = db_to_lin(ceiling_db);
        let mut limiter = Limiter::new(sr);
        limiter.set_params(
            LimiterParams {
                enabled: true,
                ceiling_db,
                release_ms: 40.0,
            },
            sr,
        );
        // fs/4 at 45°: every sample sits at ±0.707 of the waveform peak, so no
        // individual sample reaches the ceiling even though the waveform does.
        let freq = sr / 4.0;
        let mut sample_peak = 0.0f32;
        let mut output_peak = 0.0f32;
        for i in 0..24_000 {
            let phase =
                2.0 * std::f32::consts::PI * freq * i as f32 / sr + std::f32::consts::FRAC_PI_4;
            let x = phase.sin();
            let (l, _) = limiter.process(x, x);
            if i > 4_000 {
                sample_peak = sample_peak.max(x.abs());
                output_peak = output_peak.max(l.abs());
            }
        }
        assert!(
            sample_peak < ceiling * 0.95,
            "test signal must stay under the ceiling: {sample_peak}"
        );
        // The analog peak of this tone is output_peak / sin(45°).
        let analog_peak = output_peak / std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            analog_peak <= ceiling * 1.03,
            "reconstructed peak {analog_peak} exceeds ceiling {ceiling}"
        );
    }

    #[test]
    fn compressor_ignores_deep_bass() {
        let sr = 48_000.0;
        let params = CompressorParams {
            enabled: true,
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 5.0,
            release_ms: 100.0,
            knee_db: 3.0,
            makeup_db: 0.0,
            ..Default::default()
        };
        let mut bass = Compressor::new(sr);
        let mut mid = Compressor::new(sr);
        bass.set_params(params.clone(), sr);
        mid.set_params(params, sr);
        for i in 0..48_000 {
            let t = i as f32 / sr;
            let low = 0.6 * (2.0 * std::f32::consts::PI * 30.0 * t).sin();
            let tone = 0.6 * (2.0 * std::f32::consts::PI * 1_000.0 * t).sin();
            bass.process(low, low);
            mid.process(tone, tone);
        }
        assert!(
            bass.gain_reduction_db() > -1.0,
            "deep bass ducked the mix: {} dB",
            bass.gain_reduction_db()
        );
        assert!(
            mid.gain_reduction_db() < -5.0,
            "midrange was not compressed: {} dB",
            mid.gain_reduction_db()
        );
    }
}
