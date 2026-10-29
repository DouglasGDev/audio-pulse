//! Built-in presets: listening modes and measured headphone corrections.
//!
//! Headphone corrections use the AutoEQ parametric model (MIT License) and are
//! computed from public measurements published by Rtings and oratory1990.
//! They are compensation targets for the listed model, not measurements of
//! the user's own unit: fit, earcup sealing and unit variation still change the
//! result. Any measured AutoEQ or OPRA profile can also be imported at runtime
//! and behaves exactly like these built-ins.

// Filter Q factors such as 3.14 are data from the published corrections, not π.
#![allow(clippy::approx_constant)]

use serde::{Deserialize, Serialize};

use crate::bass::BASS_SHELF_MAX_DB;
use crate::biquad::Biquad;
use crate::equalizer::EQ_Q;
use crate::params::{
    AutoEqFilter, AutoEqParams, BassParams, CompressorParams, CrossfeedParams, FilterKind,
    LimiterParams, LoudnessParams, ProcessingParams, SpatialMode, SpatialParams, EQ_FREQUENCIES,
};

/// Boost a mode may keep before the preamp compensates it. Without an
/// allowance every mode would be noticeably quieter than Flat at the same
/// Windows volume.
pub const PRESET_HEADROOM_ALLOWANCE_DB: f32 = 3.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PresetCategory {
    Music,
    Cinema,
    Gaming,
    Podcast,
    Vocal,
    Clarity,
    Immersive,
    Flat,
    Bass,
    Night,
    Protection,
    Headphone,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub category: PresetCategory,
    pub description: String,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub built_in: bool,
    pub params: ProcessingParams,
}

fn pk(freq: f32, gain_db: f32, q: f32) -> AutoEqFilter {
    AutoEqFilter {
        kind: FilterKind::Peaking,
        freq,
        gain_db,
        q,
    }
}

fn ls(freq: f32, gain_db: f32, q: f32) -> AutoEqFilter {
    AutoEqFilter {
        kind: FilterKind::Lowshelf,
        freq,
        gain_db,
        q,
    }
}

fn hs(freq: f32, gain_db: f32, q: f32) -> AutoEqFilter {
    AutoEqFilter {
        kind: FilterKind::Highshelf,
        freq,
        gain_db,
        q,
    }
}

/// Largest static gain the graphic EQ cascade, the tone controls and the bass
/// shelf can add, in dB, sampled on a log grid at 48 kHz.
///
/// This runs while presets are built (or imported), never in the audio thread.
/// It is what lets a mode ship with a preamp that keeps its boosts out of the
/// limiter instead of letting the limiter ride the curve permanently.
fn max_static_boost_db(params: &ProcessingParams) -> f32 {
    const SR: f32 = 48_000.0;
    let eq: Vec<Biquad> = EQ_FREQUENCIES
        .iter()
        .enumerate()
        .map(|(index, &freq)| Biquad::peaking(SR, freq, params.eq_db[index], EQ_Q))
        .collect();
    let shelf = (params.bass.enabled && params.bass.intensity > 0.0001).then(|| {
        Biquad::low_shelf(
            SR,
            params.bass.cutoff_hz.clamp(30.0, 250.0),
            BASS_SHELF_MAX_DB * params.bass.intensity,
            0.707,
        )
    });
    let mut peak = 0.0f32;
    let mut freq = 20.0f32;
    while freq < 20_000.0 {
        let mut gain: f32 = eq.iter().map(|filter| filter.magnitude_db(freq, SR)).sum();
        if let Some(shelf) = &shelf {
            gain += shelf.magnitude_db(freq, SR);
        }
        peak = peak.max(gain);
        freq *= 1.03;
    }
    // The tone filters overlap the EQ grid only partially; adding both positive
    // gains is conservative, and presets currently leave tone at zero.
    peak + params.tone.mid_db.max(0.0) + params.tone.treble_db.max(0.0)
}

fn mode(
    id: &str,
    name: &str,
    category: PresetCategory,
    description: &str,
    configure: impl FnOnce(&mut ProcessingParams),
) -> Preset {
    let mut params = ProcessingParams::default();
    params.limiter.enabled = true;
    configure(&mut params);
    let headroom = (max_static_boost_db(&params) - PRESET_HEADROOM_ALLOWANCE_DB).max(0.0);
    params.preamp_db = (params.preamp_db - headroom).clamp(-24.0, 12.0);
    Preset {
        id: id.into(),
        name: name.into(),
        category,
        description: description.into(),
        brand: None,
        model: None,
        built_in: true,
        params,
    }
}

