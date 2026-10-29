//! Audio Pulse DSP.
//!
//! A real-time-safe stereo processing chain for headphones:
//!
//! ```text
//! preamp → AutoEQ → 10-band EQ → tone → bass/sub → spatial → crossfeed
//!        → adaptive EQ → compressor → volume → ISO 226 loudness
//!        → loud sound reduction → limiter
//! ```
//!
//! The chain never allocates while audio is being processed. Parameter updates
//! are applied at buffer boundaries and only rebuild the stages that changed.

pub mod adaptive_eq;
pub mod autoeq;
pub mod bass;
pub mod biquad;
pub mod chain;
pub mod crossfeed;
pub mod delay;
pub mod dynamics;
pub mod equalizer;
pub mod loudness;
pub mod params;
pub mod presets;
pub mod spatial;
pub mod spectrum;

pub use chain::{AnalysisSnapshot, Processor};
pub use params::{
    AdaptiveEqParams, AutoEqFilter, AutoEqParams, BassParams, CompressorParams, CrossfeedParams,
    EarProfileParams, FilterKind, LimiterParams, LoudSoundReductionParams, LoudnessParams,
    ProcessingParams, SpatialMode, SpatialParams, ToneParams, ADAPTIVE_EQ_BANDS,
    ADAPTIVE_EQ_FREQUENCIES, ADAPTIVE_EQ_MAX_DB, EQ_BAND_COUNT, EQ_FREQUENCIES, MAX_AUTOEQ_FILTERS,
    SPECTRUM_BANDS,
};
pub use presets::{builtin_presets, find_builtin, Preset, PresetCategory};

/// Presets are serialized as a small envelope so users can identify the file
/// and future versions can migrate safely.
pub const PRESET_FORMAT: &str = "audiopulse-preset";
pub const PRESET_FORMAT_VERSION: u32 = 1;
