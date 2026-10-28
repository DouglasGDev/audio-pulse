//! Virtual surround and binaural rendering.
//!
//! Stereo is up-mixed to an 8-channel virtual layout (L, R, C, LFE, Ls, Rs,
//! Lb, Rb). Every virtual speaker is rendered through a simplified spherical
//! head model: Woodworth interaural time difference, frequency-dependent
//! contralateral shadowing, a pinna notch, and a shared early-reflection /
//! Schroeder reverb field. The result is a convincing externalized image from
//! ordinary stereo content without personalised HRIR measurements.
//!
//! Binaural mode bypasses the surround up-mix and virtualizes only L/R plus a
//! small centre anchor, which preserves the original mix while adding distance
//! and cross-ear cues. HRTF mode adds a dynamic layer on top: the virtual pair
//! tracks how decorrelated the incoming stereo mix is and opens the azimuth
//! from a near-frontal image (mono) to a wide one (hard panned or decorrelated
//! content), which modulates the Woodworth ITD, the shadow cutoff and the
//! far-ear level in real time.

use crate::biquad::{Biquad, OnePole};
use crate::delay::FractionalDelay;
use crate::params::{SpatialMode, SpatialParams};

const SPEED_OF_SOUND: f32 = 343.0;
const HEAD_RADIUS: f32 = 0.0875;
const HRTF_MIN_SPREAD_DEG: f32 = 20.0;
const HRTF_MAX_SPREAD_DEG: f32 = 78.0;

/// How often the moving pair is retuned. The azimuth itself is smoothed, so
/// these steps stay far below the threshold of a delay-line click.
const HRTF_UPDATE_SECONDS: f32 = 0.002;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SpeakerKind {
    Front,
    Center,
    Lfe,
    Side,
    Rear,
}

struct VirtualSpeaker {
    kind: SpeakerKind,
    base_azimuth: f32,
    level: f32,
    active: bool,
    was_active: bool,
    gain_l: f32,
    gain_r: f32,
    delay_l_samples: f32,
    delay_r_samples: f32,
    delay_l: FractionalDelay,
    delay_r: FractionalDelay,
    shadow_l: OnePole,
    shadow_r: OnePole,
    pinna_l: Biquad,
    pinna_r: Biquad,
}

impl VirtualSpeaker {
    fn new(kind: SpeakerKind, base_azimuth: f32, level: f32, sample_rate: f32) -> Self {
        Self {
            kind,
            base_azimuth,
            level,
            active: false,
            was_active: false,
            gain_l: 1.0,
            gain_r: 1.0,
            delay_l_samples: 0.0,
            delay_r_samples: 0.0,
            delay_l: FractionalDelay::new(512),
            delay_r: FractionalDelay::new(512),
            shadow_l: OnePole::low_pass(sample_rate, 19_000.0),
            shadow_r: OnePole::low_pass(sample_rate, 19_000.0),
            pinna_l: Biquad::identity(),
            pinna_r: Biquad::identity(),
        }
    }

