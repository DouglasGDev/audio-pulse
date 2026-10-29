//! Parse AutoEQ and OPRA parametric correction profiles.
//!
//! Three public file shapes are accepted:
//!
//! * AutoEQ `ParametricEQ.txt`:
//! ```text
//! Preamp: -6.2 dB
//! Filter 1: ON PK Fc 105 Hz Gain 5.5 dB Q 0.70
//! Filter 2: ON LS Fc 80 Hz Gain -2.0 dB Q 0.70
//! ```
//! * AutoEQ `ParametricEQ.json`, as written by the `autoeq` Python package:
//!   `{"fs": 48000, "filters": [{"type": "PEAKING", "fc": 105, "q": 0.7,
//!   "gain": 5.5}]}`. It carries no preamp, so one is estimated from the
//!   cascade.
//! * OPRA equalizer records: `{"type": "parametric_eq", "parameters":
//!   {"gain_db": 0.0, "bands": [{"type": "peak_dip", "frequency": 105, "q":
//!   0.7, "gain_db": 5.5}]}}`, including a whole `database_v1.jsonl` line
//!   wrapped in `{"type": "eq", "id": ..., "data": ...}`.
//!
//! The parser accepts peaking (PK) and low/high shelves in every spelling the
//! two projects use, skips unsupported kinds (pass, notch, all-pass) and
//! disabled filters, ignores comments, and clamps the result to the engine's
//! filter budget.

use serde_json::{Map, Value};

use crate::biquad::Biquad;
use crate::params::{AutoEqFilter, FilterKind, MAX_AUTOEQ_FILTERS};

/// Sample rate used when estimating how much static boost a cascade adds. The
/// estimate only sets a preamp; the engine still runs at the device rate.
const ESTIMATE_SAMPLE_RATE: f32 = 48_000.0;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedAutoEq {
    pub preamp_db: f32,
    pub filters: Vec<AutoEqFilter>,
    /// Profile or product name when the file carries one (OPRA records do).
    pub name: Option<String>,
}

fn token_number(tokens: &[&str], key: &str) -> Option<f32> {
    let pos = tokens.iter().position(|t| t.eq_ignore_ascii_case(key))?;
    tokens.get(pos + 1)?.parse::<f32>().ok()
}

fn parse_preamp(line: &str) -> Option<f32> {
    let rest = line.split_once(':')?.1.trim();
    let value = rest.split_whitespace().next()?;
    value.parse::<f32>().ok()
}

fn kind_of(text: &str) -> Option<FilterKind> {
    let normalized = text.trim().to_ascii_uppercase().replace([' ', '-'], "_");
    match normalized.as_str() {
        "PK" | "PEQ" | "PEAK" | "PEAKING" | "PEAK_DIP" => Some(FilterKind::Peaking),
        "LS" | "LSC" | "LOW_SHELF" | "LOWSHELF" | "LOW_SHELVING" | "LOW_SHELF_FILTER" => {
            Some(FilterKind::Lowshelf)
        }
        "HS" | "HSC" | "HIGH_SHELF" | "HIGHSHELF" | "HIGH_SHELVING" | "HIGH_SHELF_FILTER" => {
            Some(FilterKind::Highshelf)
        }
        _ => None,
    }
}

fn json_number(value: Option<&Value>) -> Option<f32> {
    value
        .and_then(Value::as_f64)
        .map(|number| number as f32)
        .filter(|number| number.is_finite())
}

fn number_key(object: &Map<String, Value>, keys: &[&str]) -> Option<f32> {
    keys.iter().find_map(|key| json_number(object.get(*key)))
}

