//! End-to-end measurements of the processing chain.
//!
//! These are not stage unit tests: they pin the numbers a listener actually
//! experiences — latency, ceiling margin, spectral flatness, bass handling and
//! headroom across the whole preset catalog — so later changes cannot regress
//! them silently. Run with `cargo test -p audio-pulse-dsp --test baseline --
//! --nocapture` to see the measured table.

use audio_pulse_dsp::{
    builtin_presets, find_builtin, CompressorParams, LimiterParams, ProcessingParams, Processor,
};

const SR: f32 = 48_000.0;
const TWO_PI: f32 = std::f32::consts::TAU;

fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

fn run_tone(processor: &mut Processor, freq: f32, amp: f32, seconds: f32) -> Vec<f32> {
    let frames = (seconds * SR) as usize;
    let mut buffer = vec![0.0f32; frames * 2];
    for i in 0..frames {
        let x = amp * (TWO_PI * freq * i as f32 / SR).sin();
        buffer[i * 2] = x;
        buffer[i * 2 + 1] = x;
    }
    processor.process_interleaved(&mut buffer);
    buffer.chunks_exact(2).map(|frame| frame[0]).collect()
}

fn tail_rms(samples: &[f32]) -> f64 {
    let tail = &samples[samples.len() / 2..];
    (tail.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / tail.len() as f64).sqrt()
}

fn gain_db(params: &ProcessingParams, freq: f32) -> f64 {
    let mut chain = Processor::new(SR);
    chain.set_params(params);
    let samples = run_tone(&mut chain, freq, 0.1, 1.0);
    20.0 * (tail_rms(&samples) / (0.1 / 2f64.sqrt())).log10()
}

#[test]
fn reported_latency_is_the_measured_latency() {
    let mut chain = Processor::new(SR);
    chain.set_params(&ProcessingParams {
        volume: 1.0,
        ..Default::default()
    });
    let reported = chain.latency_samples();

    let mut buffer = vec![0.0f32; 2_048 * 2];
    buffer[0] = 0.5;
    buffer[1] = 0.5;
    chain.process_interleaved(&mut buffer);
    let measured = buffer
        .chunks_exact(2)
        .position(|frame| frame[0].abs() > 1e-6)
        .expect("impulse vanished");

    println!(
        "latency: reported {reported} samples ({:.1} ms), measured {measured} samples",
        reported as f32 * 1_000.0 / SR
    );
    assert_eq!(measured, reported);
}

#[test]
fn limiter_holds_the_ceiling_on_an_inter_sample_peak() {
    let params = ProcessingParams {
        volume: 1.0,
        limiter: LimiterParams {
            enabled: true,
            ceiling_db: -0.3,
            release_ms: 80.0,
        },
        ..Default::default()
    };
    let ceiling = db_to_lin(-0.3);
    let mut chain = Processor::new(SR);
    chain.set_params(&params);

    // fs/4 at 45°: samples sit at ±0.707 of the waveform peak, so the sample
    // grid never shows the true peak. 1.6 linear is 4 dB over full scale. The
    // four-sample pattern is exact; accumulating the phase in f32 would drift
    // over 48 000 samples and move the peak between interpolation phases.
    let frames = (SR * 1.0) as usize;
    let step = 1.6 * std::f32::consts::FRAC_1_SQRT_2;
    let mut buffer = vec![0.0f32; frames * 2];
    for i in 0..frames {
        let x = if i % 4 < 2 { step } else { -step };
        buffer[i * 2] = x;
        buffer[i * 2 + 1] = x;
    }
    chain.process_interleaved(&mut buffer);

    let sample_peak = buffer[40_000..]
        .iter()
        .fold(0.0f32, |peak, s| peak.max(s.abs()));
    let analog_peak = sample_peak / std::f32::consts::FRAC_1_SQRT_2;
    let gr = chain.analysis().limiter_gr_db;

    println!(
        "true peak: sample {:.3} ({:+.1} dBFS), waveform {:.3} ({:+.1} dBFS), \
         ceiling {:.3} ({:+.1} dBFS), GR {:+.1} dB",
        sample_peak,
        20.0 * sample_peak.log10(),
        analog_peak,
        20.0 * analog_peak.log10(),
        ceiling,
        -0.3,
        gr
    );
    assert!(gr < -3.0, "the limiter barely engaged: {gr:.1} dB");
    assert!(
        sample_peak <= ceiling + 1e-3,
        "sample peak {sample_peak} above ceiling {ceiling}"
    );
    assert!(
        analog_peak <= ceiling * 1.03,
        "waveform peak {analog_peak} overshoots the ceiling {ceiling}"
    );
}

