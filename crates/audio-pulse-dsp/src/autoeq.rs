//! Parse AutoEQ `ParametricEQ.txt` files.
//!
//! AutoEQ publishes correction files in this format:
//! ```text
//! Preamp: -6.2 dB
//! Filter 1: ON PK Fc 105 Hz Gain 5.5 dB Q 0.70
//! Filter 2: ON LS Fc 80 Hz Gain -2.0 dB Q 0.70
//! ```
//! The parser accepts PK/LS/HS (and LSC/HSC variants), ignores disabled
//! filters and comments, and clamps the result to the engine's filter budget.

use crate::params::{AutoEqFilter, FilterKind, MAX_AUTOEQ_FILTERS};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedAutoEq {
    pub preamp_db: f32,
    pub filters: Vec<AutoEqFilter>,
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

pub fn parse_autoeq_text(text: &str) -> Result<ParsedAutoEq, String> {
    let mut parsed = ParsedAutoEq::default();
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
                parsed.preamp_db = db;
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
        let kind = tokens.iter().find_map(|token| {
            let t = token.trim_matches(':').to_ascii_uppercase();
            match t.as_str() {
                "PK" => Some(FilterKind::Peaking),
                "LS" | "LSC" => Some(FilterKind::Lowshelf),
                "HS" | "HSC" => Some(FilterKind::Highshelf),
                _ => None,
            }
        });
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
    Ok(parsed)
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
}