fn headphone(
    id: &str,
    brand: &str,
    model: &str,
    source: &str,
    preamp_db: f32,
    filters: Vec<AutoEqFilter>,
) -> Preset {
    let params = ProcessingParams {
        auto_eq: AutoEqParams {
            enabled: true,
            preamp_db,
            filters,
            name: None,
        },
        limiter: LimiterParams {
            enabled: true,
            ..Default::default()
        },
        ..Default::default()
    };
    Preset {
        id: id.into(),
        name: format!("{brand} {model}"),
        category: PresetCategory::Headphone,
        description: format!(
            "Correção medida via AutoEQ ({source}). Alvo de compensação: o resultado varia \
             com a vedação, a unidade e o encaixe."
        ),
        brand: Some(brand.into()),
        model: Some(model.into()),
        built_in: true,
        params,
    }
}

fn mode_presets() -> Vec<Preset> {
    vec![
        mode(
            "music",
            "Música",
            PresetCategory::Music,
            "Curva equilibrada, graves sutis e compensação de loudness moderada.",
            |p| {
                p.eq_db = [1.0, 0.8, 0.3, 0.0, -0.4, 0.0, 0.3, 0.6, 0.8, 0.5];
                p.bass = BassParams {
                    enabled: true,
                    intensity: 0.25,
                    cutoff_hz: 85.0,
                    harmonics: 0.25,
                };
                p.loudness = LoudnessParams {
                    enabled: true,
                    intensity: 0.5,
                };
            },
        ),
        mode(
            "cinema",
            "Cinema",
            PresetCategory::Cinema,
            "Palco amplo, LFE reforçado e diálogos centralizados.",
            |p| {
                p.eq_db = [2.5, 2.0, 1.0, 0.0, -0.5, 0.0, 1.0, 1.5, 1.0, 0.0];
                p.bass = BassParams {
                    enabled: true,
                    intensity: 0.5,
                    cutoff_hz: 75.0,
                    harmonics: 0.4,
                };
                p.spatial = SpatialParams {
                    mode: SpatialMode::Surround,
                    width: 1.1,
                    room: 0.5,
                };
                p.compressor = CompressorParams {
                    enabled: true,
                    threshold_db: -20.0,
                    ratio: 2.2,
                    attack_ms: 15.0,
                    release_ms: 200.0,
                    knee_db: 6.0,
                    makeup_db: 2.0,
                    ..Default::default()
                };
                p.loudness = LoudnessParams {
                    enabled: true,
                    intensity: 0.4,
                };
            },
        ),
        mode(
            "gaming",
            "Jogos",
            PresetCategory::Gaming,
            "Passos nítidos, palco amplo e dinâmica controlada.",
            |p| {
                p.eq_db = [0.5, 0.5, 0.0, -1.0, 0.0, 1.0, 2.0, 1.5, 0.5, 0.0];
                p.bass = BassParams {
                    enabled: true,
                    intensity: 0.25,
                    cutoff_hz: 85.0,
                    harmonics: 0.2,
                };
                p.spatial = SpatialParams {
                    mode: SpatialMode::Surround,
                    width: 1.3,
                    room: 0.25,
                };
                p.crossfeed = CrossfeedParams {
                    enabled: true,
                    amount: 0.15,
                    delay_ms: 0.3,
                };
                p.compressor = CompressorParams {
                    enabled: true,
                    threshold_db: -22.0,
                    ratio: 2.0,
                    attack_ms: 5.0,
                    release_ms: 100.0,
                    knee_db: 6.0,
                    makeup_db: 2.0,
                    ..Default::default()
                };
                p.limiter.ceiling_db = -0.5;
            },
        ),
        mode(
            "competitive",
            "Competitivo",
            PresetCategory::Gaming,
            "Passos e direções em primeiro plano: graves contidos, presença realçada e reflexões mínimas.",
            |p| {
                p.eq_db = [-3.0, -2.0, -1.0, -0.5, 0.0, 1.0, 2.5, 3.0, 1.5, 0.0];
                p.spatial = SpatialParams {
                    mode: SpatialMode::Hrtf,
                    width: 1.1,
                    room: 0.1,
                };
                p.compressor = CompressorParams {
                    enabled: true,
                    threshold_db: -26.0,
                    ratio: 2.5,
                    attack_ms: 3.0,
                    release_ms: 90.0,
                    knee_db: 4.0,
                    makeup_db: 1.0,
                    ..Default::default()
                };
                p.limiter.ceiling_db = -1.0;
            },
        ),
        mode(
            "podcast",
            "Podcast",
            PresetCategory::Podcast,
            "Voz à frente, graves reduzidos e nível consistente.",
            |p| {
                p.eq_db = [-4.0, -3.0, -2.0, -1.0, 0.0, 1.0, 2.5, 2.0, 1.0, 0.0];
                p.compressor = CompressorParams {
                    enabled: true,
                    threshold_db: -24.0,
                    ratio: 3.0,
                    attack_ms: 8.0,
                    release_ms: 120.0,
                    knee_db: 6.0,
                    makeup_db: 4.0,
                    ..Default::default()
                };
                p.loudness = LoudnessParams {
                    enabled: true,
                    intensity: 0.7,
                };
            },
        ),
        mode(
            "vocal",
            "Vocal",
            PresetCategory::Vocal,
            "Presença e clareza para voz, mantendo os extremos suaves.",
            |p| {
                p.eq_db = [-2.0, -1.5, -0.5, 0.0, 0.8, 1.5, 2.5, 2.0, 1.0, 0.5];
                p.compressor = CompressorParams {
                    enabled: true,
                    threshold_db: -20.0,
                    ratio: 2.0,
                    attack_ms: 12.0,
                    release_ms: 160.0,
                    knee_db: 3.0,
                    makeup_db: 1.5,
                    ..Default::default()
                };
                p.loudness = LoudnessParams {
                    enabled: true,
                    intensity: 0.5,
                };
            },
        ),
        mode(
            "clarity",
            "Clareza",
            PresetCategory::Clarity,
            "Detalhes e articulação em destaque, com médios-graves limpos e sem compressão pesada.",
            |p| {
                p.eq_db = [0.0, -0.5, -1.5, -1.0, 0.0, 0.5, 1.5, 2.5, 2.0, 1.0];
                p.loudness = LoudnessParams {
                    enabled: true,
                    intensity: 0.35,
                };
            },
        ),
        mode(
            "immersive",
            "Imersivo",
            PresetCategory::Immersive,
            "Palco amplo com reflexões e ambiência para filmes, séries e trilhas.",
            |p| {
                p.eq_db = [1.5, 1.2, 0.5, 0.0, -0.3, 0.0, 0.8, 1.2, 1.0, 0.5];
                p.bass = BassParams {
                    enabled: true,
                    intensity: 0.35,
                    cutoff_hz: 80.0,
                    harmonics: 0.25,
                };
                p.spatial = SpatialParams {
                    mode: SpatialMode::Surround,
                    width: 1.25,
                    room: 0.65,
                };
                p.loudness = LoudnessParams {
                    enabled: true,
                    intensity: 0.45,
                };
            },
        ),
        mode(
            "flat",
            "Flat",
            PresetCategory::Flat,
            "Sem equalização ou realce; apenas proteção do limiter.",
            |_p| {},
        ),
        mode(
            "bass",
            "Bass",
            PresetCategory::Bass,
            "Subwoofer virtual e graves amplos para fones de qualquer tamanho.",
            |p| {
                p.eq_db = [1.5, 1.5, 1.0, 0.0, -1.0, 0.0, 0.5, 0.5, 0.0, 0.0];
                p.bass = BassParams {
                    enabled: true,
                    intensity: 0.4,
                    cutoff_hz: 70.0,
                    harmonics: 0.2,
                };
                p.loudness = LoudnessParams {
                    enabled: true,
                    intensity: 0.2,
                };
            },
        ),
        mode(
            "night",
            "Noturno",
            PresetCategory::Night,
            "Faixa dinâmica comprimida e compensação de loudness forte.",
            |p| {
                p.eq_db = [-1.0, -0.5, 0.0, 0.5, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0];
                p.compressor = CompressorParams {
                    enabled: true,
                    threshold_db: -30.0,
                    ratio: 5.0,
                    attack_ms: 5.0,
                    release_ms: 250.0,
                    knee_db: 6.0,
                    makeup_db: 6.0,
                    ..Default::default()
                };
                p.loudness = LoudnessParams {
                    enabled: true,
                    intensity: 0.9,
                };
                p.limiter.ceiling_db = -1.0;
            },
        ),
        mode(
            "protection",
            "Proteção",
            PresetCategory::Protection,
            "Graves contidos, dinâmica controlada, redução de sons altos e teto de pico baixo para sessões longas.",
            |p| {
                p.preamp_db = -2.0;
                p.compressor = CompressorParams {
                    enabled: true,
                    threshold_db: -24.0,
                    ratio: 3.0,
                    attack_ms: 5.0,
                    release_ms: 250.0,
                    knee_db: 6.0,
                    makeup_db: 0.0,
                    // This mode exists to control the whole spectrum evenly;
                    // the bass-insensitive sidechain would leave the sub-bass
                    // louder than the mids.
                    sidechain_hpf: false,
                };
                p.loud_sound_reduction.enabled = true;
                p.limiter.ceiling_db = -3.0;
            },
        ),
    ]
}