    fn configure(&mut self, azimuth_deg: f32, level: f32, active: bool, sample_rate: f32) {
        if active != self.was_active {
            self.delay_l.reset();
            self.delay_r.reset();
            self.shadow_l.reset();
            self.shadow_r.reset();
            self.pinna_l.reset();
            self.pinna_r.reset();
            self.was_active = active;
        }
        self.active = active;
        self.level = level;
        if !active {
            return;
        }

        let az = azimuth_deg.clamp(-180.0, 180.0);
        let abs_az = az.abs();
        let effective = if abs_az > 90.0 { 180.0 - abs_az } else { abs_az };
        let radians = effective.to_radians();
        let itd_seconds = (HEAD_RADIUS / SPEED_OF_SOUND) * (radians + radians.sin());
        let itd_samples = itd_seconds * sample_rate;
        let base_delay = 0.00015 * sample_rate;

        if self.kind == SpeakerKind::Lfe {
            self.delay_l_samples = 0.0;
            self.delay_r_samples = 0.0;
            self.gain_l = 1.0;
            self.gain_r = 1.0;
            self.shadow_l.set_freq(sample_rate, 20_000.0);
            self.shadow_r.set_freq(sample_rate, 20_000.0);
            self.pinna_l = Biquad::identity();
            self.pinna_r = Biquad::identity();
            return;
        }

        let shadow_cutoff = 750.0 + 8_500.0 * radians.cos().max(0.0);
        let far_gain_db = if abs_az < 1.0 {
            0.0
        } else {
            2.5 + 9.5 * radians.sin()
        };
        let far_gain = 10f32.powf(-far_gain_db / 20.0).clamp(0.04, 1.0);
        let pinna_freq = if self.kind == SpeakerKind::Rear { 5_400.0 } else { 7_300.0 };
        let pinna_gain = if self.kind == SpeakerKind::Rear { -2.4 } else { -1.4 };
        self.pinna_l
            .set_peaking(sample_rate, pinna_freq, pinna_gain, 1.6);
        self.pinna_r
            .set_peaking(sample_rate, pinna_freq, pinna_gain, 1.6);

        if abs_az < 1.0 {
            self.delay_l_samples = base_delay;
            self.delay_r_samples = base_delay;
            self.gain_l = 1.0;
            self.gain_r = 1.0;
            self.shadow_l.set_freq(sample_rate, 19_000.0);
            self.shadow_r.set_freq(sample_rate, 19_000.0);
        } else if az < 0.0 {
            self.delay_l_samples = base_delay;
            self.delay_r_samples = base_delay + itd_samples;
            self.gain_l = 1.0;
            self.gain_r = far_gain;
            self.shadow_l.set_freq(sample_rate, 19_000.0);
            self.shadow_r.set_freq(sample_rate, shadow_cutoff);
        } else {
            self.delay_l_samples = base_delay + itd_samples;
            self.delay_r_samples = base_delay;
            self.gain_l = far_gain;
            self.gain_r = 1.0;
            self.shadow_l.set_freq(sample_rate, shadow_cutoff);
            self.shadow_r.set_freq(sample_rate, 19_000.0);
        }
    }

    #[inline]
    fn render(&mut self, sample: f32) -> (f32, f32) {
        if !self.active {
            return (0.0, 0.0);
        }
        let left = self.delay_l.write_read(sample, self.delay_l_samples);
        let right = self.delay_r.write_read(sample, self.delay_r_samples);
        if self.kind == SpeakerKind::Lfe {
            return (left * self.level, right * self.level);
        }
        let left = self.pinna_l.process(self.shadow_l.process(left));
        let right = self.pinna_r.process(self.shadow_r.process(right));
        (
            left * self.gain_l * self.level,
            right * self.gain_r * self.level,
        )
    }
}

struct Comb {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
    damping: f32,
    filter: f32,
}

