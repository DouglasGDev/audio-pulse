//! Fractional delay line with linear interpolation.

#[derive(Clone, Debug)]
pub struct FractionalDelay {
    buffer: Vec<f32>,
    write: usize,
}

impl FractionalDelay {
    pub fn new(max_delay_samples: usize) -> Self {
        Self {
            buffer: vec![0.0; max_delay_samples.max(4) + 2],
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
        let max_delay = (len - 2) as f32;
        let d = if delay.is_finite() {
            delay.clamp(0.0, max_delay)
        } else {
            0.0
        };
        let whole = d.floor();
        let frac = d - whole;
        let base = self.write as isize - whole as isize;
        let i0 = base.rem_euclid(len as isize) as usize;
        let i1 = (i0 + len - 1) % len;
        self.write = (self.write + 1) % len;
        self.buffer[i0] * (1.0 - frac) + self.buffer[i1] * frac
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
}