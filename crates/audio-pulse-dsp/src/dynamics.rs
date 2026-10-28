//! Stereo-linked feed-forward compressor and look-ahead brickwall limiter.

use crate::params::{CompressorParams, LimiterParams};

fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

fn smoothing_coeff(time_ms: f32, sample_rate: f32) -> f32 {
    let seconds = time_ms.max(0.05) / 1_000.0;
    (1.0 - (-1.0 / (seconds * sample_rate)).exp()).clamp(0.0, 1.0)
}

pub struct Compressor {
    enabled: bool,
    threshold_db: f32,
    ratio: f32,
    knee_db: f32,
    makeup: f32,
    attack: f32,
    release: f32,
    gain: f32,
    gain_reduction_db: f32,
}

impl Compressor {
    pub fn new(sample_rate: f32) -> Self {
        let mut c = Self {
            enabled: false,
            threshold_db: -18.0,
            ratio: 2.0,
            knee_db: 6.0,
            makeup: 1.0,
            attack: 0.1,
            release: 0.05,
            gain: 1.0,
            gain_reduction_db: 0.0,
        };
        c.set_params(CompressorParams::default(), sample_rate);
        c
    }

    pub fn set_params(&mut self, params: CompressorParams, sample_rate: f32) {
        self.enabled = params.enabled;
        self.threshold_db = params.threshold_db;
        self.ratio = params.ratio.max(1.0);
        self.knee_db = params.knee_db.max(0.0);
        self.makeup = db_to_lin(params.makeup_db);
        self.attack = smoothing_coeff(params.attack_ms, sample_rate);
        self.release = smoothing_coeff(params.release_ms, sample_rate);
        if !params.enabled {
            self.gain = 1.0;
            self.gain_reduction_db = 0.0;
        }
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.enabled {
            return (l, r);
        }
        let peak = l.abs().max(r.abs()).max(1e-9);
        let level_db = 20.0 * peak.log10();
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
    }
}

/// Look-ahead limiter.
///
/// A sliding-window minimum of the required gain is computed from the incoming
/// (future) samples. The audio itself is delayed by the same look-ahead, so the
/// gain applied to each sample can never exceed the gain required to keep it
/// below the ceiling. A slow release smooths the upward movement of that
/// minimum; attacks remain instantaneous, guaranteeing the ceiling.
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

        let peak = l.abs().max(r.abs());
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

    pub fn gain_reduction_db(&self) -> f32 {
        self.gain_reduction_db.min(0.0)
    }

    pub fn reset(&mut self) {
        self.buffer_l.fill(0.0);
        self.buffer_r.fill(0.0);
        self.write = 0;
        self.gain_hold = 1.0;
        self.gain_reduction_db = 0.0;
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
        assert!(
            peak <= ceiling + 1e-4,
            "peak {peak} ceiling {ceiling}"
        );
    }
}