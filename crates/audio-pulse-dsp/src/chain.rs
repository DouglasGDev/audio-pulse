//! Full processing chain. The chain is mono-threaded and allocation-free while
//! audio is flowing; filter coefficients are rebuilt only when a parameter
//! block actually changes.

use crate::adaptive_eq::AdaptiveEqStage;
use crate::bass::BassStage;
use crate::crossfeed::CrossfeedStage;
use crate::dynamics::{Compressor, Limiter};
use crate::equalizer::{GraphicEq, ParametricEq, ToneStage};
use crate::loudness::LoudnessStage;
use crate::params::{ProcessingParams, ADAPTIVE_EQ_BANDS, SPECTRUM_BANDS};
use crate::spatial::SpatialStage;
use crate::spectrum::{LevelTracker, SpectrumAnalyzer};

#[derive(Debug, Clone)]
pub struct AnalysisSnapshot {
    pub bands: [f32; SPECTRUM_BANDS],
    pub rms_l: f32,
    pub rms_r: f32,
    pub peak_l: f32,
    pub peak_r: f32,
    pub compressor_gr_db: f32,
    pub limiter_gr_db: f32,
    pub adaptive_gains_db: [f32; ADAPTIVE_EQ_BANDS],
}

impl Default for AnalysisSnapshot {
    fn default() -> Self {
        Self {
            bands: [0.0; SPECTRUM_BANDS],
            rms_l: 0.0,
            rms_r: 0.0,
            peak_l: 0.0,
            peak_r: 0.0,
            compressor_gr_db: 0.0,
            limiter_gr_db: 0.0,
            adaptive_gains_db: [0.0; ADAPTIVE_EQ_BANDS],
        }
    }
}

/// A discrete change (preset, bypass, spatial mode) swaps filter coefficients
/// and gains in one sample, which steps the waveform and clicks. The processor
/// fades out over a couple of milliseconds, applies the pending block at zero,
/// then fades back in. Continuous controls skip the fade: their changes are
/// small enough per update to stay click-free.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Transition {
    Idle,
    FadeOut,
    FadeIn,
}

const DECLICK_FADE_OUT_SECONDS: f32 = 0.002;
const DECLICK_FADE_IN_SECONDS: f32 = 0.030;

pub struct Processor {
    sample_rate: f32,
    params: ProcessingParams,
    last_params: Option<ProcessingParams>,
    pending_params: Option<ProcessingParams>,
    transition: Transition,
    transition_gain: f32,
    fade_out_step: f32,
    fade_in_step: f32,
    preamp: f32,
    auto_preamp: f32,
    graphic: GraphicEq,
    parametric: ParametricEq,
    tone: ToneStage,
    bass: BassStage,
    spatial: SpatialStage,
    crossfeed: CrossfeedStage,
    adaptive_eq: AdaptiveEqStage,
    compressor: Compressor,
    limiter: Limiter,
    loudness: LoudnessStage,
    volume_current: f32,
    volume_target: f32,
    volume_coeff: f32,
    master_volume_enabled: bool,
    bypass: bool,
    analyzer: SpectrumAnalyzer,
    levels: LevelTracker,
    analysis_bands: [f32; SPECTRUM_BANDS],
}

fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

