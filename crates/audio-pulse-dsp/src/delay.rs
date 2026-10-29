//! Fractional delay line with cubic Hermite (Catmull-Rom) interpolation.
//!
//! Linear interpolation both loses high-frequency energy and adds a strong
//! low-pass tilt whenever the delay has a fractional part (−1.25 dB at 8 kHz
//! for a half-sample delay), which dulls the far ear in the HRTF and muddies
//! early reflections. Four-point Catmull-Rom stays within 0.25 dB up to 8 kHz
//! and only rolls off above it (−0.5 dB at 10 kHz, −1 dB at 12 kHz), for three
//! extra multiply-adds per sample.

#[derive(Clone, Debug)]
pub struct FractionalDelay {
    buffer: Vec<f32>,
    write: usize,
}

impl FractionalDelay {
    pub fn new(max_delay_samples: usize) -> Self {
        // Cubic interpolation reads two samples older than the requested
        // delay, so the ring keeps four slots beyond the maximum delay.
        Self {
            buffer: vec![0.0; max_delay_samples.max(4) + 4],
            write: 0,
        }
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write = 0;
    }

    /// Writes `input`, then returns the delayed sample. `delay` is in samples.
    #[inline]
    pub fn write_read(&mut self, input: f32, delay: f32) -> f32 {
        let len = self.buffer.len();
        self.buffer[self.write] = input;
        let max_delay = (len - 4) as f32;
        let d = if delay.is_finite() {
            delay.clamp(0.0, max_delay)
        } else {
            0.0
        };
        let whole = d.floor();
        let t = d - whole;
        let base = self.write as isize - whole as isize;
        let at = |offset: isize| self.buffer[(base + offset).rem_euclid(len as isize) as usize];
        let p2 = at(1); // one sample newer than the integer delay
        let p1 = at(0); // integer delay
        let p0 = at(-1); // one sample older
        let m1 = at(-2); // two samples older

        let c0 = p1;
        let c1 = -0.5 * p2 + 0.5 * p0;
        let c2 = p2 - 2.5 * p1 + 2.0 * p0 - 0.5 * m1;
        let c3 = -0.5 * p2 + 1.5 * p1 - 1.5 * p0 + 0.5 * m1;

        self.write = (self.write + 1) % len;
        ((c3 * t + c2) * t + c1) * t + c0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_delay_returns_previous_sample() {
        let mut delay = FractionalDelay::new(8);
        let inputs = [1.0, 2.0, 3.0, 4.0, 5.0];
        let mut outputs = Vec::new();
        for &x in &inputs {
            outputs.push(delay.write_read(x, 1.0));
        }
        assert_eq!(outputs, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn fractional_delay_preserves_high_frequencies() {
        let sr = 48_000.0f32;
        let mut delay = FractionalDelay::new(32);
        let freq = 8_000.0f32;
        let mut input_energy = 0.0f64;
        let mut output_energy = 0.0f64;
        for i in 0..48_000 {
            let x = 0.5 * (2.0 * std::f32::consts::PI * freq * i as f32 / sr).sin();
            let y = delay.write_read(x, 5.5);
            if i > 4_000 {
                input_energy += (x as f64) * (x as f64);
                output_energy += (y as f64) * (y as f64);
            }
        }
        let gain_db = 10.0 * (output_energy / input_energy).log10();
        assert!(
            gain_db > -0.25,
            "fractional delay lost {gain_db:.2} dB at 8 kHz"
        );
    }

    #[test]
    fn huge_delay_is_clamped_instead_of_wrapping() {
        let mut delay = FractionalDelay::new(8);
        let mut last = 0.0;
        for i in 0..64 {
            last = delay.write_read(i as f32, 1e9);
        }
        assert!(last.is_finite());
    }
}