#[test]
fn music_compressor_leaves_deep_bass_alone() {
    let params = ProcessingParams {
        volume: 1.0,
        compressor: CompressorParams {
            enabled: true,
            threshold_db: -18.0,
            ratio: 2.0,
            attack_ms: 10.0,
            release_ms: 150.0,
            knee_db: 6.0,
            makeup_db: 0.0,
            sidechain_hpf: true,
        },
        limiter: LimiterParams {
            enabled: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bypass = params.clone();
    bypass.compressor.enabled = false;
    // Measure the delivered level, not the instantaneous meter: the detector
    // ripples with a 30 Hz tone, so one sample would misreport the kick.
    let reduction = |freq: f32| {
        let mut on = Processor::new(SR);
        on.set_params(&params);
        let mut off = Processor::new(SR);
        off.set_params(&bypass);
        let r_on = tail_rms(&run_tone(&mut on, freq, 0.9, 2.0));
        let r_off = tail_rms(&run_tone(&mut off, freq, 0.9, 2.0));
        20.0 * (r_on / r_off).log10()
    };
    let bass = reduction(30.0);
    let mid = reduction(1_000.0);
    println!("compressor level change: 30 Hz {bass:+.1} dB, 1 kHz {mid:+.1} dB");
    assert!(bass > -1.5, "a kick ducked the whole mix: {bass:.1} dB");
    assert!(mid < -4.0, "midrange was not compressed: {mid:.1} dB");
}

#[test]
fn protection_preset_is_spectrally_flat() {
    let params = find_builtin("protection")
        .expect("protection preset")
        .params;
    let reference = gain_db(&params, 1_000.0);
    for freq in [31.0, 62.0, 125.0] {
        let tilt = gain_db(&params, freq) - reference;
        println!("protection tilt at {freq} Hz: {tilt:+.2} dB");
        assert!(
            tilt.abs() < 1.0,
            "protection preset tilts {freq} Hz by {tilt:+.2} dB"
        );
    }
}

/// Every built-in preset, including the measured headphone corrections, must
/// keep a hot program inside its own limiter ceiling.
#[test]
fn presets_keep_a_hot_signal_under_their_ceiling() {
    let presets = builtin_presets();
    let mut worst: Option<(String, f32, f32, f32)> = None;
    for preset in &presets {
        let mut chain = Processor::new(SR);
        chain.set_params(&preset.params);
        let ceiling = db_to_lin(preset.params.limiter.ceiling_db);
        let frames = (SR * 0.5) as usize;
        let mut buffer = vec![0.0f32; frames * 2];
        for i in 0..frames {
            let x = 0.9 * (TWO_PI * 997.0 * i as f32 / SR).sin();
            buffer[i * 2] = x;
            buffer[i * 2 + 1] = x;
        }
        chain.process_interleaved(&mut buffer);
        assert!(
            buffer.iter().all(|sample| sample.is_finite()),
            "{} produced a non-finite sample",
            preset.id
        );
        let peak = buffer[4_000..]
            .iter()
            .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
        if peak > ceiling + 1e-3 {
            panic!("{} peaks at {peak:.3}, ceiling {ceiling:.3}", preset.id);
        }
        let margin = 20.0 * (ceiling / peak.max(1e-6)).log10();
        if worst
            .as_ref()
            .map(|(_, _, _, m)| margin < *m)
            .unwrap_or(true)
        {
            worst = Some((preset.id.clone(), peak, ceiling, margin));
        }
    }
    let (id, peak, ceiling, margin) = worst.expect("no presets");
    println!(
        "presets checked: {}, tightest margin {id} at {peak:.3} (ceiling {ceiling:.3}, {margin:.1} dB)",
        presets.len()
    );
}
