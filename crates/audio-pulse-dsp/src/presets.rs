//! Built-in presets: mix modes and headphone reference corrections.
//!
//! The headphone corrections are Audio Pulse reference curves, not
//! laboratory measurements. Every profile uses the same parametric AutoEQ
//! filter model, so measured profiles exported as `ParametricEQ.txt` can be
//! imported at runtime and behave identically.

use serde::{Deserialize, Serialize};

use crate::params::{
    AutoEqFilter, AutoEqParams, BassParams, CompressorParams, CrossfeedParams, FilterKind,
    LimiterParams, LoudnessParams, ProcessingParams, SpatialMode, SpatialParams,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PresetCategory {
    Music,
    Cinema,
    Gaming,
    Podcast,
    Vocal,
    Flat,
    Bass,
    Night,
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
    preamp_db: f32,
    filters: Vec<AutoEqFilter>,
) -> Preset {
    let params = ProcessingParams {
        auto_eq: AutoEqParams {
            enabled: true,
            preamp_db,
            filters,
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
        description: "Curva de referência Audio Pulse. Importe um perfil AutoEQ medido para ajuste específico.".into(),
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
                };
                p.limiter.ceiling_db = -0.5;
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
                };
                p.loudness = LoudnessParams {
                    enabled: true,
                    intensity: 0.5,
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
                p.eq_db = [4.0, 3.5, 2.0, 0.0, -1.0, 0.0, 0.5, 0.5, 0.0, 0.0];
                p.bass = BassParams {
                    enabled: true,
                    intensity: 0.85,
                    cutoff_hz: 70.0,
                    harmonics: 0.35,
                };
                p.loudness = LoudnessParams {
                    enabled: true,
                    intensity: 0.4,
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
                };
                p.loudness = LoudnessParams {
                    enabled: true,
                    intensity: 0.9,
                };
                p.limiter.ceiling_db = -1.0;
            },
        ),
    ]
}

fn headphone_presets() -> Vec<Preset> {
    vec![
        headphone(
            "hp-sony-wh1000xm4",
            "Sony",
            "WH-1000XM4",
            -6.0,
            vec![
                ls(105.0, -2.5, 0.70),
                pk(220.0, 1.5, 1.00),
                pk(1_100.0, 0.8, 1.20),
                pk(2_700.0, 4.0, 1.10),
                pk(4_300.0, -2.0, 2.00),
                pk(6_200.0, -3.0, 1.50),
                hs(9_000.0, 2.0, 0.70),
            ],
        ),
        headphone(
            "hp-sony-wh1000xm5",
            "Sony",
            "WH-1000XM5",
            -5.0,
            vec![
                ls(100.0, -3.0, 0.70),
                pk(250.0, 1.0, 1.00),
                pk(3_000.0, 3.5, 1.10),
                pk(6_000.0, -2.5, 1.80),
                hs(10_000.0, 1.5, 0.70),
            ],
        ),
        headphone(
            "hp-bose-qc45",
            "Bose",
            "QuietComfort 45",
            -4.0,
            vec![
                ls(120.0, -1.5, 0.70),
                pk(350.0, 1.0, 1.00),
                pk(2_300.0, 3.0, 1.20),
                pk(5_000.0, -1.5, 1.50),
                hs(9_000.0, 1.0, 0.70),
            ],
        ),
        headphone(
            "hp-bose-qc-ultra",
            "Bose",
            "QuietComfort Ultra",
            -5.0,
            vec![
                ls(90.0, -2.0, 0.70),
                pk(400.0, 1.5, 1.00),
                pk(2_800.0, 2.5, 1.20),
                pk(5_500.0, -2.0, 1.50),
                hs(10_000.0, 1.5, 0.70),
            ],
        ),
        headphone(
            "hp-sennheiser-hd600",
            "Sennheiser",
            "HD 600",
            -2.0,
            vec![
                ls(60.0, 2.0, 0.70),
                pk(3_500.0, -1.5, 1.50),
                pk(5_500.0, 1.0, 2.00),
                hs(10_000.0, -1.0, 0.70),
            ],
        ),
        headphone(
            "hp-sennheiser-momentum4",
            "Sennheiser",
            "Momentum 4",
            -3.0,
            vec![
                ls(110.0, 1.5, 0.70),
                pk(900.0, -1.0, 1.20),
                pk(2_600.0, 2.0, 1.20),
                pk(6_000.0, -1.5, 1.50),
                hs(9_500.0, 1.0, 0.70),
            ],
        ),
        headphone(
            "hp-apple-airpods-max",
            "Apple",
            "AirPods Max",
            -2.5,
            vec![
                ls(90.0, 1.0, 0.70),
                pk(400.0, -0.5, 1.00),
                pk(2_500.0, 1.5, 1.20),
                pk(5_500.0, -1.0, 1.50),
                hs(9_000.0, 0.5, 0.70),
            ],
        ),
        headphone(
            "hp-apple-airpods-pro2",
            "Apple",
            "AirPods Pro (2ª geração)",
            -3.0,
            vec![
                ls(100.0, -1.0, 0.70),
                pk(200.0, 1.0, 1.00),
                pk(3_000.0, 2.0, 1.20),
                pk(6_000.0, -1.0, 1.50),
                hs(9_000.0, 0.5, 0.70),
            ],
        ),
        headphone(
            "hp-jbl-tune770nc",
            "JBL",
            "Tune 770NC",
            -3.0,
            vec![
                ls(100.0, 2.0, 0.70),
                pk(500.0, -1.0, 1.00),
                pk(2_500.0, 1.5, 1.20),
                hs(8_000.0, 1.5, 0.70),
            ],
        ),
        headphone(
            "hp-samsung-buds2-pro",
            "Samsung",
            "Galaxy Buds2 Pro",
            -4.0,
            vec![
                ls(90.0, -2.5, 0.70),
                pk(300.0, 1.0, 1.00),
                pk(2_800.0, 2.0, 1.20),
                pk(5_500.0, -1.5, 1.50),
                hs(10_000.0, 1.0, 0.70),
            ],
        ),
        headphone(
            "hp-ath-m50x",
            "Audio-Technica",
            "ATH-M50x",
            -3.5,
            vec![
                ls(100.0, -2.0, 0.70),
                pk(300.0, 2.0, 1.00),
                pk(4_000.0, -3.0, 2.00),
                pk(9_000.0, 2.0, 1.20),
            ],
        ),
        headphone(
            "hp-beyerdynamic-dt770",
            "Beyerdynamic",
            "DT 770 Pro",
            -4.0,
            vec![
                ls(80.0, 2.0, 0.70),
                pk(250.0, -1.0, 1.00),
                pk(6_000.0, -3.0, 2.00),
                pk(10_000.0, -4.0, 1.20),
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
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique_and_filters_within_budget() {
        let presets = builtin_presets();
        let ids: HashSet<_> = presets.iter().map(|p| &p.id).collect();
        assert_eq!(ids.len(), presets.len());
        assert!(presets.len() >= 15);
        for preset in &presets {
            assert!(preset.params.auto_eq.filters.len() <= 16);
            assert!(preset.params.eq_db.iter().all(|g| g.is_finite()));
        }
    }

    #[test]
    fn modes_exist() {
        for id in ["music", "cinema", "gaming", "podcast", "vocal", "flat"] {
            assert!(find_builtin(id).is_some(), "{id}");
        }
    }
}