impl Comb {
    fn new(sample_rate: f32, delay_ms: f32, feedback: f32) -> Self {
        let len = ((delay_ms / 1_000.0 * sample_rate) as usize).max(8);
        Self {
            buffer: vec![0.0; len],
            index: 0,
            feedback,
            damping: 0.35,
            filter: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let out = self.buffer[self.index];
        self.filter = out * (1.0 - self.damping) + self.filter * self.damping;
        self.buffer[self.index] = input + self.filter * self.feedback;
        self.index = (self.index + 1) % self.buffer.len();
        out
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
        self.filter = 0.0;
    }
}

struct AllPass {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
}

impl AllPass {
    fn new(sample_rate: f32, delay_ms: f32) -> Self {
        let len = ((delay_ms / 1_000.0 * sample_rate) as usize).max(4);
        Self {
            buffer: vec![0.0; len],
            index: 0,
            feedback: 0.5,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let buffered = self.buffer[self.index];
        let out = -input + buffered;
        self.buffer[self.index] = input + buffered * self.feedback;
        self.index = (self.index + 1) % self.buffer.len();
        out
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
    }
}

struct StereoReverb {
    pre_l: FractionalDelay,
    pre_r: FractionalDelay,
    combs_l: Vec<Comb>,
    combs_r: Vec<Comb>,
    aps_l: Vec<AllPass>,
    aps_r: Vec<AllPass>,
}

impl StereoReverb {
    fn new(sample_rate: f32) -> Self {
        let pre = (0.008 * sample_rate).clamp(1.0, 4_000.0);
        Self {
            pre_l: FractionalDelay::new(pre as usize + 8),
            pre_r: FractionalDelay::new(pre as usize + 8),
            combs_l: vec![
                Comb::new(sample_rate, 29.7, 0.68),
                Comb::new(sample_rate, 37.1, 0.66),
                Comb::new(sample_rate, 41.1, 0.65),
                Comb::new(sample_rate, 43.7, 0.63),
            ],
            combs_r: vec![
                Comb::new(sample_rate, 31.3, 0.67),
                Comb::new(sample_rate, 35.9, 0.66),
                Comb::new(sample_rate, 40.3, 0.64),
                Comb::new(sample_rate, 45.1, 0.62),
            ],
            aps_l: vec![AllPass::new(sample_rate, 5.0), AllPass::new(sample_rate, 1.7)],
            aps_r: vec![AllPass::new(sample_rate, 4.3), AllPass::new(sample_rate, 1.3)],
        }
    }

    #[inline]
    fn process(&mut self, input: f32, delay_samples: f32) -> (f32, f32) {
        let xl = self.pre_l.write_read(input, delay_samples);
        let xr = self.pre_r.write_read(input, delay_samples);
        let mut left = 0.0;
        let mut right = 0.0;
        for c in &mut self.combs_l {
            left += c.process(xl);
        }
        for c in &mut self.combs_r {
            right += c.process(xr);
        }
        left *= 0.25;
        right *= 0.25;
        for ap in &mut self.aps_l {
            left = ap.process(left);
        }
        for ap in &mut self.aps_r {
            right = ap.process(right);
        }
        (left, right)
    }

    fn reset(&mut self) {
        self.pre_l.reset();
        self.pre_r.reset();
        for c in &mut self.combs_l {
            c.reset();
        }
        for c in &mut self.combs_r {
            c.reset();
        }
        for a in &mut self.aps_l {
            a.reset();
        }
        for a in &mut self.aps_r {
            a.reset();
        }
    }
}

/// Follows the stereo image and turns it into an azimuth. A centre-heavy mix
/// (mid ≫ side) stays near the front; a hard-panned or decorrelated mix makes
/// the virtual pair open up. The envelopes use a fast attack and a slow
/// release so short transients do not swing the head model, and the azimuth is
/// smoothed again before it reaches the delay lines.
struct HrtfTracker {
    spread_deg: f32,
    mid_env: f32,
    side_env: f32,
    countdown: usize,
    update_interval: usize,
    env_attack: f32,
    env_release: f32,
    spread_attack: f32,
    spread_release: f32,
}

impl HrtfTracker {
    fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(8_000.0);
        let interval = ((HRTF_UPDATE_SECONDS * sr) as usize).max(16);
        Self {
            spread_deg: HRTF_MIN_SPREAD_DEG,
            mid_env: 0.0,
            side_env: 0.0,
            countdown: interval,
            update_interval: interval,
            env_attack: 1.0 - (-1.0 / (0.012 * sr)).exp(),
            env_release: 1.0 - (-1.0 / (0.120 * sr)).exp(),
            spread_attack: 1.0 - (-(interval as f32) / (0.035 * sr)).exp(),
            spread_release: 1.0 - (-(interval as f32) / (0.320 * sr)).exp(),
        }
    }

    fn reset(&mut self, width: f32) {
        self.spread_deg = HRTF_MIN_SPREAD_DEG * width.clamp(0.0, 1.5);
        self.mid_env = 0.0;
        self.side_env = 0.0;
        self.countdown = self.update_interval;
    }

