//! Pinna geometry measured on a monocular depth map.
//!
//! The user frames the ear inside a fixed guide ellipse, so the analysis can
//! treat that ellipse as the pinna region. Inside it we measure three proxies
//! that follow pinna acoustics:
//!
//! - how far the concha bowl sits behind the ear's own rim (depth delta),
//! - how much of the expected concha zone the bowl fills (relative size),
//! - how pronounced the helix rim is (depth contrast at the upper rim).
//!
//! The concha zone is centred near the guide but searched over a small window,
//! because framing offsets move the bowl by tens of pixels. Depth is normalized
//! by the depth spread of the ear itself. These are relative measurements, not
//! millimetres and not an HRTF.

use image::RgbImage;

pub const CROP_SIZE: usize = 512;
pub const GUIDE_CX: f32 = 256.0;
pub const GUIDE_CY: f32 = 256.0;
pub const GUIDE_RX: f32 = 196.0;
pub const GUIDE_RY: f32 = 240.0;

// Concha zone size, in guide-normalized units (u right, v down).
const ZONE_RX: f32 = 0.30;
const ZONE_RY: f32 = 0.28;
// Search window for the zone centre. It covers framing offsets without
// letting a shadow at the edge of the guide become the concha.
const SEARCH_U: f32 = 0.30;
const SEARCH_V_MIN: f32 = -0.22;
const SEARCH_V_MAX: f32 = 0.30;
const SEARCH_STEP: f32 = 0.04;
/// Inner disc used as the bowl reference, as a fraction of the zone radius.
const INNER_SCALE: f32 = 0.5;
/// Rim reference ring around the zone.
const RING_INNER: f32 = 0.95;
const RING_OUTER: f32 = 1.30;
const RING_SECTORS: usize = 12;
/// The centre may move this far (in zone radii) for free before the score is
/// penalized, which keeps shadows near the guide border from winning.
const CENTER_FREE_DISTANCE: f32 = 0.70;
const CENTER_PENALTY: f32 = 0.10;
/// Fraction of the rim-to-bowl contrast that defines the basin contour.
const BASIN_DEPTH_FRACTION: f32 = 0.5;
/// Ear-local rim-to-bowl contrast that maps to a fully deep concha.
/// Calibrated on real ear photos.
const BOWL_DEPTH_SCALE: f32 = 0.22;

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
    /// Centre of the selected concha zone, in crop pixels.
    pub zone_center_x: f32,
    pub zone_center_y: f32,
    /// Bowl area as a fraction of the concha zone.
    pub concha_size: f32,
    /// Rim-to-bowl depth contrast relative to the ear's own depth spread, 0..1.
    pub concha_depth: f32,
    /// Upper-rim depth contrast, 0..1.
    pub helix_prominence: f32,
    /// Depth spread inside the guide, 0..1.
    pub depth_range: f32,
    pub brightness: f32,
    pub confidence: f32,
    pub warnings: Vec<&'static str>,
}

/// Distance from the guide centre, 1.0 at its border.
pub fn radius_at(x: usize, y: usize) -> f32 {
    let (u, v) = normalized_at(x as f32 + 0.5, y as f32 + 0.5);
    (u * u + v * v).sqrt()
}

/// Distance from a concha zone centre, 1.0 at its border. The centre is given
/// in crop pixels.
pub fn zone_radius_at(x: usize, y: usize, center_x: f32, center_y: f32) -> f32 {
    let (u, v) = normalized_at(x as f32 + 0.5, y as f32 + 0.5);
    let (center_u, center_v) = normalized_at(center_x, center_y);
    zone_radius(u, v, center_u, center_v)
}

fn normalized_at(x: f32, y: f32) -> (f32, f32) {
    ((x - GUIDE_CX) / GUIDE_RX, (y - GUIDE_CY) / GUIDE_RY)
}

fn zone_radius(u: f32, v: f32, center_u: f32, center_v: f32) -> f32 {
    let du = (u - center_u) / ZONE_RX;
    let dv = (v - center_v) / ZONE_RY;
    (du * du + dv * dv).sqrt()
}

fn guide_radius(u: f32, v: f32) -> f32 {
    (u * u + v * v).sqrt()
}

