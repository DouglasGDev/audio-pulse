//! Pinna geometry measured on a monocular depth map.
//!
//! The user frames the ear inside a fixed guide ellipse, so the analysis can
//! treat that ellipse as the pinna region. Within it we measure three proxies
//! that follow pinna acoustics:
//!
//! - how far the concha bowl sits behind the helix rim (depth delta),
//! - how much of the guide the bowl occupies (relative concha size),
//! - how pronounced the outer rim is (depth contrast at the helix).
//!
//! These are relative depth measurements, not millimetres and not an HRTF.

use image::RgbImage;

pub const CROP_SIZE: usize = 512;
pub const GUIDE_CX: f32 = 256.0;
pub const GUIDE_CY: f32 = 256.0;
pub const GUIDE_RX: f32 = 196.0;
pub const GUIDE_RY: f32 = 240.0;

// Normalized ellipse coordinates (u right, v down) used to locate the concha.
const BASIN_CENTER_U: f32 = 0.02;
const BASIN_CENTER_V: f32 = 0.29;
const BASIN_RX: f32 = 0.40;
const BASIN_RY: f32 = 0.34;
const BASIN_CENTER_DEPTH_BAND: f32 = 0.10;

fn concha_zone(u: f32, v: f32) -> Option<f32> {
    let du = (u - BASIN_CENTER_U) / BASIN_RX;
    let dv = (v - BASIN_CENTER_V) / BASIN_RY;
    let radius = (du * du + dv * dv).sqrt();
    (radius <= 1.0).then_some(radius)
}

#[derive(Debug, Clone)]
pub struct DepthMap {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl DepthMap {
    pub fn sample(&self, x: f32, y: f32) -> f32 {
        if self.width == 0 || self.height == 0 {
            return 0.0;
        }
        let x = x.clamp(0.0, (self.width - 1) as f32);
        let y = y.clamp(0.0, (self.height - 1) as f32);
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;
        let at = |px: usize, py: usize| self.data[py * self.width + px];
        let top = at(x0, y0) * (1.0 - fx) + at(x1, y0) * fx;
        let bottom = at(x0, y1) * (1.0 - fx) + at(x1, y1) * fx;
        let value = top * (1.0 - fy) + bottom * fy;
        if value.is_finite() {
            value
        } else {
            0.0
        }
    }