    /// Called once per sample. Returns a new azimuth only when the retune
    /// interval elapses, so the caller can skip rebuilding geometry otherwise.
    fn track(&mut self, mid: f32, side: f32, width: f32) -> Option<f32> {
        let mid_abs = mid.abs();
        let side_abs = side.abs();
        let coeff = if mid_abs > self.mid_env {
            self.env_attack
        } else {
            self.env_release
        };
        self.mid_env += (mid_abs - self.mid_env) * coeff;
        let coeff = if side_abs > self.side_env {
            self.env_attack
        } else {
            self.env_release
        };
        self.side_env += (side_abs - self.side_env) * coeff;

        self.countdown = self.countdown.saturating_sub(1);
        if self.countdown > 0 {
            return None;
        }
        self.countdown = self.update_interval;

        // A hard-panned source gives |mid| = |side|; a centred one gives
        // |side| ≈ 0. Doubling keeps the normalised lateral factor in 0..1.
        let lateral =
            (2.0 * self.side_env / (self.mid_env + self.side_env + 1e-6)).clamp(0.0, 1.0);
        let target = (HRTF_MIN_SPREAD_DEG
            + (HRTF_MAX_SPREAD_DEG - HRTF_MIN_SPREAD_DEG) * lateral)
            * width.clamp(0.0, 1.5);
        let target = target.clamp(0.0, 88.0);
        let coeff = if target > self.spread_deg {
            self.spread_attack
        } else {
            self.spread_release
        };
        self.spread_deg += (target - self.spread_deg) * coeff;
        Some(self.spread_deg)
    }
}

pub struct SpatialStage {
    sample_rate: f32,
    mode: SpatialMode,
    width: f32,
    room: f32,
    last_mode: SpatialMode,
    last_width: f32,
    speakers: [VirtualSpeaker; 8],
    side_l: FractionalDelay,
    side_r: FractionalDelay,
    rear_l: FractionalDelay,
    rear_r: FractionalDelay,
    lfe_lp: Biquad,
    reverb: StereoReverb,
    wet_scale: f32,
    hrtf: HrtfTracker,
}

impl SpatialStage {
    pub fn new(sample_rate: f32) -> Self {
        let speakers = std::array::from_fn(|i| {
            let (kind, az, level) = match i {
                0 => (SpeakerKind::Front, -30.0, 1.0),
                1 => (SpeakerKind::Front, 30.0, 1.0),
                2 => (SpeakerKind::Center, 0.0, 0.75),
                3 => (SpeakerKind::Lfe, 0.0, 0.35),
                4 => (SpeakerKind::Side, -110.0, 0.72),
                5 => (SpeakerKind::Side, 110.0, 0.72),
                6 => (SpeakerKind::Rear, -150.0, 0.52),
                _ => (SpeakerKind::Rear, 150.0, 0.52),
            };
            VirtualSpeaker::new(kind, az, level, sample_rate)
        });
        let mut stage = Self {
            sample_rate,
            mode: SpatialMode::Off,
            width: 1.0,
            room: 0.35,
            last_mode: SpatialMode::Off,
            last_width: 1.0,
            speakers,
            side_l: FractionalDelay::new((0.030 * sample_rate) as usize + 16),
            side_r: FractionalDelay::new((0.030 * sample_rate) as usize + 16),
            rear_l: FractionalDelay::new((0.040 * sample_rate) as usize + 16),
            rear_r: FractionalDelay::new((0.040 * sample_rate) as usize + 16),
            lfe_lp: Biquad::low_pass(sample_rate, 90.0, 0.707),
            reverb: StereoReverb::new(sample_rate),
            wet_scale: 0.0,
            hrtf: HrtfTracker::new(sample_rate),
        };
        stage.set_params(SpatialParams::default(), sample_rate);
        stage
    }

