//! Draws the analyzed region over the user's crop so the estimate is
//! inspectable instead of a black box.

use base64::Engine;
use image::{ImageFormat, Rgb, RgbImage};
use std::io::Cursor;

use super::geometry::{Geometry, CROP_SIZE, radius_at};

const GUIDE: [u8; 3] = [92, 228, 255];
const BASIN: [u8; 3] = [255, 150, 70];

fn blend(pixel: &mut Rgb<u8>, color: [u8; 3], alpha: f32) {
    for channel in 0..3 {
        let base = pixel[channel] as f32;
        pixel[channel] = (base * (1.0 - alpha) + color[channel] as f32 * alpha)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
}

pub fn render(rgb: &RgbImage, geometry: &Geometry) -> Result<String, String> {
    let mut canvas = rgb.clone();
    for y in 0..CROP_SIZE {
        for x in 0..CROP_SIZE {
            let radius = radius_at(x, y);
            let index = y * CROP_SIZE + x;
            if radius > 1.0 {
                let pixel = canvas.get_pixel_mut(x as u32, y as u32);
                for channel in 0..3 {
                    pixel[channel] = (pixel[channel] as f32 * 0.72) as u8;
                }
                continue;
            }
            if (radius - 1.0).abs() < 0.0045 {
                blend(canvas.get_pixel_mut(x as u32, y as u32), GUIDE, 0.55);
            } else if (radius - 0.78).abs() < 0.0035 {
                blend(canvas.get_pixel_mut(x as u32, y as u32), GUIDE, 0.22);
            }
            if geometry.basin[index] {
                blend(canvas.get_pixel_mut(x as u32, y as u32), BASIN, 0.26);
            }
        }
    }

    // Basin outline.
    for y in 1..CROP_SIZE - 1 {
        for x in 1..CROP_SIZE - 1 {
            let index = y * CROP_SIZE + x;
            if !geometry.basin[index] {
                continue;
            }
            let edge = !geometry.basin[index - 1]
                || !geometry.basin[index + 1]
                || !geometry.basin[index - CROP_SIZE]
                || !geometry.basin[index + CROP_SIZE];
            if edge {
                blend(canvas.get_pixel_mut(x as u32, y as u32), BASIN, 0.9);
            }
        }
    }

    // Center marker on the deepest point of the bowl.
    if geometry.basin.iter().any(|inside| *inside) {
        let cx = geometry.basin_center_x.round() as i32;
        let cy = geometry.basin_center_y.round() as i32;
        for offset in -6..=6 {
            for (x, y) in [(cx + offset, cy), (cx, cy + offset)] {
                if x >= 0 && y >= 0 && (x as usize) < CROP_SIZE && (y as usize) < CROP_SIZE {
                    blend(
                        canvas.get_pixel_mut(x as u32, y as u32),
                        [255, 220, 190],
                        0.85,
                    );
                }
            }
        }
    }

    let mut bytes = Vec::new();
    canvas
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|err| format!("Falha ao gerar a visualização: {err}"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}