fn headphone_presets() -> Vec<Preset> {
    vec![
        headphone(
            "hp-jbl-quantum-100",
            "JBL",
            "Quantum 100",
            "AutoEQ · Rtings HMS II.3",
            -6.6,
            vec![
                ls(105.0, 1.7, 0.70),
                pk(192.0, -7.3, 0.73),
                pk(1660.0, 3.7, 0.18),
                pk(50.0, 3.0, 0.91),
                pk(7901.0, -8.6, 1.68),
                hs(10000.0, -3.5, 0.70),
                pk(4052.0, 5.0, 3.59),
                pk(2856.0, -4.3, 4.23),
                pk(552.0, 3.3, 3.18),
                pk(779.0, -3.3, 3.14),
            ],
        ),
        headphone(
            "hp-jbl-quantum-400",
            "JBL",
            "Quantum 400",
            "AutoEQ · Rtings HMS II.3",
            -6.5,
            vec![
                ls(105.0, 18.0, 0.70),
                pk(8615.0, -6.4, 0.34),
                pk(47.0, -20.0, 0.30),
                pk(4612.0, 11.1, 1.76),
                pk(291.0, 6.0, 0.76),
                hs(10000.0, 0.3, 0.70),
                pk(1580.0, 1.1, 1.42),
                pk(2869.0, -2.7, 3.79),
                pk(3833.0, 1.5, 4.28),
                pk(766.0, -0.9, 2.86),
            ],
        ),
        headphone(
            "hp-jbl-quantum-800",
            "JBL",
            "Quantum 800",
            "AutoEQ · Rtings HMS II.3",
            -6.2,
            vec![
                ls(105.0, 9.2, 0.70),
                pk(10000.0, -7.5, 0.40),
                pk(59.0, -12.1, 0.30),
                pk(4339.0, 10.0, 1.93),
                pk(375.0, 3.8, 0.37),
                hs(10000.0, 3.6, 0.70),
                pk(9190.0, -2.0, 3.27),
                pk(1529.0, 1.6, 2.86),
                pk(2660.0, -1.4, 3.70),
                pk(690.0, -1.1, 3.19),
            ],
        ),
        headphone(
            "hp-jbl-quantum-one",
            "JBL",
            "Quantum ONE",
            "AutoEQ · Rtings HMS II.3",
            -3.0,
            vec![
                ls(105.0, 2.1, 0.70),
                pk(6146.0, -2.8, 3.65),
                pk(1338.0, 3.7, 0.76),
                pk(2482.0, -7.0, 2.23),
                pk(8616.0, -4.0, 2.54),
                hs(10000.0, -1.7, 0.70),
                pk(46.0, -1.5, 1.95),
                pk(152.0, 1.3, 2.20),
                pk(4404.0, 2.9, 6.00),
                pk(371.0, 0.9, 2.51),
            ],
        ),
        headphone(
            "hp-jbl-tune770nc",
            "JBL",
            "Tune 770NC",
            "AutoEQ · Rtings B&K 5128",
            -2.5,
            vec![
                ls(105.0, -3.8, 0.70),
                pk(8989.0, -4.6, 1.76),
                pk(1646.0, 2.9, 1.03),
                pk(111.0, 2.9, 1.24),
                pk(2845.0, -2.6, 2.14),
                hs(10000.0, -1.8, 0.70),
                pk(514.0, -2.6, 2.09),
                pk(5044.0, 3.0, 4.22),
                pk(806.0, 1.4, 2.84),
                pk(6424.0, -2.3, 6.00),
            ],
        ),
        headphone(
            "hp-sony-wh1000xm4",
            "Sony",
            "WH-1000XM4",
            "AutoEQ · oratory1990",
            -6.1,
            vec![
                ls(105.0, -4.2, 0.70),
                pk(143.0, -5.2, 1.10),
                pk(2289.0, 6.1, 1.57),
                pk(56.0, 1.2, 1.19),
                pk(5144.0, -3.2, 6.00),
                hs(10000.0, -1.0, 0.70),
                pk(407.0, 1.7, 3.14),
                pk(6715.0, 3.0, 5.99),
                pk(1007.0, 1.0, 3.41),
                pk(576.0, -1.2, 3.55),
            ],
        ),
        headphone(
            "hp-sony-wh1000xm5",
            "Sony",
            "WH-1000XM5",
            "AutoEQ · oratory1990",
            -6.2,
            vec![
                ls(105.0, -3.2, 0.70),
                pk(2448.0, 6.9, 2.46),
                pk(173.0, -5.6, 0.96),
                pk(3028.0, -5.4, 2.03),
                pk(1327.0, 3.3, 0.58),
                hs(10000.0, 4.9, 0.70),
                pk(6110.0, -2.3, 5.81),
                pk(875.0, -1.2, 4.07),
                pk(1197.0, 1.0, 3.28),
                pk(63.0, 0.4, 2.13),
            ],
        ),
        headphone(
            "hp-bose-qc45",
            "Bose",
            "QuietComfort 45",
            "AutoEQ · oratory1990",
            -2.2,
            vec![
                ls(105.0, -1.4, 0.70),
                pk(161.0, -2.2, 1.15),
                pk(5490.0, -7.7, 2.06),
                pk(4242.0, 3.5, 0.24),
                pk(2328.0, -5.2, 2.15),
                hs(10000.0, -7.0, 0.70),
                pk(8352.0, 2.9, 1.93),
                pk(390.0, 0.5, 2.32),
                pk(6508.0, -1.0, 5.04),
                pk(70.0, -0.1, 1.97),
            ],
        ),
        headphone(
            "hp-bose-qc-ultra",
            "Bose",
            "QuietComfort Ultra Headphones",
            "AutoEQ · Rtings B&K 5128",
            -2.8,
            vec![
                ls(105.0, -3.0, 0.70),
                pk(5032.0, 2.8, 3.69),
                pk(156.0, 1.5, 1.61),
                pk(94.0, -2.4, 2.32),
                pk(2373.0, -2.8, 5.58),
                hs(10000.0, 2.7, 0.70),
                pk(1633.0, 2.6, 3.51),
                pk(491.0, -1.1, 2.50),
                pk(2194.0, -1.0, 3.76),
                pk(36.0, -0.5, 2.83),
            ],
        ),
        headphone(
            "hp-sennheiser-hd600",
            "Sennheiser",
            "HD 600",
            "AutoEQ · oratory1990",
            -6.3,
            vec![
                ls(105.0, 6.5, 0.70),
                pk(125.0, -2.7, 0.55),
                pk(8445.0, 3.3, 1.61),
                pk(522.0, 0.7, 1.02),
                pk(1298.0, -1.2, 2.14),
                hs(10000.0, -3.1, 0.70),
                pk(3158.0, -1.8, 3.67),
                pk(2166.0, 0.9, 3.32),
                pk(6639.0, 2.2, 5.82),
                pk(5433.0, -1.2, 5.70),
            ],
        ),
        headphone(
            "hp-sennheiser-momentum4",
            "Sennheiser",
            "Momentum 4 Wireless",
            "AutoEQ · oratory1990",
            -6.4,
            vec![
                ls(105.0, -7.4, 0.70),
                pk(195.0, -3.6, 1.22),
                pk(2393.0, 6.1, 1.81),
                pk(1726.0, 1.6, 1.71),
                pk(2893.0, -3.2, 5.94),
                hs(10000.0, 0.5, 0.70),
                pk(63.0, 1.7, 2.86),
                pk(652.0, 1.2, 2.25),
                pk(952.0, -1.0, 3.69),
                pk(316.0, -0.9, 3.46),
            ],
        ),
        headphone(
            "hp-apple-airpods-max",
            "Apple",
            "AirPods Max",
            "AutoEQ · oratory1990",
            -4.7,
            vec![
                ls(105.0, -3.0, 0.70),
                pk(7273.0, 3.6, 2.41),
                pk(218.0, -2.9, 1.41),
                pk(1031.0, -3.2, 0.99),
                pk(3185.0, 3.1, 0.56),
                hs(10000.0, -5.5, 0.70),
                pk(9508.0, 2.7, 2.20),
                pk(66.0, 0.6, 1.65),
                pk(4045.0, 2.1, 5.82),
                pk(4834.0, -1.8, 6.00),
            ],
        ),
        headphone(
            "hp-apple-airpods-pro2",
            "Apple",
            "AirPods Pro 2",
            "AutoEQ · Rtings B&K 5128",
            -3.1,
            vec![
                ls(105.0, -1.5, 0.70),
                pk(377.0, -2.5, 0.74),
                pk(3442.0, 3.0, 0.67),
                pk(6303.0, -3.4, 4.47),
                pk(1339.0, 0.5, 2.65),
                hs(10000.0, 3.1, 0.70),
                pk(82.0, 1.0, 1.92),
                pk(170.0, -0.6, 1.86),
                pk(8133.0, -1.1, 2.86),
                pk(311.0, 0.4, 3.57),
            ],
        ),
        headphone(
            "hp-samsung-buds2-pro",
            "Samsung",
            "Galaxy Buds2 Pro",
            "AutoEQ · oratory1990",
            -2.3,
            vec![
                ls(105.0, -2.3, 0.70),
                pk(7526.0, 2.2, 2.78),
                pk(266.0, -0.8, 1.35),
                pk(3426.0, -2.3, 3.70),
                pk(64.0, 2.2, 1.18),
                hs(10000.0, -3.8, 0.70),
                pk(8817.0, 1.6, 2.28),
                pk(2283.0, 1.2, 3.75),
                pk(4955.0, -1.2, 5.11),
                pk(977.0, 1.0, 5.18),
            ],
        ),
        headphone(
            "hp-ath-m50x",
            "Audio-Technica",
            "ATH-M50x",
            "AutoEQ · oratory1990",
            -3.1,
            vec![
                ls(105.0, 0.6, 0.70),
                pk(156.0, -5.2, 0.73),
                pk(326.0, 5.3, 1.59),
                pk(7077.0, 2.8, 2.22),
                pk(3483.0, 2.1, 5.82),
                hs(10000.0, -4.1, 0.70),
                pk(45.0, -1.1, 1.90),
                pk(66.0, 1.4, 3.59),
                pk(787.0, -0.5, 1.79),
                pk(1640.0, 0.9, 3.41),
            ],
        ),
        headphone(
            "hp-beyerdynamic-dt770",
            "Beyerdynamic",
            "DT 770 Pro",
            "AutoEQ · oratory1990",
            -5.1,
            vec![
                ls(105.0, -3.0, 0.70),
                pk(6430.0, -4.2, 0.97),
                pk(3684.0, 6.8, 2.88),
                pk(213.0, 3.5, 2.46),
                pk(139.0, -3.9, 4.92),
                hs(10000.0, -5.4, 0.70),
                pk(93.0, 3.5, 3.00),
                pk(9561.0, 2.3, 1.86),
                pk(45.0, -1.3, 1.74),
                pk(119.0, -2.1, 5.83),
            ],
        ),
        headphone(
            "hp-hyperx-cloud-2",
            "HyperX",
            "Cloud II",
            "AutoEQ · oratory1990",
            -6.4,
            vec![
                ls(105.0, 4.1, 0.70),
                pk(170.0, -7.0, 1.06),
                pk(271.0, 3.7, 0.58),
                pk(3828.0, 5.4, 3.60),
                pk(57.0, -4.8, 0.58),
                hs(10000.0, -7.0, 0.70),
                pk(8085.0, -3.0, 3.98),
                pk(2378.0, -1.9, 2.78),
                pk(3700.0, 1.3, 2.71),
                pk(1123.0, 1.1, 3.29),
            ],
        ),
        headphone(
            "hp-steelseries-arctis-nova-pro",
            "SteelSeries",
            "Arctis Nova Pro",
            "AutoEQ · Rtings B&K 5128",
            -6.7,
            vec![
                ls(105.0, -2.2, 0.70),
                pk(141.0, -5.7, 1.88),
                pk(8720.0, -7.0, 0.34),
                pk(4510.0, 11.9, 1.52),
                pk(359.0, 4.6, 1.63),
                hs(10000.0, 6.6, 0.70),
                pk(64.0, 4.0, 3.30),
                pk(9316.0, -2.5, 1.54),
                pk(92.0, -1.5, 2.55),
                pk(1981.0, 1.0, 5.16),
            ],
        ),
        headphone(
            "hp-logitech-g-pro-x",
            "Logitech",
            "G Pro X",
            "AutoEQ · Rtings HMS II.3",
            -5.9,
            vec![
                ls(105.0, 1.3, 0.70),
                pk(177.0, -5.1, 0.70),
                pk(3727.0, 5.9, 2.52),
                pk(40.0, 2.9, 1.19),
                pk(381.0, 3.2, 1.10),
                hs(10000.0, 4.2, 0.70),
                pk(1060.0, 2.4, 3.35),
                pk(1557.0, -2.1, 2.55),
                pk(8739.0, -1.9, 2.81),
                pk(5189.0, 1.9, 6.00),
            ],
        ),
        headphone(
            "hp-razer-blackshark-v2",
            "Razer",
            "BlackShark V2",
            "AutoEQ · Rtings HMS II.3",
            -5.9,
            vec![
                ls(105.0, 8.8, 0.70),
                pk(1186.0, 3.5, 0.18),
                pk(72.0, -5.1, 0.27),
                pk(8531.0, -4.4, 1.38),
                pk(2984.0, -5.8, 1.61),
                hs(10000.0, -2.0, 0.70),
                pk(4457.0, 2.8, 6.00),
                pk(104.0, 0.7, 2.16),
                pk(3714.0, -1.9, 6.00),
                pk(62.0, -0.7, 2.34),
            ],
        ),
    ]
}