    pub fn set_params(&mut self, params: SpatialParams, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.room = params.room.clamp(0.0, 1.0);
        self.wet_scale = self.room * 0.85;
        let mode_changed = params.mode != self.last_mode;
        let width_changed = (params.width - self.last_width).abs() > 0.002;
        if !mode_changed && !width_changed && params.mode == self.mode {
            return;
        }
        self.mode = params.mode;
        self.width = params.width.clamp(0.0, 1.5);
        self.last_mode = params.mode;
        self.last_width = self.width;

        if mode_changed {
            self.reverb.reset();
            self.side_l.reset();
            self.side_r.reset();
            self.rear_l.reset();
            self.rear_r.reset();
            self.lfe_lp = Biquad::low_pass(sample_rate, 90.0, 0.707);
            self.hrtf.reset(self.width);
        }

        for (i, speaker) in self.speakers.iter_mut().enumerate() {
            let (active, level, az) = match self.mode {
                SpatialMode::Off => (false, 0.0, speaker.base_azimuth),
                SpatialMode::Binaural => match i {
                    0 => (true, 0.9, -30.0),
                    1 => (true, 0.9, 30.0),
                    2 => (true, 0.16, 0.0),
                    _ => (false, 0.0, speaker.base_azimuth),
                },
                SpatialMode::Hrtf => match i {
                    0 => (true, 0.9, -self.hrtf.spread_deg),
                    1 => (true, 0.9, self.hrtf.spread_deg),
                    2 => (true, 0.15, 0.0),
                    _ => (false, 0.0, speaker.base_azimuth),
                },
                SpatialMode::Surround => {
                    let level = match i {
                        0 | 1 => 0.9,
                        2 => 0.58,
                        3 => 0.22,
                        4 | 5 => 0.7,
                        _ => 0.5,
                    };
                    (true, level, speaker.base_azimuth)
                }
            };
            let az = if matches!(speaker.kind, SpeakerKind::Lfe) || self.mode == SpatialMode::Hrtf
            {
                // HRTF already folds the width slider into its tracked spread.
                az
            } else {
                az * self.width
            };
            speaker.configure(az, level, active, sample_rate);
        }
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.mode == SpatialMode::Off {
            return (l, r);
        }

        let mid = (l + r) * 0.5;
        let side = (l - r) * 0.5;
        let mut channels = [0.0f32; 8];
        let ambient;
        let mut hrtf_spread = None;

        match self.mode {
            SpatialMode::Binaural => {
                channels[0] = l * 0.88;
                channels[1] = r * 0.88;
                channels[2] = mid * 0.16;
                ambient = mid * 0.10 + side * 0.10;
            }
            SpatialMode::Hrtf => {
                channels[0] = l * 0.88;
                channels[1] = r * 0.88;
                channels[2] = mid * 0.15;
                ambient = mid * 0.08 + side * 0.12;
                hrtf_spread = self.hrtf.track(mid, side, self.width);
            }
            SpatialMode::Surround => {
                let sr = self.sample_rate;
                let center = mid * 0.62;
                channels[0] = (l - mid * 0.33) * 0.9;
                channels[1] = (r - mid * 0.33) * 0.9;
                channels[2] = center;
                channels[3] = self.lfe_lp.process(mid) * 0.4;
                channels[4] = self
                    .side_l
                    .write_read(side, 0.009 * sr)
                    * 0.8;
                channels[5] = self
                    .side_r
                    .write_read(-side, 0.012 * sr)
                    * 0.8;
                channels[6] = self.rear_l.write_read(side, 0.019 * sr) * 0.55;
                channels[7] = self.rear_r.write_read(-side, 0.023 * sr) * 0.55;
                ambient = side * 0.42 + mid * 0.08;
            }
            SpatialMode::Off => unreachable!(),
        }

        if let Some(spread) = hrtf_spread {
            let sr = self.sample_rate;
            self.speakers[0].configure(-spread, 0.9, true, sr);
            self.speakers[1].configure(spread, 0.9, true, sr);
        }

        let mut out_l = 0.0;
        let mut out_r = 0.0;
        for (speaker, &signal) in self.speakers.iter_mut().zip(channels.iter()) {
            let (sl, sr) = speaker.render(signal);
            out_l += sl;
            out_r += sr;
        }

        if self.room > 0.001 {
            let (wet_l, wet_r) = self.reverb.process(ambient, 0.008 * self.sample_rate);
            out_l += wet_l * self.wet_scale;
            out_r += wet_r * self.wet_scale;
        }

        let trim = match self.mode {
            SpatialMode::Surround => 0.82,
            SpatialMode::Hrtf => 0.9,
            _ => 0.94,
        };
        (out_l * trim, out_r * trim)
    }

