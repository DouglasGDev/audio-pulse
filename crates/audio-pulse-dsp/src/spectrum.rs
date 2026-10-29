//! Real-time FFT spectrum analysis with log-spaced bands and level metering.

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

use crate::params::SPECTRUM_BANDS;

const FFT_SIZE: usize = 2_048;
const HOP: usize = 1_024;

pub struct SpectrumAnalyzer {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    window_sum: f32,
    buffer: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    accum: Vec<f32>,
    accum_pos: usize,
    band_ranges: Vec<(usize, usize)>,
    bands: [f32; SPECTRUM_BANDS],
    sample_rate: f32,
    frames: u64,
}

impl SpectrumAnalyzer {
    pub fn new(sample_rate: f32) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|n| {
                0.5 - 0.5 * (2.0 * std::f32::consts::PI * n as f32 / (FFT_SIZE - 1) as f32).cos()
            })
            .collect();
        let window_sum = window.iter().sum::<f32>();
        let scratch_len = fft.get_inplace_scratch_len().max(1);
        let max_freq = (sample_rate * 0.45).min(18_000.0);
        let min_freq = 30.0f32;
        let ratio = (max_freq / min_freq).powf(1.0 / SPECTRUM_BANDS as f32);
        let mut band_ranges = Vec::with_capacity(SPECTRUM_BANDS);
        for band in 0..SPECTRUM_BANDS {
            let low = min_freq * ratio.powi(band as i32);
            let high = min_freq * ratio.powi(band as i32 + 1);
            let bin_hz = sample_rate / FFT_SIZE as f32;
            let low_bin = ((low / bin_hz).floor() as usize).clamp(1, FFT_SIZE / 2 - 1);
            let high_bin = ((high / bin_hz).ceil() as usize).clamp(low_bin + 1, FFT_SIZE / 2);
            band_ranges.push((low_bin, high_bin));
        }
        Self {
            fft,
            window,
            window_sum,
            buffer: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            accum: vec![0.0; FFT_SIZE],
            accum_pos: 0,
            band_ranges,
            bands: [0.0; SPECTRUM_BANDS],
            sample_rate,
            frames: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, mono: f32) {
        self.accum[self.accum_pos] = mono;
        self.accum_pos += 1;
        if self.accum_pos >= FFT_SIZE {
            self.frames += 1;
            self.analyze();
            self.accum.copy_within(HOP..FFT_SIZE, 0);
            self.accum_pos = HOP;
        }
    }

    fn analyze(&mut self) {
        for i in 0..FFT_SIZE {
            self.buffer[i] = Complex::new(self.accum[i] * self.window[i], 0.0);
        }
        self.fft
            .process_with_scratch(&mut self.buffer, &mut self.scratch);

        let norm = 2.0 / self.window_sum;
        for (band, &(low, high)) in self.band_ranges.iter().enumerate() {
            let mut power = 0.0f32;
            let mut count = 0usize;
            for bin in low..high {
                let c = self.buffer[bin];
                power += c.re * c.re + c.im * c.im;
                count += 1;
            }
            let mean_power = power / count.max(1) as f32;
            let amplitude = (mean_power.sqrt() * norm).max(1e-9);
            let db = 20.0 * amplitude.log10();
            // Map -90 dBFS..0 dBFS to 0..1 with a slight low-frequency tilt.
            let mut value = ((db + 90.0) / 90.0).clamp(0.0, 1.0);
            if band < 10 {
                value = (value * 1.06).min(1.0);
            }
            let current = self.bands[band];
            let coeff = if value > current { 0.7 } else { 0.18 };
            self.bands[band] = current + (value - current) * coeff;
        }
    }

    pub fn bands(&self) -> &[f32; SPECTRUM_BANDS] {
        &self.bands
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LevelTracker {
    pub rms_l: f32,
    pub rms_r: f32,
    pub peak_l: f32,
    pub peak_r: f32,
}

impl LevelTracker {
    pub fn update(&mut self, mean_square_l: f32, mean_square_r: f32, peak_l: f32, peak_r: f32) {
        let rms_l = mean_square_l.max(0.0).sqrt().clamp(0.0, 1.5);
        let rms_r = mean_square_r.max(0.0).sqrt().clamp(0.0, 1.5);
        self.rms_l += (rms_l - self.rms_l) * 0.35;
        self.rms_r += (rms_r - self.rms_r) * 0.35;
        self.peak_l = self.peak_l.max(peak_l).min(1.5);
        self.peak_r = self.peak_r.max(peak_r).min(1.5);
    }

    pub fn decay(&mut self) {
        self.peak_l *= 0.92;
        self.peak_r *= 0.92;
        if self.peak_l < 1e-4 {
            self.peak_l = 0.0;
        }
        if self.peak_r < 1e-4 {
            self.peak_r = 0.0;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_energy_lands_in_expected_band() {
        let sr = 48_000.0;
        let mut analyzer = SpectrumAnalyzer::new(sr);
        for i in 0..(FFT_SIZE * 3) {
            let x = 0.5 * (2.0 * std::f32::consts::PI * 1_000.0 * i as f32 / sr).sin();
            analyzer.push(x);
        }
        assert!(analyzer.frames() > 0);
        let bands = analyzer.bands();
        let max_band = (0..SPECTRUM_BANDS)
            .max_by(|&a, &b| bands[a].partial_cmp(&bands[b]).unwrap())
            .unwrap();
        let center =
            30.0 * (18_000.0f32 / 30.0).powf((max_band as f32 + 0.5) / SPECTRUM_BANDS as f32);
        assert!(
            (center - 1_000.0).abs() / 1_000.0 < 0.5,
            "peak band {max_band} center {center}"
        );
    }
}
