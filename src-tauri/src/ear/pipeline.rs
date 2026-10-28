//! Photo loading, preprocessing, acoustic mapping and the public analysis.

use base64::Engine;
use image::{imageops::FilterType, RgbImage};
use serde::Serialize;
use audio_pulse_dsp::{AutoEqFilter, FilterKind};
use std::path::Path;
use std::time::Instant;

use super::geometry::{self, CROP_SIZE};
use super::inference::{self, INPUT_SIZE};
use super::model;
use super::overlay;

const IMAGE_MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const IMAGE_STD: [f32; 3] = [0.229, 0.224, 0.225];
const PHOTO_MAX_SIDE: u32 = 1400;
const PHOTO_MAX_BYTES: u64 = 60 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EarPhoto {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EarMetrics {
    /// 0..1, how far the concha sits behind the rim.
    pub concha_depth: f32,
    /// Bowl area as a fraction of the guide.
    pub concha_size: f32,
    /// 0..1, outer-rim depth contrast.
    pub helix_prominence: f32,
    /// 0..1, depth spread inside the guide.
    pub depth_range: f32,
    /// Mean luminance of the analyzed region, 0..255.
    pub brightness: f32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EarAnalysis {
    pub filters: Vec<AutoEqFilter>,
    pub preamp_db: f32,
    /// Estimated first concha resonance used by the curve.
    pub resonance_hz: f32,
    pub confidence: f32,
    pub metrics: EarMetrics,
    pub warnings: Vec<String>,
    pub overlay_png: String,
    pub model: String,
    pub elapsed_ms: u32,
}

pub fn load_photo(path: &str) -> Result<EarPhoto, String> {
    let file = Path::new(path);
    let size = std::fs::metadata(file)
        .map_err(|err| format!("Não foi possível abrir a foto: {err}"))?
        .len();
    if size > PHOTO_MAX_BYTES {
        return Err("A foto é muito grande (limite de 60 MB).".into());
    }
    let decoded = image::ImageReader::open(file)
        .map_err(|err| format!("Não foi possível abrir a foto: {err}"))?
        .with_guessed_format()
        .map_err(|err| format!("Formato de imagem desconhecido: {err}"))?
        .decode()
        .map_err(|err| format!("Não foi possível ler a foto: {err}"))?;
    let mut rgb = decoded.to_rgb8();
    let (mut width, mut height) = rgb.dimensions();
    if width.max(height) > PHOTO_MAX_SIDE {
        let scale = PHOTO_MAX_SIDE as f32 / width.max(height) as f32;
        width = ((width as f32 * scale).round() as u32).max(1);
        height = ((height as f32 * scale).round() as u32).max(1);
        rgb = image::imageops::resize(&rgb, width, height, FilterType::Triangle);
    }
    let mut png = Vec::new();
    rgb.write_to(
        &mut std::io::Cursor::new(&mut png),
        image::ImageFormat::Png,
    )
    .map_err(|err| format!("Falha ao preparar a foto: {err}"))?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(png);
    Ok(EarPhoto {
        data_url: format!("data:image/png;base64,{encoded}"),
        width,
        height,
    })
}

fn decode_crop(input: &str) -> Result<RgbImage, String> {
    let payload = input
        .find(',')
        .map(|comma| &input[comma + 1..])
        .unwrap_or(input);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|err| format!("Recorte inválido: {err}"))?;
    let image = image::load_from_memory(&bytes)
        .map_err(|err| format!("Não foi possível ler o recorte: {err}"))?;
    let mut rgb = image.to_rgb8();
    if rgb.width() as usize != CROP_SIZE || rgb.height() as usize != CROP_SIZE {
        rgb = image::imageops::resize(
            &rgb,
            CROP_SIZE as u32,
            CROP_SIZE as u32,
            FilterType::Triangle,
        );
    }
    Ok(rgb)
}

fn preprocess(image: &RgbImage) -> Vec<f32> {
    let resized = image::imageops::resize(
        image,
        INPUT_SIZE as u32,
        INPUT_SIZE as u32,
        FilterType::Triangle,
    );
    let plane = INPUT_SIZE * INPUT_SIZE;
    let mut tensor = vec![0.0f32; 3 * plane];
    for (index, pixel) in resized.pixels().enumerate() {
        for channel in 0..3 {
            let value = pixel[channel] as f32 / 255.0;
            tensor[channel * plane + index] = (value - IMAGE_MEAN[channel]) / IMAGE_STD[channel];
        }
    }
    tensor
}

fn round_to(value: f32, step: f32) -> f32 {
    (value / step).round() * step
}

/// Maps relative pinna geometry to a gentle correction curve.
///
/// The mapping follows the direction reported by pinna-anthropometry HRTF
/// studies: a deeper/larger concha lowers and strengthens the first concha
/// resonance, so the correction moves it back toward the population average.
/// It is a heuristic, not a measured transfer function.
pub fn map_filters(geometry: &geometry::Geometry) -> (Vec<AutoEqFilter>, f32, f32) {
    let depth = geometry.concha_depth.clamp(0.0, 1.0);
    let size = ((geometry.concha_size - 0.02) / 0.12).clamp(0.0, 1.0);
    let rim = geometry.helix_prominence.clamp(0.0, 1.0);

    let f1 = (4650.0 - 850.0 * size - 450.0 * depth).clamp(3_100.0, 4_700.0);
    let g1 = (1.1 - 3.4 * depth - 1.1 * size).clamp(-3.5, 1.5);
    let f2 = (f1 * 1.85).clamp(5_200.0, 8_500.0);
    let g2 = (g1 * 0.45).clamp(-1.8, 1.0);
    let g3 = ((rim - 0.45) * 2.4).clamp(-1.0, 1.5);

    let filters = vec![
        AutoEqFilter {
            kind: FilterKind::Peaking,
            freq: round_to(f1, 10.0),
            gain_db: round_to(g1, 0.1),
            q: 1.15,
        },
        AutoEqFilter {
            kind: FilterKind::Peaking,
            freq: round_to(f2, 10.0),
            gain_db: round_to(g2, 0.1),
            q: 1.9,
        },
        AutoEqFilter {
            kind: FilterKind::Peaking,
            freq: 7_600.0,
            gain_db: round_to(g3, 0.1),
            q: 0.9,
        },
    ];
    let max_boost = filters
        .iter()
        .map(|filter| filter.gain_db)
        .fold(0.0f32, f32::max);
    let preamp = -round_to(max_boost.max(0.0), 0.1);
    (filters, preamp, round_to(f1, 10.0))
}

pub fn analyze(image_data_url: &str) -> Result<EarAnalysis, String> {
    if !model::status().installed {
        return Err("Baixe o modelo de profundidade antes de analisar a foto.".into());
    }
    let started = Instant::now();
    let crop = decode_crop(image_data_url)?;
    let input = preprocess(&crop);
    let depth = inference::run(input)?.resized(CROP_SIZE, CROP_SIZE);
    let geometry = geometry::analyze(&crop, &depth)?;
    let (filters, preamp_db, resonance_hz) = map_filters(&geometry);
    let overlay_png = overlay::render(&crop, &geometry)?;

    Ok(EarAnalysis {
        filters,
        preamp_db,
        resonance_hz,
        confidence: geometry.confidence,
        metrics: EarMetrics {
            concha_depth: geometry.concha_depth,
            concha_size: geometry.concha_size,
            helix_prominence: geometry.helix_prominence,
            depth_range: geometry.depth_range,
            brightness: geometry.brightness,
        },
        warnings: geometry
            .warnings
            .iter()
            .map(|code| (*code).to_string())
            .collect(),
        overlay_png,
        model: model::MODEL_NAME.into(),
        elapsed_ms: started.elapsed().as_millis() as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geom(depth: f32, size: f32, rim: f32) -> geometry::Geometry {
        geometry::Geometry {
            basin: vec![false; CROP_SIZE * CROP_SIZE],
            basin_center_x: 256.0,
            basin_center_y: 320.0,
            concha_size: size,
            concha_depth: depth,
            helix_prominence: rim,
            depth_range: 0.4,
            brightness: 140.0,
            confidence: 0.8,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn deep_concha_cuts_the_lower_midrange() {
        let (filters, preamp, resonance) = map_filters(&geom(1.0, 0.16, 0.5));
        assert!(filters[0].gain_db < -2.0, "gain {}", filters[0].gain_db);
        assert!((3_000.0..=3_500.0).contains(&resonance), "f1 {resonance}");
        assert!(preamp <= 0.0);
    }

    #[test]
    fn shallow_concha_lifts_gently() {
        let (filters, preamp, resonance) = map_filters(&geom(0.05, 0.02, 0.5));
        assert!(filters[0].gain_db > 0.5, "gain {}", filters[0].gain_db);
        assert!(resonance > 4_500.0, "f1 {resonance}");
        assert!((preamp + filters[0].gain_db).abs() < 0.11);
    }

    #[test]
    fn filters_stay_inside_the_ear_band() {
        for depth in [0.0, 0.5, 1.0] {
            for size in [0.0, 0.07, 0.2] {
                for rim in [0.0, 0.5, 1.0] {
                    let (filters, preamp, _) = map_filters(&geom(depth, size, rim));
                    assert_eq!(filters.len(), 3);
                    for filter in &filters {
                        assert!((2_000.0..=8_500.0).contains(&filter.freq));
                        assert!(filter.gain_db.abs() <= 4.0);
                        assert!(filter.q > 0.1);
                    }
                    assert!(preamp <= 0.0);
                }
            }
        }
    }

    #[test]
    fn preprocessing_is_nchw_normalized() {
        let image = RgbImage::from_pixel(4, 4, image::Rgb([255, 128, 0]));
        let tensor = preprocess(&image);
        let plane = INPUT_SIZE * INPUT_SIZE;
        assert_eq!(tensor.len(), 3 * plane);
        assert!((tensor[0] - (1.0 - IMAGE_MEAN[0]) / IMAGE_STD[0]).abs() < 1e-4);
        assert!((tensor[plane] - (128.0 / 255.0 - IMAGE_MEAN[1]) / IMAGE_STD[1]).abs() < 1e-4);
        assert!(tensor[2 * plane] < -1.0);
    }
}