    pub fn reset(&mut self) {
        for s in &mut self.speakers {
            s.delay_l.reset();
            s.delay_r.reset();
            s.shadow_l.reset();
            s.shadow_r.reset();
            s.pinna_l.reset();
            s.pinna_r.reset();
        }
        self.side_l.reset();
        self.side_r.reset();
        self.rear_l.reset();
        self.rear_r.reset();
        self.lfe_lp.reset();
        self.reverb.reset();
        self.hrtf.reset(self.width);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binaural_decorrelates_and_stays_finite() {
        let sr = 48_000.0;
        let mut stage = SpatialStage::new(sr);
        stage.set_params(
            SpatialParams {
                mode: SpatialMode::Binaural,
                width: 1.0,
                room: 0.4,
            },
            sr,
        );
        let mut diff = 0.0f64;
        let mut energy = 0.0f64;
        for i in 0..48_000 {
            let x = 0.2 * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr).sin();
            let (l, r) = stage.process(x, x);
            assert!(l.is_finite() && r.is_finite());
            if i > 2_000 {
                diff += ((l - r) as f64).powi(2);
                energy += (l as f64).powi(2);
            }
        }
        assert!(diff > 1e-6, "expected binaural difference");
        assert!(energy > 0.0);
    }

    #[test]
    fn surround_produces_wider_image_than_off() {
        let sr = 48_000.0;
        let mut stage = SpatialStage::new(sr);
        stage.set_params(
            SpatialParams {
                mode: SpatialMode::Surround,
                width: 1.2,
                room: 0.3,
            },
            sr,
        );
        let mut side = 0.0f64;
        for i in 0..24_000 {
            let x = 0.2 * (2.0 * std::f32::consts::PI * 700.0 * i as f32 / sr).sin();
            let (l, r) = stage.process(x, 0.0);
            if i > 2_000 {
                side += ((l - r) as f64).powi(2);
            }
        }
        assert!(side > 1e-4);
    }

    #[test]
    fn hrtf_keeps_a_mono_mix_near_the_front() {
        let sr = 48_000.0;
        let mut stage = SpatialStage::new(sr);
        stage.set_params(
            SpatialParams {
                mode: SpatialMode::Hrtf,
                width: 1.0,
                room: 0.0,
            },
            sr,
        );
        for i in 0..24_000 {
            let x = 0.25 * (2.0 * std::f32::consts::PI * 600.0 * i as f32 / sr).sin();
            let (l, r) = stage.process(x, x);
            assert!(l.is_finite() && r.is_finite());
        }
        assert!(
            stage.hrtf.spread_deg < 28.0,
            "mono opened the image to {} deg",
            stage.hrtf.spread_deg
        );
    }

    #[test]
    fn hrtf_opens_up_and_shadows_the_far_ear_when_panned() {
        let sr = 48_000.0;
        let mut stage = SpatialStage::new(sr);
        stage.set_params(
            SpatialParams {
                mode: SpatialMode::Hrtf,
                width: 1.0,
                room: 0.0,
            },
            sr,
        );
        let mut left_energy = 0.0f64;
        let mut right_energy = 0.0f64;
        for i in 0..96_000 {
            let x = 0.3 * (2.0 * std::f32::consts::PI * 500.0 * i as f32 / sr).sin();
            let (l, r) = stage.process(x, 0.0);
            if i > 48_000 {
                left_energy += (l as f64).powi(2);
                right_energy += (r as f64).powi(2);
            }
        }
        assert!(
            stage.hrtf.spread_deg > 70.0,
            "panned content only reached {} deg",
            stage.hrtf.spread_deg
        );
        assert!(
            left_energy > right_energy * 3.0,
            "far ear was not shadowed: L {left_energy} R {right_energy}"
        );
    }
}