impl Processor {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8_000.0);
        let mut processor = Self {
            sample_rate: sr,
            params: ProcessingParams::default(),
            last_params: None,
            pending_params: None,
            transition: Transition::Idle,
            transition_gain: 1.0,
            fade_out_step: 1.0 / (DECLICK_FADE_OUT_SECONDS * sr).max(1.0),
            fade_in_step: 1.0 / (DECLICK_FADE_IN_SECONDS * sr).max(1.0),
            preamp: 1.0,
            auto_preamp: 1.0,
            graphic: GraphicEq::new(sr),
            parametric: ParametricEq::new(),
            tone: ToneStage::new(sr),
            bass: BassStage::new(sr),
            spatial: SpatialStage::new(sr),
            crossfeed: CrossfeedStage::new(sr),
            adaptive_eq: AdaptiveEqStage::new(sr),
            compressor: Compressor::new(sr),
            limiter: Limiter::new(sr),
            loudness: LoudnessStage::new(sr),
            volume_current: 1.0,
            volume_target: 1.0,
            volume_coeff: 1.0 - (-1.0 / (0.02 * sr)).exp(),
            master_volume_enabled: true,
            bypass: false,
            analyzer: SpectrumAnalyzer::new(sr),
            levels: LevelTracker::default(),
            analysis_bands: [0.0; SPECTRUM_BANDS],
        };
        processor.set_params(&ProcessingParams::default());
        processor.volume_current = processor.volume_target;
        processor
    }

    /// Applies a block of parameters immediately. Used for continuous
    /// controls (sliders), whose small steps do not need a fade.
    pub fn set_params(&mut self, params: &ProcessingParams) {
        self.apply_or_defer(params, false);
    }

    /// Applies a block of parameters behind a short fade. Preset changes,
    /// bypass toggles and spatial mode switches step the waveform hard enough
    /// to click, so they go through the transition instead.
    pub fn set_params_declick(&mut self, params: &ProcessingParams) {
        self.apply_or_defer(params, true);
    }

    fn apply_or_defer(&mut self, params: &ProcessingParams, declick: bool) {
        if declick || self.transition != Transition::Idle {
            // A newer block replaces an older pending one; the fade keeps
            // running so rapid switches still land on the last request.
            self.pending_params = Some(params.clone());
            if self.transition == Transition::Idle {
                self.transition = Transition::FadeOut;
            }
            return;
        }
        self.commit(params);
    }

    fn commit(&mut self, params: &ProcessingParams) {
        let new = params.clone().sanitized();
        let old = self.last_params.replace(new.clone());

        self.bypass = new.bypass;
        self.preamp = db_to_lin(new.preamp_db);
        self.auto_preamp = if new.auto_eq.enabled {
            db_to_lin(new.auto_eq.preamp_db)
        } else {
            1.0
        };

        match &old {
            None => {
                self.graphic.set_gains(&new.eq_db, self.sample_rate);
                self.parametric
                    .set_filters(&new.auto_eq.filters, self.sample_rate);
                self.tone
                    .set_params(self.sample_rate, new.tone.mid_db, new.tone.treble_db);
                self.bass
                    .set_params(new.bass.clone(), self.sample_rate);
                self.spatial
                    .set_params(new.spatial.clone(), self.sample_rate);
                self.crossfeed
                    .set_params(new.crossfeed.clone(), self.sample_rate);
                self.adaptive_eq
                    .set_params(new.adaptive_eq, self.sample_rate);
                self.compressor
                    .set_params(new.compressor.clone(), self.sample_rate);
                self.limiter
                    .set_params(new.limiter.clone(), self.sample_rate);
            }
            Some(old) => {
                if new.eq_db != old.eq_db {
                    self.graphic.set_gains(&new.eq_db, self.sample_rate);
                }
                if new.auto_eq != old.auto_eq {
                    self.parametric
                        .set_filters(&new.auto_eq.filters, self.sample_rate);
                }
                if new.tone != old.tone {
                    self.tone
                        .set_params(self.sample_rate, new.tone.mid_db, new.tone.treble_db);
                }
                if new.bass != old.bass {
                    self.bass.set_params(new.bass.clone(), self.sample_rate);
                }
                if new.spatial != old.spatial {
                    self.spatial
                        .set_params(new.spatial.clone(), self.sample_rate);
                }
                if new.crossfeed != old.crossfeed {
                    self.crossfeed
                        .set_params(new.crossfeed.clone(), self.sample_rate);
                }
                if new.adaptive_eq != old.adaptive_eq {
                    self.adaptive_eq
                        .set_params(new.adaptive_eq, self.sample_rate);
                }
                if new.compressor != old.compressor {
                    self.compressor
                        .set_params(new.compressor.clone(), self.sample_rate);
                }
                if new.limiter != old.limiter {
                    self.limiter
                        .set_params(new.limiter.clone(), self.sample_rate);
                }
                if new.bypass != old.bypass {
                    self.limiter.reset();
                    self.compressor.reset();
                }
            }
        }

        self.loudness.set_params(
            self.sample_rate,
            new.volume,
            new.loudness.intensity,
            new.loudness.enabled,
        );
        self.volume_target = new.volume;
        if old.is_none() {
            self.volume_current = self.volume_target;
        }
        self.params = new;
    }

    /// When the Windows endpoint owns the master volume, the DSP must not
    /// scale the signal a second time. Loudness keeps using `params.volume`.
    pub fn set_master_volume_enabled(&mut self, enabled: bool) {
        if enabled == self.master_volume_enabled {
            return;
        }
        if enabled {
            self.volume_current = self.volume_target;
        }
        self.master_volume_enabled = enabled;
    }

    #[inline]
    fn master_gain(&mut self) -> f32 {
        self.volume_current += (self.volume_target - self.volume_current) * self.volume_coeff;
        self.volume_current
    }

    /// Advances the declick envelope and applies the pending block at zero.
    #[inline]
    fn transition_gain(&mut self) -> f32 {
        match self.transition {
            Transition::Idle => 1.0,
            Transition::FadeOut => {
                self.transition_gain -= self.fade_out_step;
                if self.transition_gain <= 0.0 {
                    self.transition_gain = 0.0;
                    if let Some(pending) = self.pending_params.take() {
                        self.commit(&pending);
                    }
                    self.transition = Transition::FadeIn;
                }
                self.transition_gain
            }
            Transition::FadeIn => {
                self.transition_gain += self.fade_in_step;
                if self.transition_gain >= 1.0 {
                    self.transition_gain = 1.0;
                    self.transition = Transition::Idle;
                }
                self.transition_gain
            }
        }
    }

    #[inline]
    fn process_stereo(&mut self, mut l: f32, mut r: f32) -> (f32, f32) {
        let fade = self.transition_gain();
        if self.bypass {
            // The master is a level control, not processing: bypassing the
            // chain must not remove the user's volume.
            if self.master_volume_enabled {
                let gain = self.master_gain();
                l *= gain;
                r *= gain;
            }
            l *= fade;
            r *= fade;
            return (l, r);
        }

        let gain = self.preamp;
        l *= gain;
        r *= gain;

        if self.params.auto_eq.enabled {
            let g = self.auto_preamp;
            l *= g;
            r *= g;
            (l, r) = self.parametric.process(l, r);
        }

        (l, r) = self.graphic.process(l, r);
        (l, r) = self.tone.process(l, r);
        (l, r) = self.bass.process(l, r);
        (l, r) = self.spatial.process(l, r);
        (l, r) = self.crossfeed.process(l, r);
        (l, r) = self.adaptive_eq.process(l, r);
        (l, r) = self.compressor.process(l, r);

        if self.master_volume_enabled {
            let gain = self.master_gain();
            l *= gain;
            r *= gain;
        }

        (l, r) = self.loudness.process(l, r);
        (l, r) = self.limiter.process(l, r);

        if !l.is_finite() {
            l = 0.0;
        }
        if !r.is_finite() {
            r = 0.0;
        }
        l *= fade;
        r *= fade;
        (l, r)
    }

    /// Processes an interleaved stereo buffer in place.
    pub fn process_interleaved(&mut self, buffer: &mut [f32]) {
        let frames = buffer.len() / 2;
        if frames == 0 {
            return;
        }
        let mut sum_l = 0.0f64;
        let mut sum_r = 0.0f64;
        let mut peak_l = 0.0f32;
        let mut peak_r = 0.0f32;

        for frame in buffer.chunks_exact_mut(2) {
            let (l, r) = self.process_stereo(frame[0], frame[1]);
            frame[0] = l;
            frame[1] = r;
            sum_l += (l as f64) * (l as f64);
            sum_r += (r as f64) * (r as f64);
            peak_l = peak_l.max(l.abs());
            peak_r = peak_r.max(r.abs());
            self.analyzer.push((l + r) * 0.5);
        }

        self.levels.update(
            (sum_l / frames as f64) as f32,
            (sum_r / frames as f64) as f32,
            peak_l,
            peak_r,
        );
        self.levels.decay();
        self.analysis_bands = *self.analyzer.bands();
    }

    pub fn analysis(&self) -> AnalysisSnapshot {
        AnalysisSnapshot {
            bands: self.analysis_bands,
            rms_l: self.levels.rms_l,
            rms_r: self.levels.rms_r,
            peak_l: self.levels.peak_l,
            peak_r: self.levels.peak_r,
            compressor_gr_db: self.compressor.gain_reduction_db(),
            limiter_gr_db: self.limiter.gain_reduction_db(),
            adaptive_gains_db: self.adaptive_eq.gains_db(),
        }
    }

    pub fn latency_samples(&self) -> usize {
        self.limiter.lookahead_samples()
    }

    pub fn reset(&mut self) {
        self.graphic.reset();
        self.parametric.reset();
        self.bass.reset();
        self.spatial.reset();
        self.crossfeed.reset();
        self.adaptive_eq.reset();
        self.compressor.reset();
        self.limiter.reset();
        self.loudness.reset();
        self.levels.reset();
        self.volume_current = self.volume_target;
        self.pending_params = None;
        self.transition = Transition::Idle;
        self.transition_gain = 1.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{
        BassParams, CompressorParams, CrossfeedParams, LoudnessParams, SpatialMode, SpatialParams,
    };

    fn run_sine(processor: &mut Processor, freq: f32, amp: f32, seconds: f32) -> Vec<f32> {
        let sr = 48_000.0;
        let frames = (seconds * sr) as usize;
        let mut buffer = vec![0.0f32; frames * 2];
        for i in 0..frames {
            let x = amp * (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin();
            buffer[i * 2] = x;
            buffer[i * 2 + 1] = x;
        }
        processor.process_interleaved(&mut buffer);
        buffer
    }

    #[test]
    fn bypass_is_exact_passthrough() {
        let mut processor = Processor::new(48_000.0);
        let mut params = ProcessingParams {
            bypass: true,
            volume: 1.0,
            ..Default::default()
        };
        params.eq_db[4] = 9.0;
        processor.set_params(&params);
        let input: Vec<f32> = (0..2048)
            .map(|i| (i as f32 * 0.013).sin() * 0.4)
            .collect();
        let mut buffer = Vec::with_capacity(2048);
        for x in &input {
            buffer.push(*x);
            buffer.push(*x);
        }
        processor.process_interleaved(&mut buffer);
        for (i, x) in input.iter().enumerate() {
            assert!((buffer[i * 2] - x).abs() < 1e-6);
        }
    }

    #[test]
    fn bypass_keeps_the_master_volume() {
        let mut processor = Processor::new(48_000.0);
        let params = ProcessingParams {
            bypass: true,
            volume: 0.25,
            ..Default::default()
        };
        processor.set_params(&params);
        processor.set_master_volume_enabled(false);
        processor.set_master_volume_enabled(true);
        let out = run_sine(&mut processor, 440.0, 0.4, 0.1);
        let peak = out.iter().fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        assert!(
            (peak - 0.1).abs() < 0.005,
            "bypass dropped the master gain: peak {peak}"
        );
    }

    #[test]
    fn full_chain_is_finite_and_limited() {
        let sr = 48_000.0;
        let mut processor = Processor::new(sr);
        let params = ProcessingParams {
            preamp_db: 6.0,
            eq_db: [12.0, 12.0, 6.0, 0.0, -3.0, 0.0, 3.0, 6.0, 9.0, 9.0],
            bass: BassParams {
                enabled: true,
                intensity: 1.0,
                cutoff_hz: 120.0,
                harmonics: 1.0,
            },
            compressor: CompressorParams {
                enabled: true,
                makeup_db: 10.0,
                ..Default::default()
            },
            loudness: LoudnessParams {
                enabled: true,
                intensity: 1.0,
            },
            spatial: SpatialParams {
                mode: SpatialMode::Surround,
                width: 1.4,
                room: 0.8,
            },
            crossfeed: CrossfeedParams {
                enabled: true,
                amount: 1.0,
                delay_ms: 0.5,
            },
            volume: 0.6,
            ..Default::default()
        };
        processor.set_params(&params);
        let out = run_sine(&mut processor, 440.0, 0.9, 1.0);
        let ceiling = 10f32.powf(-0.3 / 20.0) + 1e-3;
        for (i, sample) in out.iter().enumerate() {
            assert!(sample.is_finite(), "non-finite at {i}");
            if i > 5_000 {
                assert!(sample.abs() <= ceiling, "clip {sample} at {i}");
            }
        }
    }

    #[test]
    fn master_volume_can_move_to_the_system_mixer() {
        let mut processor = Processor::new(48_000.0);
        let mut params = ProcessingParams {
            volume: 0.0,
            ..Default::default()
        };
        params.auto_eq.enabled = false;
        params.limiter.enabled = false;
        processor.set_params(&params);

        processor.set_master_volume_enabled(false);
        let out = run_sine(&mut processor, 440.0, 0.25, 0.1);
        let peak = out.iter().fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        assert!(peak > 0.2, "delegated master still scaled the signal: {peak}");

        processor.set_master_volume_enabled(true);
        let out = run_sine(&mut processor, 440.0, 0.25, 0.1);
        assert!(out.iter().all(|sample| sample.abs() < 1e-6));
    }

    #[test]
    fn analysis_reports_spectrum_energy() {
        let mut processor = Processor::new(48_000.0);
        let _ = run_sine(&mut processor, 1_000.0, 0.4, 0.5);
        let analysis = processor.analysis();
        assert!(analysis.bands.iter().any(|b| *b > 0.1));
        assert!(analysis.rms_l > 0.0);
    }

    fn max_step(samples: &[f32]) -> f32 {
        samples
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .fold(0.0f32, f32::max)
    }

    /// Continuous-phase 440 Hz tone, so block boundaries do not add steps.
    fn feed_tone(processor: &mut Processor, phase: &mut f32, amp: f32, frames: usize) -> Vec<f32> {
        let sr = 48_000.0f32;
        let mut buffer = vec![0.0f32; frames * 2];
        for i in 0..frames {
            let x = amp * (2.0 * std::f32::consts::PI * 440.0 * *phase / sr).sin();
            *phase += 1.0;
            buffer[i * 2] = x;
            buffer[i * 2 + 1] = x;
        }
        processor.process_interleaved(&mut buffer);
        buffer
    }

    #[test]
    fn declicked_change_has_no_waveform_step() {
        let mut processor = Processor::new(48_000.0);
        let before = ProcessingParams {
            eq_db: [4.0, 3.5, 2.0, 0.0, -1.0, 0.0, 0.5, 0.5, 0.0, 0.0],
            bass: BassParams {
                enabled: true,
                intensity: 0.85,
                cutoff_hz: 70.0,
                harmonics: 0.35,
            },
            ..Default::default()
        };
        processor.set_params(&before);

        let mut phase = 0.0f32;
        let mut steady = Vec::new();
        for _ in 0..20 {
            steady.extend_from_slice(&feed_tone(&mut processor, &mut phase, 0.25, 480));
        }
        let reference = max_step(&steady[steady.len() / 2..]);

        // A preset-sized jump: preamp, the whole EQ curve and the bass stage.
        let after = ProcessingParams {
            preamp_db: -6.0,
            eq_db: [-4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.5, 2.0, 1.0, 0.0],
            ..Default::default()
        };
        processor.set_params_declick(&after);

        let mut switched = Vec::new();
        for _ in 0..20 {
            switched.extend_from_slice(&feed_tone(&mut processor, &mut phase, 0.25, 480));
        }

        let transient = max_step(&switched);
        assert!(
            transient <= reference * 1.5,
            "switch stepped the waveform: {transient} vs steady {reference}"
        );
        assert!(
            switched.iter().all(|sample| sample.is_finite()),
            "non-finite sample after the switch"
        );
        let tail_rms = (switched[switched.len() - 480..]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / 480.0)
            .sqrt();
        assert!(tail_rms > 0.01, "sound did not come back after the fade");
    }

    #[test]
    fn repeated_declick_requests_land_on_the_last_block() {
        let mut processor = Processor::new(48_000.0);
        let mut first = ProcessingParams::default();
        first.eq_db[0] = 6.0;
        let mut second = ProcessingParams::default();
        second.eq_db[1] = -6.0;
        processor.set_params_declick(&first);
        processor.set_params_declick(&second);
        let out = run_sine(&mut processor, 440.0, 0.2, 0.1);
        assert!(out.iter().all(|sample| sample.is_finite()));
        assert_eq!(processor.params.eq_db, second.eq_db);
        assert_eq!(processor.transition, Transition::Idle);
    }
}