pub fn builtin_presets() -> Vec<Preset> {
    let mut presets = mode_presets();
    presets.extend(headphone_presets());
    presets
}

pub fn find_builtin(id: &str) -> Option<Preset> {
    builtin_presets().into_iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::Processor;
    use std::collections::HashSet;
    use std::f64::consts::PI;

    const SR: f32 = 48_000.0;

    fn gain_db(params: &ProcessingParams, freq: f32) -> f64 {
        let mut chain = Processor::new(SR);
        chain.set_params(params);
        let amp = 0.1f64;
        let frames = SR as usize;
        let skip = frames / 2;
        let mut buf = [0.0f32; 1024];
        let mut sum = 0.0f64;
        let mut count = 0usize;
        let mut done = 0usize;
        while done < frames {
            let n = (frames - done).min(512);
            for i in 0..n {
                let t = (done + i) as f64 / SR as f64;
                let v = (amp * (2.0 * PI * freq as f64 * t).sin()) as f32;
                buf[i * 2] = v;
                buf[i * 2 + 1] = v;
            }
            chain.process_interleaved(&mut buf[..n * 2]);
            if done >= skip {
                for frame in buf[..n * 2].chunks_exact(2) {
                    sum += frame[0] as f64 * frame[0] as f64;
                    count += 1;
                }
            }
            done += n;
        }
        let rms = (sum / count as f64).sqrt();
        20.0 * (rms / (amp / 2f64.sqrt())).log10()
    }

    #[test]
    fn ids_are_unique_and_filters_within_budget() {
        let presets = builtin_presets();
        let ids: HashSet<_> = presets.iter().map(|p| &p.id).collect();
        assert_eq!(ids.len(), presets.len());
        assert!(presets.len() >= 30);
        for preset in &presets {
            assert!(preset.params.auto_eq.filters.len() <= 16);
            assert!(preset.params.eq_db.iter().all(|g| g.is_finite()));
        }
    }

    #[test]
    fn modes_exist() {
        for id in [
            "music",
            "cinema",
            "gaming",
            "competitive",
            "podcast",
            "vocal",
            "clarity",
            "immersive",
            "flat",
            "bass",
            "night",
            "protection",
        ] {
            assert!(find_builtin(id).is_some(), "{id}");
        }
    }

    #[test]
    fn headroom_estimate_tracks_the_eq_cascade() {
        let mut params = ProcessingParams::default();
        params.eq_db[5] = 6.0; // 1 kHz
        let boost = max_static_boost_db(&params);
        assert!((boost - 6.0).abs() < 0.2, "estimated {boost:.2} dB");

        let mut bass = ProcessingParams::default();
        bass.bass.enabled = true;
        bass.bass.intensity = 0.4;
        bass.bass.cutoff_hz = 70.0;
        let boost = max_static_boost_db(&bass);
        assert!((boost - 6.0).abs() < 0.3, "bass shelf {boost:.2} dB");
    }

    #[test]
    fn content_modes_keep_their_boost_inside_the_allowance() {
        for preset in builtin_presets()
            .iter()
            .filter(|preset| preset.category != PresetCategory::Headphone)
        {
            let residual = max_static_boost_db(&preset.params) + preset.params.preamp_db;
            assert!(
                residual <= PRESET_HEADROOM_ALLOWANCE_DB + 0.2,
                "{} leaves {residual:+.1} dB of static boost",
                preset.id
            );
            assert!(
                preset.params.preamp_db >= -12.0,
                "{} attenuates far beyond its static boost",
                preset.id
            );
        }
    }

    #[test]
    fn headphone_profiles_are_measured_corrections() {
        let measured = [
            "hp-jbl-quantum-100",
            "hp-jbl-quantum-400",
            "hp-jbl-quantum-800",
            "hp-jbl-quantum-one",
            "hp-jbl-tune770nc",
            "hp-sony-wh1000xm4",
            "hp-sony-wh1000xm5",
            "hp-bose-qc45",
            "hp-bose-qc-ultra",
            "hp-sennheiser-hd600",
            "hp-sennheiser-momentum4",
            "hp-apple-airpods-max",
            "hp-apple-airpods-pro2",
            "hp-samsung-buds2-pro",
            "hp-ath-m50x",
            "hp-beyerdynamic-dt770",
            "hp-hyperx-cloud-2",
            "hp-steelseries-arctis-nova-pro",
            "hp-logitech-g-pro-x",
            "hp-razer-blackshark-v2",
        ];
        for id in measured {
            let preset = find_builtin(id).unwrap_or_else(|| panic!("{id} is missing"));
            assert_eq!(preset.category, PresetCategory::Headphone);
            let auto_eq = &preset.params.auto_eq;
            assert!(auto_eq.enabled, "{id}");
            assert_eq!(auto_eq.filters.len(), 10, "{id}");
            assert!((-24.0..=0.0).contains(&auto_eq.preamp_db), "{id}");
            assert!(preset.params.eq_db.iter().all(|gain| *gain == 0.0), "{id}");
            assert!(preset.brand.is_some() && preset.model.is_some(), "{id}");
            assert!(preset.description.contains("AutoEQ"), "{id}");
        }
    }

    // Guards against the bass preset stacking EQ, shelf and loudness into a
    // low-end tilt that is physically uncomfortable on closed-back headphones.
    #[test]
    fn bass_preset_keeps_low_frequency_gain_bounded() {
        let mut params = find_builtin("bass").unwrap().params;
        params.volume = 0.19;
        params.bypass = false;
        let reference = gain_db(&params, 1_000.0);
        for freq in [20.0, 31.0, 40.0, 62.0, 125.0] {
            let tilt = gain_db(&params, freq) - reference;
            assert!(
                tilt < 15.0,
                "bass preset boosts {freq} Hz by {tilt:+.1} dB over 1 kHz"
            );
        }
    }

    #[test]
    fn protection_preset_is_flat_and_peak_limited() {
        let mut params = find_builtin("protection").unwrap().params;
        params.volume = 0.19;
        params.bypass = false;
        assert!(!params.bass.enabled);
        assert!(!params.loudness.enabled);
        assert!(params.compressor.enabled);
        assert!(params.loud_sound_reduction.enabled);
        assert!(params.preamp_db <= -2.0);
        assert!(params.limiter.enabled);
        assert!(params.limiter.ceiling_db <= -3.0);

        let reference = gain_db(&params, 1_000.0);
        for freq in [31.0, 62.0, 125.0] {
            let tilt = gain_db(&params, freq) - reference;
            assert!(
                tilt.abs() < 1.0,
                "protection preset tilts {freq} Hz by {tilt:+.1} dB"
            );
        }
    }
}
