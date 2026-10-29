//! Serializable processing parameters shared by the engine, presets and UI.

use serde::{Deserialize, Serialize};

pub const EQ_BAND_COUNT: usize = 10;
pub const EQ_FREQUENCIES: [f32; EQ_BAND_COUNT] = [
    31.0, 62.0, 125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];
pub const MAX_AUTOEQ_FILTERS: usize = 16;
pub const SPECTRUM_BANDS: usize = 56;
pub const ADAPTIVE_EQ_BANDS: usize = 5;
pub const ADAPTIVE_EQ_FREQUENCIES: [f32; ADAPTIVE_EQ_BANDS] =
    [80.0, 300.0, 1_000.0, 3_500.0, 11_000.0];
pub const ADAPTIVE_EQ_MAX_DB: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SpatialMode {
    #[default]
    Off,
    Binaural,
    /// Content-adaptive binaural rendering: the virtual pair opens up as the
    /// mix decorrelates, so ITD/ILD tracking follows the stereo image.
    Hrtf,
    Surround,
}

impl SpatialMode {
    pub fn cycle(self) -> Self {
        match self {
            Self::Off => Self::Binaural,
            Self::Binaural => Self::Hrtf,
            Self::Hrtf => Self::Surround,
            Self::Surround => Self::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FilterKind {
    #[default]
    Peaking,
    Lowshelf,
    Highshelf,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutoEqFilter {
    #[serde(default)]
    pub kind: FilterKind,
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
}

impl Default for AutoEqFilter {
    fn default() -> Self {
        Self {
            kind: FilterKind::Peaking,
            freq: 1_000.0,
            gain_db: 0.0,
            q: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct BassParams {
    pub enabled: bool,
    /// 0..1, maps to up to +15 dB of low shelf.
    pub intensity: f32,
    pub cutoff_hz: f32,
    /// Amount of synthesized second/third harmonic content, 0..1.
    pub harmonics: f32,
}

impl Default for BassParams {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.45,
            cutoff_hz: 90.0,
            harmonics: 0.35,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ToneParams {
    pub mid_db: f32,
    pub treble_db: f32,
}

impl Default for ToneParams {
    fn default() -> Self {
        Self {
            mid_db: 0.0,
            treble_db: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct CompressorParams {
    pub enabled: bool,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub knee_db: f32,
    pub makeup_db: f32,
}

impl Default for CompressorParams {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_db: -18.0,
            ratio: 2.0,
            attack_ms: 10.0,
            release_ms: 150.0,
            knee_db: 6.0,
            makeup_db: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct LimiterParams {
    pub enabled: bool,
    pub ceiling_db: f32,
    pub release_ms: f32,
}

impl Default for LimiterParams {
    fn default() -> Self {
        Self {
            enabled: true,
            ceiling_db: -0.3,
            release_ms: 80.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct LoudnessParams {
    pub enabled: bool,
    /// 0..1 blend between flat and full ISO 226 compensation.
    pub intensity: f32,
}

impl Default for LoudnessParams {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.6,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct LoudSoundReductionParams {
    pub enabled: bool,
    /// 0..1. Higher values lower the threshold and raise the ratio.
    pub intensity: f32,
}

impl Default for LoudSoundReductionParams {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.6,
        }
    }
}

impl LoudSoundReductionParams {
    /// A slow, stereo-linked compressor acting after the master volume, so it
    /// follows the level actually reaching the ears.
    pub fn to_compressor(&self) -> CompressorParams {
        let intensity = self.intensity.clamp(0.0, 1.0);
        CompressorParams {
            enabled: self.enabled && intensity > 0.01,
            threshold_db: -10.0 - 14.0 * intensity,
            ratio: 1.5 + 2.5 * intensity,
            attack_ms: 5.0,
            release_ms: 200.0,
            knee_db: 8.0,
            makeup_db: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct SpatialParams {
    pub mode: SpatialMode,
    /// 0..1.5 virtual speaker spread.
    pub width: f32,
    /// 0..1 ambience / early reflection amount.
    pub room: f32,
}

impl Default for SpatialParams {
    fn default() -> Self {
        Self {
            mode: SpatialMode::Off,
            width: 1.0,
            room: 0.35,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct CrossfeedParams {
    pub enabled: bool,
    /// 0..1, maps to roughly -6 dB of cross-channel bleed.
    pub amount: f32,
    pub delay_ms: f32,
}

impl Default for CrossfeedParams {
    fn default() -> Self {
        Self {
            enabled: false,
            amount: 0.3,
            delay_ms: 0.3,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AdaptiveEqParams {
    pub enabled: bool,
    /// 0..1 blend between flat and the full computed correction.
    pub intensity: f32,
}

impl Default for AdaptiveEqParams {
    fn default() -> Self {
        Self {
            enabled: false,
            intensity: 0.55,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct AutoEqParams {
    pub enabled: bool,
    pub preamp_db: f32,
    pub filters: Vec<AutoEqFilter>,
}

impl Default for AutoEqParams {
    fn default() -> Self {
        Self {
            enabled: true,
            preamp_db: 0.0,
            filters: Vec::new(),
        }
    }
}

/// Personal correction measured from the user's ear photo. It has its own
/// stage between AutoEQ and the graphic EQ, so it composes with every other
/// correction instead of replacing the headphone profile.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct EarProfileParams {
    pub enabled: bool,
    pub preamp_db: f32,
    pub filters: Vec<AutoEqFilter>,
}

impl Default for EarProfileParams {
    fn default() -> Self {
        Self {
            enabled: false,
            preamp_db: 0.0,
            filters: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ProcessingParams {
    pub preamp_db: f32,
    pub eq_db: [f32; EQ_BAND_COUNT],
    pub bass: BassParams,
    pub tone: ToneParams,
    pub compressor: CompressorParams,
    pub limiter: LimiterParams,
    pub loudness: LoudnessParams,
    pub loud_sound_reduction: LoudSoundReductionParams,
    pub spatial: SpatialParams,
    pub crossfeed: CrossfeedParams,
    pub auto_eq: AutoEqParams,
    pub adaptive_eq: AdaptiveEqParams,
    /// User-owned ear correction; presets never carry it (see
    /// `apply_processing_from`).
    pub ear_profile: EarProfileParams,
    /// Master output volume, linear 0..1.
    pub volume: f32,
    pub bypass: bool,
}

impl Default for ProcessingParams {
    fn default() -> Self {
        Self {
            preamp_db: 0.0,
            eq_db: [0.0; EQ_BAND_COUNT],
            bass: BassParams::default(),
            tone: ToneParams::default(),
            compressor: CompressorParams::default(),
            limiter: LimiterParams::default(),
            loudness: LoudnessParams::default(),
            loud_sound_reduction: LoudSoundReductionParams::default(),
            spatial: SpatialParams::default(),
            crossfeed: CrossfeedParams::default(),
            auto_eq: AutoEqParams::default(),
            adaptive_eq: AdaptiveEqParams::default(),
            ear_profile: EarProfileParams::default(),
            volume: 1.0,
            bypass: false,
        }
    }
}

fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        fallback
    }
}

fn clamp(value: f32, min: f32, max: f32, fallback: f32) -> f32 {
    finite(value, fallback).clamp(min, max)
}

impl ProcessingParams {
    /// Clamps every parameter into its safe, audible range. Called at the UI
    /// boundary and again when presets are loaded from disk.
    pub fn sanitized(mut self) -> Self {
        self.preamp_db = clamp(self.preamp_db, -24.0, 12.0, 0.0);
        for band in &mut self.eq_db {
            *band = clamp(*band, -12.0, 12.0, 0.0);
        }

        self.bass.intensity = clamp(self.bass.intensity, 0.0, 1.0, 0.0);
        self.bass.cutoff_hz = clamp(self.bass.cutoff_hz, 30.0, 250.0, 90.0);
        self.bass.harmonics = clamp(self.bass.harmonics, 0.0, 1.0, 0.0);

        self.tone.mid_db = clamp(self.tone.mid_db, -12.0, 12.0, 0.0);
        self.tone.treble_db = clamp(self.tone.treble_db, -12.0, 12.0, 0.0);

        self.compressor.threshold_db = clamp(self.compressor.threshold_db, -60.0, 0.0, -18.0);
        self.compressor.ratio = clamp(self.compressor.ratio, 1.0, 20.0, 2.0);
        self.compressor.attack_ms = clamp(self.compressor.attack_ms, 0.1, 200.0, 10.0);
        self.compressor.release_ms = clamp(self.compressor.release_ms, 20.0, 2_000.0, 150.0);
        self.compressor.knee_db = clamp(self.compressor.knee_db, 0.0, 24.0, 6.0);
        self.compressor.makeup_db = clamp(self.compressor.makeup_db, 0.0, 24.0, 0.0);

        self.limiter.ceiling_db = clamp(self.limiter.ceiling_db, -12.0, 0.0, -0.3);
        self.limiter.release_ms = clamp(self.limiter.release_ms, 10.0, 1_000.0, 80.0);

        self.loudness.intensity = clamp(self.loudness.intensity, 0.0, 1.0, 0.0);

        self.loud_sound_reduction.intensity =
            clamp(self.loud_sound_reduction.intensity, 0.0, 1.0, 0.6);

        self.spatial.width = clamp(self.spatial.width, 0.0, 1.5, 1.0);
        self.spatial.room = clamp(self.spatial.room, 0.0, 1.0, 0.35);

        self.crossfeed.amount = clamp(self.crossfeed.amount, 0.0, 1.0, 0.0);
        self.crossfeed.delay_ms = clamp(self.crossfeed.delay_ms, 0.05, 1.0, 0.3);

        self.adaptive_eq.intensity = clamp(self.adaptive_eq.intensity, 0.0, 1.0, 0.55);

        self.auto_eq.preamp_db = clamp(self.auto_eq.preamp_db, -24.0, 12.0, 0.0);
        self.auto_eq.filters.truncate(MAX_AUTOEQ_FILTERS);
        for filter in &mut self.auto_eq.filters {
            filter.freq = clamp(filter.freq, 20.0, 20_000.0, 1_000.0);
            filter.gain_db = clamp(filter.gain_db, -24.0, 24.0, 0.0);
            filter.q = clamp(filter.q, 0.1, 18.0, 1.0);
        }

        self.ear_profile.preamp_db = clamp(self.ear_profile.preamp_db, -24.0, 12.0, 0.0);
        self.ear_profile.filters.truncate(MAX_AUTOEQ_FILTERS);
        for filter in &mut self.ear_profile.filters {
            filter.freq = clamp(filter.freq, 20.0, 20_000.0, 1_000.0);
            filter.gain_db = clamp(filter.gain_db, -24.0, 24.0, 0.0);
            filter.q = clamp(filter.q, 0.1, 18.0, 1.0);
        }

        self.volume = clamp(self.volume, 0.0, 1.0, 1.0);
        self
    }

    /// True when no shaping block would alter the signal, i.e. a freshly
    /// created configuration that has never received a preset.
    pub fn is_neutral(&self) -> bool {
        const EPS: f32 = 0.01;
        self.preamp_db.abs() < EPS
            && self.eq_db.iter().all(|gain| gain.abs() < EPS)
            && !self.bass.enabled
            && self.tone.mid_db.abs() < EPS
            && self.tone.treble_db.abs() < EPS
            && !self.compressor.enabled
            && !self.loudness.enabled
            && !self.loud_sound_reduction.enabled
            && self.spatial.mode == SpatialMode::Off
            && !self.crossfeed.enabled
            && !self.adaptive_eq.enabled
            && self.auto_eq.filters.is_empty()
    }

    /// Copies the sound-shaping fields from another parameter set while
    /// preserving user-owned state: master volume, bypass and the ear profile.
    /// Hearing protection is part of the preset, so applying one switches it on
    /// or off.
    pub fn apply_processing_from(&mut self, other: &Self) {
        self.preamp_db = other.preamp_db;
        self.eq_db = other.eq_db;
        self.bass = other.bass.clone();
        self.tone = other.tone.clone();
        self.compressor = other.compressor.clone();
        self.limiter = other.limiter.clone();
        self.loudness = other.loudness.clone();
        self.loud_sound_reduction = other.loud_sound_reduction.clone();
        self.spatial = other.spatial.clone();
        self.crossfeed = other.crossfeed.clone();
        self.auto_eq = other.auto_eq.clone();
        self.adaptive_eq = other.adaptive_eq;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_out_of_range_and_nan() {
        let mut p = ProcessingParams::default();
        p.eq_db[0] = f32::NAN;
        p.preamp_db = 99.0;
        p.bass.cutoff_hz = 10.0;
        p.loud_sound_reduction.intensity = f32::NAN;
        p.auto_eq.filters = vec![AutoEqFilter::default(); 30];
        let p = p.sanitized();
        assert_eq!(p.eq_db[0], 0.0);
        assert_eq!(p.preamp_db, 12.0);
        assert_eq!(p.bass.cutoff_hz, 30.0);
        assert_eq!(p.loud_sound_reduction.intensity, 0.6);
        assert_eq!(p.auto_eq.filters.len(), MAX_AUTOEQ_FILTERS);
    }

    #[test]
    fn neutral_only_for_flat_defaults() {
        assert!(ProcessingParams::default().is_neutral());
        let mut shaped = ProcessingParams::default();
        shaped.eq_db[3] = 2.0;
        assert!(!shaped.is_neutral());
        let mut bass = ProcessingParams::default();
        bass.bass.enabled = true;
        assert!(!bass.is_neutral());
    }

    #[test]
    fn preset_apply_preserves_volume_and_bypass() {
        let mut current = ProcessingParams {
            volume: 0.42,
            bypass: true,
            loud_sound_reduction: LoudSoundReductionParams {
                enabled: true,
                intensity: 0.8,
            },
            ..Default::default()
        };
        let mut preset = ProcessingParams::default();
        preset.eq_db[0] = 6.0;
        preset.volume = 1.0;
        preset.loud_sound_reduction = LoudSoundReductionParams::default();
        current.apply_processing_from(&preset);
        assert_eq!(current.volume, 0.42);
        assert!(current.bypass);
        assert_eq!(current.eq_db[0], 6.0);
        assert!(!current.loud_sound_reduction.enabled);
        assert_eq!(current.loud_sound_reduction.intensity, 0.6);
    }

    #[test]
    fn preset_apply_preserves_the_ear_profile() {
        let ear = EarProfileParams {
            enabled: true,
            preamp_db: -1.2,
            filters: vec![AutoEqFilter {
                kind: FilterKind::Peaking,
                freq: 3_400.0,
                gain_db: -2.0,
                q: 1.15,
            }],
        };
        let mut current = ProcessingParams {
            ear_profile: ear.clone(),
            ..Default::default()
        };
        let mut preset = ProcessingParams::default();
        preset.bass.enabled = true;
        // A stale preset that happens to carry another ear profile must not win.
        preset.ear_profile.enabled = true;
        preset.ear_profile.filters = vec![AutoEqFilter::default()];
        current.apply_processing_from(&preset);
        assert_eq!(current.ear_profile, ear);
        assert!(current.bass.enabled);
    }

    #[test]
    fn sanitizes_ear_profile_filters() {
        let mut params = ProcessingParams::default();
        params.ear_profile.filters = vec![AutoEqFilter::default(); 30];
        params.ear_profile.filters[0].freq = f32::NAN;
        params.ear_profile.preamp_db = -99.0;
        let params = params.sanitized();
        assert_eq!(params.ear_profile.filters.len(), MAX_AUTOEQ_FILTERS);
        assert_eq!(params.ear_profile.filters[0].freq, 1_000.0);
        assert_eq!(params.ear_profile.preamp_db, -24.0);
    }
}