fn guide_mask() -> Vec<bool> {
    (0..CROP_SIZE * CROP_SIZE)
        .map(|index| guide_radius_at_index(index) <= 1.0)
        .collect()
}

fn guide_radius_at_index(index: usize) -> f32 {
    let x = (index % CROP_SIZE) as f32 + 0.5;
    let y = (index / CROP_SIZE) as f32 + 0.5;
    let (u, v) = normalized_at(x, y);
    guide_radius(u, v)
}

fn percentile(sorted: &[f32], percent: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let position = (percent / 100.0) * (sorted.len() - 1) as f32;
    let index = position.round().clamp(0.0, (sorted.len() - 1) as f32) as usize;
    sorted[index]
}

fn median_in_place(values: &mut [f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    let mid = values.len() / 2;
    values.select_nth_unstable_by(mid, f32::total_cmp);
    values[mid]
}

fn percentile_owned(mut values: Vec<f32>, percent: f32) -> f32 {
    values.sort_by(f32::total_cmp);
    percentile(&values, percent)
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
/// never bleeds into the concha statistics.
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

struct Candidate {
    u: f32,
    v: f32,
    inner: f32,
    rim_low: f32,
    sectors: [f32; RING_SECTORS],
    valid_sectors: usize,
    score: f32,
    distance: f32,
}

/// Finds the zone centre whose inner disc sits deepest behind an enclosing rim.
/// Lower-quartile ring statistics require the rim to be high on every side, not
/// just on the side facing a shadow.
fn search_zone(
    u_at: &[f32],
    v_at: &[f32],
    smooth: &[f32],
    mask: &[bool],
    skin: &[bool],
    use_skin: bool,
    ear_span: f32,
) -> Option<Candidate> {
    let mut best: Option<Candidate> = None;
    let steps_v = ((SEARCH_V_MAX - SEARCH_V_MIN) / SEARCH_STEP).round() as i32;
    let steps_u = ((2.0 * SEARCH_U) / SEARCH_STEP).round() as i32;
    for iv in 0..=steps_v {
        let center_v = SEARCH_V_MIN + iv as f32 * SEARCH_STEP;
        for iu in 0..=steps_u {
            let center_u = -SEARCH_U + iu as f32 * SEARCH_STEP;
            let mut inside = true;
            for k in 0..32 {
                let angle = k as f32 * std::f32::consts::TAU / 32.0;
                let u = center_u + ZONE_RX * angle.cos();
                let v = center_v + ZONE_RY * angle.sin();
                if guide_radius(u, v) > 1.0 {
                    inside = false;
                    break;
                }
            }
            if !inside {
                continue;
            }

            let center_x = GUIDE_CX + center_u * GUIDE_RX;
            let center_y = GUIDE_CY + center_v * GUIDE_RY;
            let reach_x = ZONE_RX * GUIDE_RX * RING_OUTER;
            let reach_y = ZONE_RY * GUIDE_RY * RING_OUTER;
            let x0 = ((center_x - reach_x).floor() as isize).max(0) as usize;
            let x1 = ((center_x + reach_x).ceil() as isize).clamp(0, CROP_SIZE as isize - 1) as usize;
            let y0 = ((center_y - reach_y).floor() as isize).max(0) as usize;
            let y1 = ((center_y + reach_y).ceil() as isize).clamp(0, CROP_SIZE as isize - 1) as usize;

            let mut inner_all: Vec<f32> = Vec::new();
            let mut inner_skin: Vec<f32> = Vec::new();
            let mut ring_all: [Vec<f32>; RING_SECTORS] = std::array::from_fn(|_| Vec::new());
            let mut ring_skin: [Vec<f32>; RING_SECTORS] = std::array::from_fn(|_| Vec::new());

            for y in y0..=y1 {
                for x in x0..=x1 {
                    let index = y * CROP_SIZE + x;
                    if !mask[index] {
                        continue;
                    }
                    let du = (u_at[index] - center_u) / ZONE_RX;
                    let dv = (v_at[index] - center_v) / ZONE_RY;
                    let radius = (du * du + dv * dv).sqrt();
                    if radius <= INNER_SCALE {
                        inner_all.push(smooth[index]);
                        if skin[index] {
                            inner_skin.push(smooth[index]);
                        }
                    } else if (RING_INNER..=RING_OUTER).contains(&radius) {
                        let angle = dv.atan2(du);
                        let normalized = (angle + std::f32::consts::PI) / std::f32::consts::TAU;
                        let sector = (normalized * RING_SECTORS as f32) as usize;
                        let sector = sector.min(RING_SECTORS - 1);
                        ring_all[sector].push(smooth[index]);
                        if skin[index] {
                            ring_skin[sector].push(smooth[index]);
                        }
                    }
                }
            }

            let mut inner_values = if use_skin && inner_skin.len() >= 200 {
                inner_skin
            } else {
                inner_all
            };
            if inner_values.len() < 200 {
                continue;
            }
            let inner = median_in_place(&mut inner_values);

            let mut sectors = [f32::NAN; RING_SECTORS];
            let mut medians: Vec<f32> = Vec::with_capacity(RING_SECTORS);
            for k in 0..RING_SECTORS {
                let values = if use_skin && ring_skin[k].len() >= 8 {
                    &ring_skin[k]
                } else {
                    &ring_all[k]
                };
                if values.len() < 8 {
                    continue;
                }
                let mut copy = values.clone();
                let median = median_in_place(&mut copy);
                sectors[k] = median;
                medians.push(median);
            }
            if medians.len() < 8 {
                continue;
            }
            let valid_sectors = medians.len();
            let rim_low = percentile_owned(medians, 25.0);
            let contrast = (rim_low - inner) / ear_span;
            let distance =
                ((center_u / ZONE_RX).powi(2) + (center_v / ZONE_RY).powi(2)).sqrt();
            let score = contrast - CENTER_PENALTY * (distance - CENTER_FREE_DISTANCE).max(0.0);
            if best.as_ref().map_or(true, |current| score > current.score) {
                best = Some(Candidate {
                    u: center_u,
                    v: center_v,
                    inner,
                    rim_low,
                    sectors,
                    valid_sectors,
                    score,
                    distance,
                });
            }
        }
    }
    best
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
    let mut u_at = vec![0.0f32; CROP_SIZE * CROP_SIZE];
    let mut v_at = vec![0.0f32; CROP_SIZE * CROP_SIZE];
    for (index, valid) in mask.iter().enumerate() {
        let x = (index % CROP_SIZE) as f32 + 0.5;
        let y = (index / CROP_SIZE) as f32 + 0.5;
        let (u, v) = normalized_at(x, y);
        u_at[index] = u;
        v_at[index] = v;
        if *valid {
            let value = depth.data[index];
            if value.is_finite() {
                closeness[index] = ((value - p2) / span).clamp(0.0, 1.0);
            }
        }
    }
    let smooth = blur_masked(&blur_masked(&closeness, &mask, 3), &mask, 3);

    // The ear's own depth spread is the right yardstick for the bowl: the
    // background stays out of it, so a distant wall no longer flattens the
    // measurement.
    let skin_is_reliable = skin_count >= 1_000;
    let mut spread_values: Vec<f32> = Vec::with_capacity(ellipse_count);
    for (index, valid) in mask.iter().enumerate() {
        if *valid && (!skin_is_reliable || skin[index]) {
            spread_values.push(smooth[index]);
        }
    }
    spread_values.sort_by(f32::total_cmp);
    let ear_span =
        (percentile(&spread_values, 90.0) - percentile(&spread_values, 10.0)).max(0.04);

    let mut rim_all: Vec<f32> = Vec::new();
    let mut rim_skin: Vec<f32> = Vec::new();
    let mut helix_skin: Vec<f32> = Vec::new();
    for y in 0..CROP_SIZE {
        for x in 0..CROP_SIZE {
            let index = y * CROP_SIZE + x;
            if !mask[index] {
                continue;
            }
            let r = guide_radius(u_at[index], v_at[index]);
            if (0.65..0.98).contains(&r) {
                let value = smooth[index];
                rim_all.push(value);
                if skin[index] {
                    rim_skin.push(value);
                    if v_at[index] <= 0.30 {
                        helix_skin.push(value);
                    }
                }
            }
        }
    }
    let mut helix = if helix_skin.len() >= 200 {
        helix_skin
    } else if rim_skin.len() >= 200 {
        rim_skin
    } else {
        rim_all
    };
    helix.sort_by(f32::total_cmp);
    let helix_prominence =
        ((percentile(&helix, 90.0) - percentile(&helix, 50.0)) / 0.25).clamp(0.0, 1.0);

    let best = search_zone(
        &u_at,
        &v_at,
        &smooth,
        &mask,
        &skin,
        skin_is_reliable,
        ear_span,
    );
    let (zone_u, zone_v, inner, rim_low, sectors, valid_sectors, zone_distance) = match best {
        Some(candidate) => (
            candidate.u,
            candidate.v,
            candidate.inner,
            candidate.rim_low,
            candidate.sectors,
            candidate.valid_sectors,
            candidate.distance,
        ),
        None => {
            return Err("Não foi possível localizar a concha na foto.".into());
        }
    };
    let zone_center_x = GUIDE_CX + zone_u * GUIDE_RX;
    let zone_center_y = GUIDE_CY + zone_v * GUIDE_RY;

    // The basin is everything in the zone deeper than half the rim-to-bowl
    // contrast, connected to the deepest point. A half-contrast contour moves
    // smoothly with the measurement instead of riding a fixed depth band.
    let contrast = (rim_low - inner) / ear_span;
    let threshold = inner + BASIN_DEPTH_FRACTION * (rim_low - inner);
    let mut zone_count = 0usize;
    for y in 0..CROP_SIZE {
        for x in 0..CROP_SIZE {
            let index = y * CROP_SIZE + x;
            if mask[index] && zone_radius(u_at[index], v_at[index], zone_u, zone_v) <= 1.0 {
                zone_count += 1;
            }
        }
    }

    let mut seed: Option<(usize, f32)> = None;
    for y in 0..CROP_SIZE {
        for x in 0..CROP_SIZE {
            let index = y * CROP_SIZE + x;
            if !mask[index] || zone_radius(u_at[index], v_at[index], zone_u, zone_v) > 0.85 {
                continue;
            }
            if skin_is_reliable && !skin[index] {
                continue;
            }
            let value = smooth[index];
            if seed.map_or(true, |(_, current)| value < current) {
                seed = Some((index, value));
            }
        }
    }
    if seed.is_none() {
        // Skin detection can reject a concha in deep shadow; retry without it.
        for y in 0..CROP_SIZE {
            for x in 0..CROP_SIZE {
                let index = y * CROP_SIZE + x;
                if !mask[index] || zone_radius(u_at[index], v_at[index], zone_u, zone_v) > 0.85 {
                    continue;
                }
                let value = smooth[index];
                if seed.map_or(true, |(_, current)| value < current) {
                    seed = Some((index, value));
                }
            }
        }
    }

    let mut basin = vec![false; CROP_SIZE * CROP_SIZE];
    let mut basin_count = 0usize;
    let mut basin_sum_x = 0.0f64;
    let mut basin_sum_y = 0.0f64;
    let mut touches_border = false;
    if let Some((start, _)) = seed {
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
            let radius = zone_radius(u_at[index], v_at[index], zone_u, zone_v);
            if radius > 1.0 {
                continue;
            }
            if smooth[index] > threshold {
                continue;
            }
            basin[index] = true;
            basin_count += 1;
            basin_sum_x += x as f64;
            basin_sum_y += y as f64;
            if radius > 0.94 {
                touches_border = true;
            }
            if basin_count >= zone_count {
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

    let basin_detected = basin_count >= 200 && contrast >= 0.05;
    let concha_size = if basin_detected && zone_count > 0 {
        basin_count as f32 / zone_count as f32
    } else {
        0.0
    };
    let concha_depth = if basin_detected {
        (contrast / BOWL_DEPTH_SCALE).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let center_x = if basin_count > 0 {
        (basin_sum_x / basin_count as f64) as f32
    } else {
        zone_center_x
    };
    let center_y = if basin_count > 0 {
        (basin_sum_y / basin_count as f64) as f32
    } else {
        zone_center_y
    };

    let enclosure_midpoint = inner + BASIN_DEPTH_FRACTION * (rim_low - inner);
    let enclosed = sectors
        .iter()
        .filter(|median| median.is_finite() && **median >= enclosure_midpoint)
        .count();
    let enclosure_score = if valid_sectors > 0 {
        enclosed as f32 / valid_sectors as f32
    } else {
        0.0
    };

    let exposure_mask: Vec<bool> = if skin_is_reliable {
        mask.iter()
            .zip(skin.iter())
            .map(|(inside, is_skin)| *inside && *is_skin)
            .collect()
    } else {
        mask.clone()
    };
    let (brightness, overexposed) = exposure(rgb, &exposure_mask);
    let contrast_score = ((ear_span - 0.06) / 0.40).clamp(0.0, 1.0);
    let bowl_score = ((contrast - 0.04) / 0.20).clamp(0.0, 1.0);
    let skin_score = ((skin_ratio - 0.12) / 0.40).clamp(0.0, 1.0);
    let exposure_factor = if brightness < 45.0 {
        0.35
    } else if overexposed > 0.12 {
        0.55
    } else {
        1.0
    };
    let mut confidence = (0.30 * contrast_score
        + 0.35 * bowl_score
        + 0.15 * enclosure_score
        + 0.20 * skin_score)
        * exposure_factor;
    if touches_border {
        confidence *= 0.85;
    }
    if !basin_detected {
        confidence *= 0.25;
    }
    if skin_ratio < 0.12 {
        confidence *= 0.40;
    }

    let mut warnings = Vec::new();
    if ear_span < 0.10 {
        warnings.push("low_contrast");
    }
    if !basin_detected {
        warnings.push("no_basin");
    }
    if zone_distance > 0.95 {
        warnings.push("off_center");
    }
    if skin_ratio < 0.12 {
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
        zone_center_x,
        zone_center_y,
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
                let dx = x as f32 - 262.0;
                let dy = y as f32 - 300.0;
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
        assert!(geometry.concha_size > 0.10, "size {}", geometry.concha_size);
        assert!((geometry.basin_center_x - 262.0).abs() < 15.0, "x {}", geometry.basin_center_x);
        assert!((geometry.basin_center_y - 300.0).abs() < 15.0, "y {}", geometry.basin_center_y);
        assert!(geometry.concha_depth > 0.5, "depth {}", geometry.concha_depth);
        assert!(geometry.confidence > 0.4, "confidence {}", geometry.confidence);
    }

    #[test]
    fn zone_moves_toward_the_bowl() {
        let rgb = RgbImage::from_pixel(CROP_SIZE as u32, CROP_SIZE as u32, image::Rgb([180, 150, 130]));
        let depth = synthetic_depth();
        let geometry = analyze(&rgb, &depth).expect("geometry");
        assert!(
            (geometry.zone_center_y - 300.0).abs() < 30.0,
            "zone y {}",
            geometry.zone_center_y
        );
    }

    #[test]
    fn flat_depth_has_low_confidence() {
        let rgb = RgbImage::from_pixel(CROP_SIZE as u32, CROP_SIZE as u32, image::Rgb([180, 150, 130]));
        let depth = DepthMap {
            width: CROP_SIZE,
            height: CROP_SIZE,
            data: vec![7.0; CROP_SIZE * CROP_SIZE],
        };
        assert!(analyze(&rgb, &depth).is_err());
    }

    #[test]
    fn bowl_outside_the_search_window_is_ignored() {
        let rgb = RgbImage::from_pixel(CROP_SIZE as u32, CROP_SIZE as u32, image::Rgb([180, 150, 130]));
        let mut depth = synthetic_depth();
        for y in 0..CROP_SIZE {
            for x in 0..CROP_SIZE {
                if radius_at(x, y) <= 1.0 {
                    depth.data[y * CROP_SIZE + x] = 10.0;
                }
            }
        }
        // Bowl at the top of the guide, far from the concha search window.
        for y in 0..CROP_SIZE {
            for x in 0..CROP_SIZE {
                let dx = x as f32 - 256.0;
                let dy = y as f32 - 90.0;
                if dx * dx + dy * dy < 40.0 * 40.0 {
                    depth.data[y * CROP_SIZE + x] = 4.0;
                }
            }
        }
        let geometry = analyze(&rgb, &depth).expect("geometry");
        assert!(geometry.concha_depth < 0.1, "depth {}", geometry.concha_depth);
        assert!(geometry.warnings.contains(&"no_basin"), "warnings {:?}", geometry.warnings);
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