fn name_key(object: &Map<String, Value>) -> Option<String> {
    ["name", "product", "product_id", "model", "id"]
        .iter()
        .filter_map(|key| object.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .find(|text| !text.is_empty())
        .map(str::to_string)
}

fn filter_from_json(value: &Value) -> Option<AutoEqFilter> {
    let object = value.as_object()?;
    let kind = object
        .get("type")
        .and_then(Value::as_str)
        .and_then(kind_of)?;
    let freq = number_key(object, &["frequency", "fc", "freq", "f"])?;
    let gain_db = number_key(object, &["gain_db", "gain", "gainDb"]).unwrap_or(0.0);
    let q = number_key(object, &["q", "Q"]).unwrap_or(0.707);
    Some(AutoEqFilter {
        kind,
        freq,
        gain_db,
        q,
    })
}

/// Largest static boost the filter cascade adds, in dB, sampled on a 1.03× log
/// grid and summing each filter's magnitude there. Used to derive a preamp for
/// profiles that do not declare one, so an imported boost cannot drive the
/// limiter permanently.
pub fn estimated_preamp_db(filters: &[AutoEqFilter]) -> f32 {
    let biquads: Vec<Biquad> = filters
        .iter()
        .map(|filter| match filter.kind {
            FilterKind::Peaking => {
                Biquad::peaking(ESTIMATE_SAMPLE_RATE, filter.freq, filter.gain_db, filter.q)
            }
            FilterKind::Lowshelf => {
                Biquad::low_shelf(ESTIMATE_SAMPLE_RATE, filter.freq, filter.gain_db, filter.q)
            }
            FilterKind::Highshelf => {
                Biquad::high_shelf(ESTIMATE_SAMPLE_RATE, filter.freq, filter.gain_db, filter.q)
            }
        })
        .collect();
    let mut peak = 0.0f32;
    let mut freq = 20.0f32;
    while freq < 20_000.0 {
        let gain: f32 = biquads
            .iter()
            .map(|biquad| biquad.magnitude_db(freq, ESTIMATE_SAMPLE_RATE))
            .sum();
        peak = peak.max(gain);
        freq *= 1.03;
    }
    let preamp = -peak.clamp(0.0, 30.0);
    (preamp * 10.0).round() / 10.0
}

pub fn parse_autoeq_text(text: &str) -> Result<ParsedAutoEq, String> {
    let mut parsed = ParsedAutoEq::default();
    let mut declared_preamp: Option<f32> = None;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty()
            || line.starts_with('#')
            || line.starts_with("//")
            || line.starts_with(';')
        {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.starts_with("preamp") {
            if let Some(db) = parse_preamp(line) {
                declared_preamp = Some(db);
            }
            continue;
        }
        if !lower.starts_with("filter") {
            continue;
        }
        if lower.contains(" off") {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        // Type follows the filter number, e.g. "Filter 1: ON PK ..."
        let kind = tokens
            .iter()
            .find_map(|token| kind_of(token.trim_matches(':')));
        let Some(kind) = kind else { continue };
        let Some(freq) = token_number(&tokens, "Fc") else {
            continue;
        };
        let Some(gain_db) = token_number(&tokens, "Gain") else {
            continue;
        };
        let q = token_number(&tokens, "Q").unwrap_or(0.707);
        parsed.filters.push(AutoEqFilter {
            kind,
            freq,
            gain_db,
            q,
        });
        if parsed.filters.len() >= MAX_AUTOEQ_FILTERS {
            break;
        }
    }
    if parsed.filters.is_empty() {
        return Err("Nenhum filtro paramétrico encontrado no arquivo.".into());
    }
    parsed.preamp_db = declared_preamp.unwrap_or_else(|| estimated_preamp_db(&parsed.filters));
    Ok(parsed)
}

fn collect_bands(
    bands: &[Value],
    declared_preamp: Option<f32>,
    name: Option<String>,
) -> Result<ParsedAutoEq, String> {
    let mut parsed = ParsedAutoEq {
        preamp_db: 0.0,
        filters: Vec::new(),
        name,
    };
    for band in bands {
        if let Some(filter) = filter_from_json(band) {
            parsed.filters.push(filter);
            if parsed.filters.len() >= MAX_AUTOEQ_FILTERS {
                break;
            }
        }
    }
    if parsed.filters.is_empty() {
        return Err("Nenhum filtro PK/LS/HS foi encontrado no arquivo.".into());
    }
    parsed.preamp_db = declared_preamp.unwrap_or_else(|| estimated_preamp_db(&parsed.filters));
    Ok(parsed)
}

/// Walks the handful of JSON shapes AutoEQ and OPRA publish.
fn collect_json(body: &Value, wrapper_name: Option<String>) -> Result<ParsedAutoEq, String> {
    let object = body
        .as_object()
        .ok_or_else(|| "O arquivo JSON não contém um perfil de equalização.".to_string())?;

    let parameters = object.get("parameters").and_then(Value::as_object);
    let (bands, declared_preamp, name) = match parameters {
        Some(parameters) => (
            parameters
                .get("bands")
                .or_else(|| parameters.get("filters")),
            number_key(
                parameters,
                &["preamp_db", "preamp", "preampDb", "gain_db", "gainDb"],
            ),
            wrapper_name.or_else(|| name_key(object)),
        ),
        None => (
            object.get("bands").or_else(|| object.get("filters")),
            number_key(
                object,
                &["preamp_db", "preamp", "preampDb", "gain_db", "gainDb"],
            ),
            wrapper_name.or_else(|| name_key(object)),
        ),
    };

    let bands = bands
        .and_then(Value::as_array)
        .ok_or_else(|| "O arquivo JSON não contém filtros paramétricos.".to_string())?;
    collect_bands(bands, declared_preamp, name)
}

pub fn parse_autoeq_json(text: &str) -> Result<ParsedAutoEq, String> {
    let value: Value = serde_json::from_str(text).map_err(|e| format!("JSON inválido: {e}"))?;
    if let Some(array) = value.as_array() {
        return collect_bands(array, None, None);
    }
    // OPRA ships one record per line: {"type":"eq","id":...,"data":{...}}.
    // Accepting that line means a user can paste a profile straight from the
    // database without editing it.
    if value.get("type").and_then(Value::as_str) == Some("eq") {
        if let Some(data) = value.get("data") {
            let id = value
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string);
            return collect_json(data, id);
        }
    }
    collect_json(&value, None)
}

/// Detects the format by content, not by extension: AutoEQ users rename files
/// freely and OPRA profiles are often saved as `.txt` snippets.
pub fn parse_autoeq(text: &str) -> Result<ParsedAutoEq, String> {
    let trimmed = text.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        parse_autoeq_json(trimmed)
    } else {
        parse_autoeq_text(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
# AutoEQ profile
Preamp: -6.2 dB
Filter 1: ON PK Fc 105 Hz Gain 5.5 dB Q 0.70
Filter 2: ON LS Fc 80 Hz Gain -2.0 dB Q 0.70
Filter 3: ON HS Fc 10000 Hz Gain 2.5 dB Q 0.70
Filter 4: OFF PK Fc 2000 Hz Gain -4.0 dB Q 1.20
"#;

    #[test]
    fn parses_preamp_and_enabled_filters() {
        let parsed = parse_autoeq_text(SAMPLE).unwrap();
        assert!((parsed.preamp_db + 6.2).abs() < 1e-5);
        assert_eq!(parsed.filters.len(), 3);
        assert_eq!(parsed.filters[1].kind, FilterKind::Lowshelf);
        assert!((parsed.filters[2].freq - 10_000.0).abs() < 0.1);
    }

    #[test]
    fn rejects_files_without_filters() {
        assert!(parse_autoeq_text("Preamp: -3 dB\n").is_err());
    }

    #[test]
    fn text_without_a_preamp_gets_an_estimate() {
        let text = "Filter 1: ON PK Fc 100 Hz Gain 6.0 dB Q 0.70\n";
        let parsed = parse_autoeq_text(text).unwrap();
        assert!(
            (parsed.preamp_db + 6.0).abs() < 0.15,
            "estimated {:.2} dB",
            parsed.preamp_db
        );
    }

    #[test]
    fn cuts_alone_do_not_raise_the_signal() {
        let filters = vec![AutoEqFilter {
            kind: FilterKind::Peaking,
            freq: 3_000.0,
            gain_db: -4.0,
            q: 1.2,
        }];
        assert_eq!(estimated_preamp_db(&filters), 0.0);
    }

    #[test]
    fn parses_autoeq_json_and_estimates_the_preamp() {
        let json = r#"{"fs": 48000, "filters": [
            {"type": "PEAKING", "fc": 100.0, "q": 0.7, "gain": 6.0},
            {"type": "HIGH_SHELF", "fc": 9000.0, "q": 0.7, "gain": -1.0}
        ]}"#;
        let parsed = parse_autoeq(json).unwrap();
        assert_eq!(parsed.filters.len(), 2);
        assert_eq!(parsed.filters[0].kind, FilterKind::Peaking);
        assert_eq!(parsed.filters[1].kind, FilterKind::Highshelf);
        assert!(
            parsed.preamp_db <= -5.4 && parsed.preamp_db >= -6.2,
            "estimated {:.2} dB",
            parsed.preamp_db
        );
    }

    #[test]
    fn parses_opra_bands_and_keeps_the_declared_gain() {
        let json = r#"{
            "type": "parametric_eq",
            "author": "oratory1990",
            "product_id": "sennheiser_hd650",
            "parameters": {
                "gain_db": -4.5,
                "bands": [
                    {"type": "peak_dip", "frequency": 105.0, "q": 0.7, "gain_db": 5.5},
                    {"type": "low_shelf", "frequency": 80.0, "q": 0.7, "gain_db": -2.0},
                    {"type": "high_shelf", "frequency": 10000.0, "q": 0.7, "gain_db": 2.5},
                    {"type": "low_pass", "frequency": 20000.0, "slope": 12.0}
                ]
            }
        }"#;
        let parsed = parse_autoeq(json).unwrap();
        assert_eq!(parsed.filters.len(), 3);
        assert_eq!(parsed.filters[0].kind, FilterKind::Peaking);
        assert!((parsed.filters[0].freq - 105.0).abs() < 1e-5);
        assert_eq!(parsed.filters[1].kind, FilterKind::Lowshelf);
        assert_eq!(parsed.filters[2].kind, FilterKind::Highshelf);
        assert!((parsed.preamp_db + 4.5).abs() < 1e-5);
        assert_eq!(parsed.name.as_deref(), Some("sennheiser_hd650"));
    }

    #[test]
    fn parses_a_whole_opra_database_line() {
        let line = r#"{"type":"eq","id":"sennheiser_hd650_oratory","data":{
            "type":"parametric_eq",
            "parameters":{"gain_db":-3.0,"bands":[
                {"type":"peak_dip","frequency":3000.0,"q":1.2,"gain_db":-2.0}
            ]}
        }}"#;
        let parsed = parse_autoeq(line).unwrap();
        assert_eq!(parsed.filters.len(), 1);
        assert_eq!(parsed.name.as_deref(), Some("sennheiser_hd650_oratory"));
    }

    #[test]
    fn json_without_supported_filters_is_rejected() {
        let json = r#"{"filters": [{"type": "low_pass", "fc": 20000.0, "q": 0.7}]}"#;
        assert!(parse_autoeq(json).is_err());
        assert!(parse_autoeq_json("{ not json").is_err());
    }

    #[test]
    fn json_is_truncated_to_the_engine_budget() {
        let bands: Vec<String> = (0..40)
            .map(|index| {
                format!(
                    r#"{{"type": "PEAKING", "fc": {}, "q": 1.0, "gain": 1.0}}"#,
                    100 + index * 100
                )
            })
            .collect();
        let json = format!(r#"{{"filters": [{}]}}"#, bands.join(","));
        let parsed = parse_autoeq_json(&json).unwrap();
        assert_eq!(parsed.filters.len(), MAX_AUTOEQ_FILTERS);
    }
}