    pub fn resized(&self, width: usize, height: usize) -> DepthMap {
        let mut data = Vec::with_capacity(width * height);
        for y in 0..height {
            let sy = (y as f32 + 0.5) * self.height as f32 / height as f32 - 0.5;
            for x in 0..width {
                let sx = (x as f32 + 0.5) * self.width as f32 / width as f32 - 0.5;
                data.push(self.sample(sx, sy));
            }
        }
        Self {
            width,
            height,
            data,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Geometry {
    /// Basin pixels, row-major over the 512×512 crop.
    pub basin: Vec<bool>,
    pub basin_center_x: f32,
    pub basin_center_y: f32,
    /// Bowl area as a fraction of the guide ellipse.
    pub concha_size: f32,
    /// Rim-to-bowl depth delta, 0..1 (1 = bowl much deeper than the rim).
    pub concha_depth: f32,
    /// Outer-rim depth contrast, 0..1.
    pub helix_prominence: f32,
    /// Depth spread inside the guide, 0..1.
    pub depth_range: f32,
    pub brightness: f32,
    pub confidence: f32,
    pub warnings: Vec<&'static str>,
}

pub fn radius_at(x: usize, y: usize) -> f32 {
    let u = (x as f32 + 0.5 - GUIDE_CX) / GUIDE_RX;
    let v = (y as f32 + 0.5 - GUIDE_CY) / GUIDE_RY;
    (u * u + v * v).sqrt()
}

fn normalized_coords(x: usize, y: usize) -> (f32, f32) {
    (
        (x as f32 + 0.5 - GUIDE_CX) / GUIDE_RX,
        (y as f32 + 0.5 - GUIDE_CY) / GUIDE_RY,
    )
}

fn guide_mask() -> Vec<bool> {
    (0..CROP_SIZE * CROP_SIZE)
        .map(|index| radius_at(index % CROP_SIZE, index / CROP_SIZE) <= 1.0)
        .collect()
}

fn percentile(sorted: &[f32], percent: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let position = (percent / 100.0) * (sorted.len() - 1) as f32;
    let index = position.round().clamp(0.0, (sorted.len() - 1) as f32) as usize;
    sorted[index]
}

fn mean(values: &[f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f32>() / values.len() as f32
}

/// YCbCr skin chroma, used to keep the background and hair out of the depth
/// statistics. It is deliberately permissive: the depth thresholds do the
/// fine-grained separation.
fn skin_mask(rgb: &RgbImage) -> Vec<bool> {
    let mut mask = vec![false; CROP_SIZE * CROP_SIZE];
    for y in 0..CROP_SIZE {
        for x in 0..CROP_SIZE {
            let pixel = rgb.get_pixel(x as u32, y as u32);
            let r = pixel[0] as f32;
            let g = pixel[1] as f32;
            let b = pixel[2] as f32;
            let luma = 0.299 * r + 0.587 * g + 0.114 * b;
            let cb = 128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b;
            let cr = 128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b;
            mask[y * CROP_SIZE + x] =
                luma > 40.0 && (77.0..=127.0).contains(&cb) && (133.0..=173.0).contains(&cr);
        }
    }
    mask
}

/// Box blur that averages only pixels inside the guide, so background depth
/// never bleeds into the rim statistics.
fn blur_masked(source: &[f32], mask: &[bool], radius: isize) -> Vec<f32> {
    let mut out = vec![0.0f32; source.len()];
    for y in 0..CROP_SIZE {
        for x in 0..CROP_SIZE {
            let index = y * CROP_SIZE + x;
            if !mask[index] {
                continue;
            }
            let mut sum = 0.0f32;
            let mut count = 0u32;
            for dy in -radius..=radius {
                let yy = y as isize + dy;
                if yy < 0 || yy >= CROP_SIZE as isize {
                    continue;
                }
                for dx in -radius..=radius {
                    let xx = x as isize + dx;
                    if xx < 0 || xx >= CROP_SIZE as isize {
                        continue;
                    }
                    let neighbor = yy as usize * CROP_SIZE + xx as usize;
                    if mask[neighbor] {
                        sum += source[neighbor];
                        count += 1;
                    }
                }
            }
            if count > 0 {
                out[index] = sum / count as f32;
            }
        }
    }
    out
}

fn deepest_in_window(
    smooth: &[f32],
    mask: &[bool],
    skin: &[bool],
    skin_only: bool,
) -> Option<(usize, f32)> {
    let mut deepest: Option<(usize, f32)> = None;
    for y in 0..CROP_SIZE {
        for x in 0..CROP_SIZE {
            let index = y * CROP_SIZE + x;
            if !mask[index] || (skin_only && !skin[index]) {
                continue;
            }
            let (u, v) = normalized_coords(x, y);
            if concha_zone(u, v).is_none() {
                continue;
            }
            let value = smooth[index];
            if deepest.map_or(true, |(_, current)| value < current) {
                deepest = Some((index, value));
            }
        }
    }
    deepest
}

pub fn analyze(rgb: &RgbImage, depth: &DepthMap) -> Result<Geometry, String> {
    if depth.width != CROP_SIZE || depth.height != CROP_SIZE {
        return Err("O mapa de profundidade não está no tamanho esperado.".into());
    }
    if rgb.width() as usize != CROP_SIZE || rgb.height() as usize != CROP_SIZE {
        return Err("O recorte da foto não está no tamanho esperado.".into());
    }

    let mask = guide_mask();
    let ellipse_count = mask.iter().filter(|valid| **valid).count();
    let skin = skin_mask(rgb);
    let skin_count = (0..CROP_SIZE * CROP_SIZE)
        .filter(|index| mask[*index] && skin[*index])
        .count();
    let skin_ratio = skin_count as f32 / ellipse_count as f32;
    let mut raw: Vec<f32> = Vec::with_capacity(ellipse_count);
    for (index, valid) in mask.iter().enumerate() {
        if *valid {
            let value = depth.data[index];
            if value.is_finite() {
                raw.push(value);
            }
        }
    }
    if raw.len() < 1_000 {
        return Err("Não há profundidade suficiente na região da orelha.".into());
    }
    raw.sort_by(f32::total_cmp);
    let p2 = percentile(&raw, 2.0);
    let p98 = percentile(&raw, 98.0);
    let span = p98 - p2;
    if span.abs() < 1e-5 {
        return Err("A foto não tem contraste de profundidade suficiente.".into());
    }
    let p5 = ((percentile(&raw, 5.0) - p2) / span).clamp(0.0, 1.0);
    let p95 = ((percentile(&raw, 95.0) - p2) / span).clamp(0.0, 1.0);
    let depth_range = (p95 - p5).clamp(0.0, 1.0);

    // Smaller output = farther from the camera for Depth Anything's inverse
    // depth. Normalize to 0..1 where 1 is closest.
    let mut closeness = vec![0.0f32; CROP_SIZE * CROP_SIZE];
    for (index, valid) in mask.iter().enumerate() {
        if *valid {
            let value = depth.data[index];
            if value.is_finite() {
                closeness[index] = ((value - p2) / span).clamp(0.0, 1.0);
            }
        }
    }
    let smooth = blur_masked(&blur_masked(&closeness, &mask, 3), &mask, 3);

    let mut rim_all: Vec<f32> = Vec::new();
    let mut rim_skin: Vec<f32> = Vec::new();
    let mut helix_skin: Vec<f32> = Vec::new();
    for y in 0..CROP_SIZE {
        for x in 0..CROP_SIZE {
            let index = y * CROP_SIZE + x;
            if !mask[index] {
                continue;
            }
            let r = radius_at(x, y);
            if (0.78..0.965).contains(&r) {
                let value = smooth[index];
                rim_all.push(value);
                if skin[index] {
                    rim_skin.push(value);
                    let (_, v) = normalized_coords(x, y);
                    if v <= 0.20 {
                        helix_skin.push(value);
                    }
                }
            }
        }
    }
    let mut rim = if rim_skin.len() >= 200 { rim_skin } else { rim_all };
    rim.sort_by(f32::total_cmp);
    let rim_median = percentile(&rim, 50.0);
    let mut helix = helix_skin;
    if helix.len() < 200 {
        helix = rim.clone();
    }
    helix.sort_by(f32::total_cmp);
    let helix_prominence =
        ((percentile(&helix, 90.0) - percentile(&helix, 50.0)) / 0.20).clamp(0.0, 1.0);

    // The concha is the deepest connected region in the lower-middle part of
    // the guide; start from the deepest skin pixel and grow a bowl around it.
    // Skin keeps the far background out of that search.
    let deepest = deepest_in_window(&smooth, &mask, &skin, true)
        .or_else(|| deepest_in_window(&smooth, &mask, &skin, false));

    let mut basin = vec![false; CROP_SIZE * CROP_SIZE];
    let mut basin_values: Vec<f32> = Vec::new();
    let mut basin_sum_x = 0.0f64;
    let mut basin_sum_y = 0.0f64;
    let mut touches_border = false;
    let mut basin_count = 0usize;
    let ellipse_limit = ellipse_count / 3;
    if let Some((start, deepest_value)) = deepest {
        let band = (depth_range * 0.22).clamp(BASIN_CENTER_DEPTH_BAND, 0.22);
        let threshold = deepest_value + band;
        let mut stack = vec![start];
        while let Some(index) = stack.pop() {
            if basin[index] {
                continue;
            }
            let x = index % CROP_SIZE;
            let y = index / CROP_SIZE;
            if !mask[index] {
                continue;
            }
            let (u, v) = normalized_coords(x, y);
            let Some(zone_radius) = concha_zone(u, v) else {
                continue;
            };
            if smooth[index] > threshold {
                continue;
            }
            basin[index] = true;
            basin_count += 1;
            basin_values.push(smooth[index]);
            basin_sum_x += x as f64;
            basin_sum_y += y as f64;
            if zone_radius > 0.94 {
                touches_border = true;
            }
            if basin_count >= ellipse_limit {
                break;
            }
            if x > 0 {
                stack.push(index - 1);
            }
            if x + 1 < CROP_SIZE {
                stack.push(index + 1);
            }
            if y > 0 {
                stack.push(index - CROP_SIZE);
            }
            if y + 1 < CROP_SIZE {
                stack.push(index + CROP_SIZE);
            }
        }
    }

    let concha_size = basin_count as f32 / ellipse_count as f32;
    let basin_mean = mean(&basin_values);
    let rim_delta = (rim_median - basin_mean).max(0.0);
    // Half of the model's 0..1 inverse-depth span maps to a fully deep concha,
    // keeping the raw value graded instead of saturating on typical photos.
    let concha_depth = (rim_delta / 0.50).clamp(0.0, 1.0);
    let center_x = if basin_count > 0 {
        (basin_sum_x / basin_count as f64) as f32
    } else {
        GUIDE_CX
    };
    let center_y = if basin_count > 0 {
        (basin_sum_y / basin_count as f64) as f32
    } else {
        GUIDE_CY + 0.29 * GUIDE_RY
    };
    let (center_u, center_v) = normalized_coords(center_x.round() as usize, center_y.round() as usize);
    let center_distance = (((center_u - BASIN_CENTER_U) / 0.34).powi(2)
        + ((center_v - BASIN_CENTER_V) / 0.29).powi(2))
    .sqrt();

    let exposure_mask: Vec<bool> = if skin_count >= 1_000 {
        mask.iter()
            .zip(skin.iter())
            .map(|(inside, is_skin)| *inside && *is_skin)
            .collect()
    } else {
        mask.clone()
    };
    let (brightness, overexposed) = exposure(rgb, &exposure_mask);
    let contrast_score = ((depth_range - 0.06) / 0.20).clamp(0.0, 1.0);
    let bowl_score = ((rim_delta - 0.008) / 0.12).clamp(0.0, 1.0);
    let center_score = (1.0 - center_distance / 0.95).clamp(0.0, 1.0);
    let exposure_factor = if brightness < 45.0 {
        0.35
    } else if overexposed > 0.12 {
        0.55
    } else {
        1.0
    };
    let mut confidence =
        (0.42 * contrast_score + 0.38 * bowl_score + 0.20 * center_score) * exposure_factor;
    if touches_border {
        confidence *= 0.85;
    }
    if basin_count == 0 {
        confidence *= 0.25;
    }
    if skin_ratio < 0.10 {
        confidence *= 0.35;
    }

    let mut warnings = Vec::new();
    if depth_range < 0.10 {
        warnings.push("low_contrast");
    }
    if basin_count == 0 || rim_delta < 0.012 || concha_size < 0.01 {
        warnings.push("no_basin");
    }
    if basin_count > 0 && center_distance > 0.8 {
        warnings.push("off_center");
    }
    if skin_ratio < 0.10 {
        warnings.push("no_ear");
    }
    if brightness < 45.0 {
        warnings.push("dark");
    }
    if overexposed > 0.12 {
        warnings.push("overexposed");
    }

    Ok(Geometry {
        basin,
        basin_center_x: center_x,
        basin_center_y: center_y,
        concha_size,
        concha_depth,
        helix_prominence,
        depth_range,
        brightness,
        confidence: confidence.clamp(0.0, 1.0),
        warnings,
    })
}

fn exposure(rgb: &RgbImage, mask: &[bool]) -> (f32, f32) {
    let mut sum = 0.0f64;
    let mut count = 0u32;
    let mut clipped = 0u32;
    for y in 0..CROP_SIZE {
        for x in 0..CROP_SIZE {
            let index = y * CROP_SIZE + x;
            if !mask[index] {
                continue;
            }
            let pixel = rgb.get_pixel(x as u32, y as u32);
            let luma = 0.2126 * pixel[0] as f32 + 0.7152 * pixel[1] as f32 + 0.0722 * pixel[2] as f32;
            sum += luma as f64;
            count += 1;
            if luma > 245.0 {
                clipped += 1;
            }
        }
    }
    if count == 0 {
        return (0.0, 0.0);
    }
    (
        (sum / count as f64) as f32,
        clipped as f32 / count as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_depth() -> DepthMap {
        let mut data = vec![10.0f32; CROP_SIZE * CROP_SIZE];
        for y in 0..CROP_SIZE {
            for x in 0..CROP_SIZE {
                if radius_at(x, y) > 1.0 {
                    data[y * CROP_SIZE + x] = 0.0;
                    continue;
                }
                let dx = x as f32 - 265.0;
                let dy = y as f32 - 320.0;
                if (dx * dx + dy * dy).sqrt() < 45.0 {
                    data[y * CROP_SIZE + x] = 4.0;
                }
            }
        }
        DepthMap {
            width: CROP_SIZE,
            height: CROP_SIZE,
            data,
        }
    }

    #[test]
    fn finds_concha_bowl() {
        let rgb = RgbImage::from_pixel(CROP_SIZE as u32, CROP_SIZE as u32, image::Rgb([180, 150, 130]));
        let depth = synthetic_depth();
        let geometry = analyze(&rgb, &depth).expect("geometry");
        assert!(geometry.concha_size > 0.01, "size {}", geometry.concha_size);
        assert!((geometry.basin_center_x - 265.0).abs() < 8.0, "x {}", geometry.basin_center_x);
        assert!((geometry.basin_center_y - 320.0).abs() < 8.0, "y {}", geometry.basin_center_y);
        assert!(geometry.concha_depth > 0.5, "depth {}", geometry.concha_depth);
        assert!(geometry.confidence > 0.4, "confidence {}", geometry.confidence);
    }

    #[test]
    fn depth_map_resamples_bilinearly() {
        let map = DepthMap {
            width: 2,
            height: 2,
            data: vec![0.0, 1.0, 2.0, 3.0],
        };
        let resized = map.resized(4, 4);
        assert_eq!(resized.width, 4);
        assert!((resized.data[0] - 0.0).abs() < 0.3);
        assert!((resized.data[15] - 3.0).abs() < 0.3);
        assert!((resized.data[5] - 1.0).abs() < 0.4);
    }
}
