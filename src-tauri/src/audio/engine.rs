//! Real-time audio engine.
//!
//! cpal streams are `!Send` on some platforms, so a dedicated engine thread
//! owns them for their whole lifetime. The UI only talks to the thread through
//! commands, and the audio callbacks read parameters from an [`ArcSwap`],
//! which keeps the hot path free of locks and allocations.

use crate::audio::devices;
use crate::models::{EngineInfo, Telemetry};
use arc_swap::ArcSwap;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, Device, FromSample, Sample, SampleFormat, SizedSample, Stream, StreamConfig};
use parking_lot::Mutex;
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use audio_pulse_dsp::{AnalysisSnapshot, ProcessingParams, Processor, ADAPTIVE_EQ_BANDS, SPECTRUM_BANDS};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::f64::consts::TAU;

/// The look-ahead limiter delays the signal by 5 ms.
const LOOKAHEAD_MS: f32 = 5.0;
const MIN_FRAMES: usize = 64;
const MAX_FRAMES: usize = 16_384;

#[derive(Debug, Clone)]
pub enum PreviewSource {
    /// Decorrelated pink noise, useful to audition spatial rendering.
    Noise,
    /// Logarithmic 20 Hz → 20 kHz sweep.
    Sweep,
    /// A user-selected WAV file, decoded and resampled at start time.
    Track(String),
}

#[derive(Debug, Clone)]
pub enum EngineStart {
    Route {
        input: Option<String>,
        output: Option<String>,
        buffer_ms: u32,
    },
    Preview {
        output: Option<String>,
        buffer_ms: u32,
        source: PreviewSource,
    },
}

/// State shared between the UI, the engine thread and the audio callbacks.
pub struct EngineShared {
    pub params: ArcSwap<ProcessingParams>,
    running: AtomicBool,
    sample_rate: AtomicU32,
    buffer_ms: AtomicU32,
    latency_us: AtomicU32,
    cpu_permille: AtomicU32,
    /// Current ring buffer fill, in stereo samples. Used as drift feedback.
    fill_samples: AtomicU32,
    xruns: AtomicU64,
    /// The Windows endpoint applies the master volume; the DSP must not.
    hardware_volume: AtomicBool,
    /// The next parameter block replaces whole filter sets; fade around it.
    declick: AtomicBool,
    mode: Mutex<String>,
    error: Mutex<Option<String>>,
    input_name: Mutex<Option<String>>,
    output_name: Mutex<Option<String>>,
    telemetry: Mutex<Telemetry>,
}

impl EngineShared {
    fn new(params: ProcessingParams) -> Self {
        Self {
            params: ArcSwap::from_pointee(params),
            running: AtomicBool::new(false),
            sample_rate: AtomicU32::new(0),
            buffer_ms: AtomicU32::new(25),
            latency_us: AtomicU32::new(0),
            cpu_permille: AtomicU32::new(0),
            fill_samples: AtomicU32::new(0),
            xruns: AtomicU64::new(0),
            hardware_volume: AtomicBool::new(false),
            declick: AtomicBool::new(false),
            mode: Mutex::new("stopped".into()),
            error: Mutex::new(None),
            input_name: Mutex::new(None),
            output_name: Mutex::new(None),
            telemetry: Mutex::new(Telemetry::default()),
        }
    }

    pub fn info(&self) -> EngineInfo {
        EngineInfo {
            running: self.running.load(Ordering::Relaxed),
            mode: self.mode.lock().clone(),
            sample_rate: self.sample_rate.load(Ordering::Relaxed),
            input_device: self.input_name.lock().clone(),
            output_device: self.output_name.lock().clone(),
            buffer_ms: self.buffer_ms.load(Ordering::Relaxed),
            latency_ms: self.latency_us.load(Ordering::Relaxed) as f32 / 1_000.0,
            cpu_percent: self.cpu_permille.load(Ordering::Relaxed) as f32 / 10.0,
            xruns: self.xruns.load(Ordering::Relaxed),
            error: self.error.lock().clone(),
        }
    }

    pub fn telemetry(&self) -> Telemetry {
        self.telemetry.lock().clone()
    }

    fn set_error(&self, message: String) {
        log::error!("audio stream error: {message}");
        *self.error.lock() = Some(message);
        self.running.store(false, Ordering::Relaxed);
        self.xruns.fetch_add(1, Ordering::Relaxed);
        let mut telemetry = self.telemetry.lock();
        telemetry.running = false;
    }
}

pub struct EngineHandle {
    tx: crossbeam_channel::Sender<EngineCommand>,
    shared: Arc<EngineShared>,
    join: Mutex<Option<JoinHandle<()>>>,
}

enum EngineCommand {
    Start {
        request: EngineStart,
        reply: crossbeam_channel::Sender<Result<(), String>>,
    },
    Stop {
        reply: crossbeam_channel::Sender<()>,
    },
    Shutdown,
}

impl EngineHandle {
    pub fn spawn(params: ProcessingParams) -> Self {
        let shared = Arc::new(EngineShared::new(params.clone().sanitized()));
        let (tx, rx) = crossbeam_channel::bounded::<EngineCommand>(8);
        let thread_shared = Arc::clone(&shared);
        let join = thread::Builder::new()
            .name("audio-pulse-engine".into())
            .spawn(move || engine_loop(thread_shared, rx))
            .ok();
        Self {
            tx,
            shared,
            join: Mutex::new(join),
        }
    }

    pub fn shared(&self) -> &Arc<EngineShared> {
        &self.shared
    }

    pub fn info(&self) -> EngineInfo {
        self.shared.info()
    }

    pub fn set_params(&self, params: &ProcessingParams) {
        self.shared.declick.store(false, Ordering::Release);
        self.shared
            .params
            .store(Arc::new(params.clone().sanitized()));
    }

    /// A discrete change replaces the whole chain at once; the DSP fades
    /// around the swap so the waveform does not step.
    pub fn set_params_declick(&self, params: &ProcessingParams) {
        self.shared.declick.store(true, Ordering::Release);
        self.shared
            .params
            .store(Arc::new(params.clone().sanitized()));
    }

    pub fn set_hardware_volume(&self, enabled: bool) {
        self.shared
            .hardware_volume
            .store(enabled, Ordering::Relaxed);
    }

    pub fn start(&self, request: EngineStart) -> Result<(), String> {
        let (reply_tx, reply_rx) = crossbeam_channel::bounded(1);
        self.tx
            .send(EngineCommand::Start {
                request,
                reply: reply_tx,
            })
            .map_err(|_| "O motor de áudio não está disponível.".to_string())?;
        match reply_rx.recv_timeout(Duration::from_secs(15)) {
            Ok(result) => result,
            Err(_) => Err("O motor de áudio demorou demais para iniciar.".into()),
        }
    }

    pub fn stop(&self) {
        let (reply_tx, reply_rx) = crossbeam_channel::bounded(1);
        if self
            .tx
            .send(EngineCommand::Stop { reply: reply_tx })
            .is_ok()
        {
            let _ = reply_rx.recv_timeout(Duration::from_secs(3));
        }
    }

    pub fn shutdown(&self) {
        let _ = self.tx.send(EngineCommand::Shutdown);
        if let Some(handle) = self.join.lock().take() {
            let _ = handle.join();
        }
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        let _ = self.tx.send(EngineCommand::Shutdown);
    }
}

fn engine_loop(shared: Arc<EngineShared>, rx: crossbeam_channel::Receiver<EngineCommand>) {
    let mut engine = EngineThread {
        shared,
        streams: None,
    };
    while let Ok(command) = rx.recv() {
        match command {
            EngineCommand::Start { request, reply } => {
                let result = engine.start(request);
                let _ = reply.send(result);
            }
            EngineCommand::Stop { reply } => {
                engine.stop_internal();
                let _ = reply.send(());
            }
            EngineCommand::Shutdown => {
                engine.stop_internal();
                break;
            }
        }
    }
}

struct Streams {
    _input: Option<Stream>,
    _output: Stream,
}

struct EngineThread {
    shared: Arc<EngineShared>,
    streams: Option<Streams>,
}

impl EngineThread {
    fn stop_internal(&mut self) {
        // Dropping the streams stops the callbacks before the state is reset.
        self.streams.take();
        let shared = &self.shared;
        shared.running.store(false, Ordering::Relaxed);
        shared.sample_rate.store(0, Ordering::Relaxed);
        shared.latency_us.store(0, Ordering::Relaxed);
        shared.cpu_permille.store(0, Ordering::Relaxed);
        shared.fill_samples.store(0, Ordering::Relaxed);
        *shared.mode.lock() = "stopped".into();
        let mut telemetry = shared.telemetry.lock();
        telemetry.bands.fill(0.0);
        telemetry.rms_l = 0.0;
        telemetry.rms_r = 0.0;
        telemetry.peak_l = 0.0;
        telemetry.peak_r = 0.0;
        telemetry.compressor_gr_db = 0.0;
        telemetry.limiter_gr_db = 0.0;
        telemetry.adaptive_gains_db = [0.0; ADAPTIVE_EQ_BANDS];
        telemetry.cpu_percent = 0.0;
        telemetry.latency_ms = 0.0;
        telemetry.running = false;
        telemetry.sample_rate = 0;
    }

    fn start(&mut self, request: EngineStart) -> Result<(), String> {
        self.stop_internal();
        *self.shared.error.lock() = None;
        self.shared.xruns.store(0, Ordering::Relaxed);
        match request {
            EngineStart::Route {
                input,
                output,
                buffer_ms,
            } => self.start_route(input, output, buffer_ms),
            EngineStart::Preview {
                output,
                buffer_ms,
                source,
            } => self.start_preview(output, buffer_ms, source),
        }
    }

    fn finish_start(&mut self, streams: Streams, label: &'static str, latency_ms: f32) {
        let shared = &self.shared;
        shared.running.store(true, Ordering::Relaxed);
        shared
            .latency_us
            .store((latency_ms * 1_000.0) as u32, Ordering::Relaxed);
        *shared.mode.lock() = label.to_string();
        self.streams = Some(streams);
    }

    fn start_route(
        &mut self,
        input: Option<String>,
        output: Option<String>,
        buffer_ms: u32,
    ) -> Result<(), String> {
        let host = cpal::default_host();
        let (output_device, output_name) = resolve_output(&host, &output)?;
        let (input_device, input_name) = resolve_input(&host, &input)?;

        let supported = devices::choose_output_config(&output_device)?;
        let rate = supported.sample_rate().0;
        let out_format = supported.sample_format();
        let frames_request = frames_for(rate, buffer_ms);
        let mut config = supported.config();
        config.buffer_size = choose_buffer_size(supported.buffer_size(), frames_request);

        let input_supported = devices::choose_input_config(&input_device, rate)?;
        let input_rate = input_supported.sample_rate().0;
        let input_format = input_supported.sample_format();
        let input_config = input_supported.config();

        let target_samples = frames_request * 2;
        // One buffer is the floor left after each pop, the second absorbs a
        // late input callback or a short output stall. Without this slack a
        // single missed callback runs the ring dry and the gap clicks.
        let prime_samples = target_samples * 2;
        let high_water = target_samples * 4;
        let capacity = (target_samples * 8).max(8_192);
        let (producer, consumer) = HeapRb::<f32>::new(capacity).split();

        let shared = Arc::clone(&self.shared);
        let fill = make_route_fill(shared.clone(), consumer, rate, prime_samples, high_water);
        let output_stream = build_output_stream(
            &output_device,
            &config,
            out_format,
            frames_request + 2_048,
            fill,
            error_callback(shared.clone(), "saída"),
        )?;

        let shared_in = Arc::clone(&self.shared);
        let on_frames = make_route_input(shared_in, producer, input_rate, rate, prime_samples);
        let input_stream = build_input_stream(
            &input_device,
            &input_config,
            input_format,
            on_frames,
            error_callback(shared, "entrada"),
        )?;

        output_stream
            .play()
            .map_err(|e| format!("Falha ao iniciar a saída de áudio: {e}"))?;
        input_stream
            .play()
            .map_err(|e| format!("Falha ao iniciar a captura de áudio: {e}"))?;

        let latency = (frames_request as f32 * 3.0 / rate as f32) * 1_000.0 + LOOKAHEAD_MS;
        self.shared.buffer_ms.store(buffer_ms, Ordering::Relaxed);
        self.shared.sample_rate.store(rate, Ordering::Relaxed);
        *self.shared.input_name.lock() = Some(input_name);
        *self.shared.output_name.lock() = Some(output_name);
        self.shared.fill_samples.store(0, Ordering::Relaxed);
        self.finish_start(
            Streams {
                _input: Some(input_stream),
                _output: output_stream,
            },
            "route",
            latency,
        );
        Ok(())
    }

    fn start_preview(
        &mut self,
        output: Option<String>,
        buffer_ms: u32,
        source: PreviewSource,
    ) -> Result<(), String> {
        let host = cpal::default_host();
        let (output_device, output_name) = resolve_output(&host, &output)?;
        let supported = devices::choose_output_config(&output_device)?;
        let rate = supported.sample_rate().0;
        let format = supported.sample_format();
        let frames_request = frames_for(rate, buffer_ms);
        let mut config = supported.config();
        config.buffer_size = choose_buffer_size(supported.buffer_size(), frames_request);

        let preview = Preview::new(source.clone(), rate)?;
        let shared = Arc::clone(&self.shared);
        let fill = make_preview_fill(shared, preview, rate, frames_request);
        let output_stream = build_output_stream(
            &output_device,
            &config,
            format,
            frames_request + 2_048,
            fill,
            error_callback(Arc::clone(&self.shared), "saída"),
        )?;
        output_stream
            .play()
            .map_err(|e| format!("Falha ao iniciar a saída de áudio: {e}"))?;

        let latency = (frames_request as f32 / rate as f32) * 1_000.0 + LOOKAHEAD_MS;
        let label = match source {
            PreviewSource::Noise => "preview-noise",
            PreviewSource::Sweep => "preview-sweep",
            PreviewSource::Track(_) => "preview-wav",
        };
        self.shared.buffer_ms.store(buffer_ms, Ordering::Relaxed);
        self.shared.sample_rate.store(rate, Ordering::Relaxed);
        *self.shared.input_name.lock() = None;
        *self.shared.output_name.lock() = Some(output_name);
        self.finish_start(
            Streams {
                _input: None,
                _output: output_stream,
            },
            label,
            latency,
        );
        Ok(())
    }
}

fn frames_for(rate: u32, buffer_ms: u32) -> usize {
    ((rate as u64 * buffer_ms.max(1) as u64) / 1_000).clamp(MIN_FRAMES as u64, MAX_FRAMES as u64)
        as usize
}

fn choose_buffer_size(supported: &cpal::SupportedBufferSize, frames: usize) -> BufferSize {
    match supported {
        cpal::SupportedBufferSize::Range { min, max } if (*min..=*max).contains(&(frames as u32)) => {
            BufferSize::Fixed(frames as u32)
        }
        _ => BufferSize::Default,
    }
}

fn resolve_output(host: &cpal::Host, requested: &Option<String>) -> Result<(Device, String), String> {
    let (name, from_default) = match requested {
        Some(name) => (name.clone(), false),
        None => (
            host.default_output_device()
                .and_then(|d| d.name().ok())
                .ok_or_else(|| "Nenhum dispositivo de saída disponível.".to_string())?,
            true,
        ),
    };
    if from_default && devices::is_virtual_name(&name) {
        return Err(
            "A saída padrão do sistema é o cabo virtual. Selecione o fone como dispositivo \
             de saída em Ajustes → Áudio."
                .into(),
        );
    }
    let device = devices::find_output(host, &name)?;
    Ok((device, name))
}

fn resolve_input(host: &cpal::Host, requested: &Option<String>) -> Result<(Device, String), String> {
    let name = match requested {
        Some(name) => name.clone(),
        None => {
            let list = devices::enumerate();
            list.inputs
                .iter()
                .find(|d| d.is_virtual)
                .map(|d| d.name.clone())
                .ok_or_else(|| {
                    "Nenhum cabo virtual detectado. Clique em Instalar cabo virtual em \
                     Ajustes → Áudio e selecione o dispositivo de captura."
                        .to_string()
                })?
        }
    };
    let device = devices::find_input(host, &name)?;
    Ok((device, name))
}

type ErrorCallback = Arc<dyn Fn(String) + Send + Sync>;
type FrameSink = Box<dyn FnMut(&[f32]) + Send>;

fn error_callback(shared: Arc<EngineShared>, label: &'static str) -> ErrorCallback {
    Arc::new(move |message: String| {
        shared.set_error(format!("Fluxo de {label}: {message}"));
    })
}

type Fill = Box<dyn FnMut(&mut [f32]) + Send>;

fn build_output_stream(
    device: &Device,
    config: &StreamConfig,
    format: SampleFormat,
    initial_frames: usize,
    fill: Fill,
    on_error: ErrorCallback,
) -> Result<Stream, String> {
    match format {
        SampleFormat::F32 => build_output_typed::<f32>(device, config, initial_frames, fill, on_error),
        SampleFormat::I16 => build_output_typed::<i16>(device, config, initial_frames, fill, on_error),
        SampleFormat::U16 => build_output_typed::<u16>(device, config, initial_frames, fill, on_error),
        SampleFormat::I32 => build_output_typed::<i32>(device, config, initial_frames, fill, on_error),
        other => Err(format!("Formato de saída não suportado: {other:?}")),
    }
}

fn build_output_typed<T>(
    device: &Device,
    config: &StreamConfig,
    initial_frames: usize,
    mut fill: Fill,
    on_error: ErrorCallback,
) -> Result<Stream, String>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels.max(1) as usize;
    let mut stereo = vec![0.0f32; initial_frames.max(1) * 2];
    device
        .build_output_stream::<T, _, _>(
            config,
            move |data: &mut [T], _| {
                let frames = data.len() / channels;
                if frames == 0 {
                    return;
                }
                if stereo.len() < frames * 2 {
                    stereo.resize(frames * 2, 0.0);
                }
                let buffer = &mut stereo[..frames * 2];
                fill(buffer);
                for frame in 0..frames {
                    let left = buffer[frame * 2].clamp(-1.0, 1.0);
                    let right = buffer[frame * 2 + 1].clamp(-1.0, 1.0);
                    let base = frame * channels;
                    if channels == 1 {
                        data[base] = T::from_sample((left + right) * 0.5);
                    } else {
                        data[base] = T::from_sample(left);
                        data[base + 1] = T::from_sample(right);
                        for channel in 2..channels {
                            data[base + channel] = T::from_sample(0.0f32);
                        }
                    }
                }
            },
            move |err| on_error(err.to_string()),
            None,
        )
        .map_err(|e| format!("Falha ao abrir o fluxo de saída: {e}"))
}

fn build_input_stream(
    device: &Device,
    config: &StreamConfig,
    format: SampleFormat,
    on_frames: FrameSink,
    on_error: ErrorCallback,
) -> Result<Stream, String> {
    match format {
        SampleFormat::F32 => build_input_typed::<f32>(device, config, on_frames, on_error),
        SampleFormat::I16 => build_input_typed::<i16>(device, config, on_frames, on_error),
        SampleFormat::U16 => build_input_typed::<u16>(device, config, on_frames, on_error),
        SampleFormat::I32 => build_input_typed::<i32>(device, config, on_frames, on_error),
        other => Err(format!("Formato de captura não suportado: {other:?}")),
    }
}

fn build_input_typed<T>(
    device: &Device,
    config: &StreamConfig,
    mut on_frames: FrameSink,
    on_error: ErrorCallback,
) -> Result<Stream, String>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels.max(1) as usize;
    let mut scratch = vec![0.0f32; 8_192];
    device
        .build_input_stream::<T, _, _>(
            config,
            move |data: &[T], _| {
                let frames = data.len() / channels;
                if frames == 0 {
                    return;
                }
                if scratch.len() < frames * 2 {
                    scratch.resize(frames * 2, 0.0);
                }
                for frame in 0..frames {
                    let base = frame * channels;
                    let (left, right) = if channels == 1 {
                        let mono = f32::from_sample(data[base]);
                        (mono, mono)
                    } else {
                        (
                            f32::from_sample(data[base]),
                            f32::from_sample(data[base + 1]),
                        )
                    };
                    scratch[frame * 2] = left;
                    scratch[frame * 2 + 1] = right;
                }
                on_frames(&scratch[..frames * 2]);
            },
            move |err| on_error(err.to_string()),
            None,
        )
        .map_err(|e| format!("Falha ao abrir o fluxo de captura: {e}"))
}

fn make_route_fill(
    shared: Arc<EngineShared>,
    mut consumer: HeapCons<f32>,
    rate: u32,
    prime_samples: usize,
    high_water: usize,
) -> Fill {
    let mut processor = Processor::new(rate as f32);
    let mut last_params: Option<Arc<ProcessingParams>> = None;
    let mut primed = false;
    let mut cpu = 0.0f32;

    Box::new(move |out: &mut [f32]| {
        let started = Instant::now();
        if !primed {
            if consumer.occupied_len() >= prime_samples {
                primed = true;
            } else {
                out.fill(0.0);
                publish_idle(&shared, rate);
                return;
            }
        }

        let available = consumer.occupied_len();
        if available > high_water {
            // The input clock is ahead; drop the oldest audio to bound latency.
            consumer.skip(available - prime_samples);
            log::debug!(
                "ring overrun: {available} samples buffered, trimmed to {prime_samples}"
            );
            shared.xruns.fetch_add(1, Ordering::Relaxed);
        }

        let read = consumer.pop_slice(out);
        if read < out.len() {
            log::debug!("ring underrun: {read} of {} samples ready", out.len());
            out[read..].fill(0.0);
            shared.xruns.fetch_add(1, Ordering::Relaxed);
        }
        shared
            .fill_samples
            .store(consumer.occupied_len() as u32, Ordering::Relaxed);

        apply_params(&mut processor, &shared, &mut last_params);
        processor.process_interleaved(out);
        publish(&shared, &processor.analysis(), started, out.len() / 2, rate, &mut cpu);
    })
}

fn make_route_input(
    shared: Arc<EngineShared>,
    mut producer: HeapProd<f32>,
    input_rate: u32,
    output_rate: u32,
    prime_samples: usize,
) -> FrameSink {
    let base_step = input_rate as f64 / output_rate as f64;
    let mut frac = 0.0f64;
    let mut prev_l = 0.0f32;
    let mut prev_r = 0.0f32;
    let mut buffer = [0.0f32; 2_048];

    Box::new(move |frames: &[f32]| {
        let fill = shared.fill_samples.load(Ordering::Relaxed) as usize;
        // Nudge the conversion ratio to follow the other device's clock.
        let correction = if fill > prime_samples * 3 / 2 {
            1.0015
        } else if fill < prime_samples / 2 {
            0.9985
        } else {
            1.0
        };
        let step = base_step * correction;
        let mut used = 0usize;
        let mut dropped = false;

        for pair in frames.chunks_exact(2) {
            let current_l = pair[0];
            let current_r = pair[1];
            let mut t = frac;
            while t < 1.0 {
                let left = prev_l + (current_l - prev_l) * t as f32;
                let right = prev_r + (current_r - prev_r) * t as f32;
                if used + 2 > buffer.len() {
                    dropped |= flush_samples(&mut producer, &buffer, used);
                    used = 0;
                }
                buffer[used] = left;
                buffer[used + 1] = right;
                used += 2;
                t += step;
            }
            frac = t - 1.0;
            prev_l = current_l;
            prev_r = current_r;
        }
        dropped |= flush_samples(&mut producer, &buffer, used);

        if dropped {
            log::debug!("ring overflow on capture: push rejected");
            shared.xruns.fetch_add(1, Ordering::Relaxed);
        }
    })
}

/// Pushes `used` interleaved samples, returning `true` if the ring was full.
fn flush_samples(producer: &mut HeapProd<f32>, buffer: &[f32], used: usize) -> bool {
    if used == 0 {
        return false;
    }
    producer.push_slice(&buffer[..used]) < used
}

fn make_preview_fill(
    shared: Arc<EngineShared>,
    mut preview: Preview,
    rate: u32,
    _frames: usize,
) -> Fill {
    let mut processor = Processor::new(rate as f32);
    let mut last_params: Option<Arc<ProcessingParams>> = None;
    let mut cpu = 0.0f32;

    Box::new(move |out: &mut [f32]| {
        let started = Instant::now();
        preview.fill(out);
        apply_params(&mut processor, &shared, &mut last_params);
        processor.process_interleaved(out);
        publish(&shared, &processor.analysis(), started, out.len() / 2, rate, &mut cpu);
    })
}

fn apply_params(
    processor: &mut Processor,
    shared: &EngineShared,
    last: &mut Option<Arc<ProcessingParams>>,
) {
    let params = shared.params.load_full();
    let changed = last
        .as_ref()
        .map(|previous| !Arc::ptr_eq(previous, &params))
        .unwrap_or(true);
    if changed {
        if shared.declick.swap(false, Ordering::AcqRel) {
            processor.set_params_declick(&params);
        } else {
            processor.set_params(&params);
        }
        *last = Some(params);
    }
    processor.set_master_volume_enabled(!shared.hardware_volume.load(Ordering::Relaxed));
}

fn publish_idle(shared: &EngineShared, rate: u32) {
    let mut telemetry = shared.telemetry.lock();
    telemetry.bands.fill(0.0);
    telemetry.rms_l = 0.0;
    telemetry.rms_r = 0.0;
    telemetry.peak_l = 0.0;
    telemetry.peak_r = 0.0;
    telemetry.compressor_gr_db = 0.0;
    telemetry.limiter_gr_db = 0.0;
    telemetry.adaptive_gains_db = [0.0; ADAPTIVE_EQ_BANDS];
    telemetry.cpu_percent = shared.cpu_permille.load(Ordering::Relaxed) as f32 / 10.0;
    telemetry.latency_ms = shared.latency_us.load(Ordering::Relaxed) as f32 / 1_000.0;
    telemetry.xruns = shared.xruns.load(Ordering::Relaxed);
    telemetry.running = shared.running.load(Ordering::Relaxed);
    telemetry.sample_rate = rate;
}

fn publish(
    shared: &EngineShared,
    analysis: &AnalysisSnapshot,
    started: Instant,
    frames: usize,
    rate: u32,
    cpu: &mut f32,
) {
    let buffer_seconds = frames as f32 / rate as f32;
    let load = started.elapsed().as_secs_f32() / buffer_seconds.max(1e-6);
    *cpu = *cpu * 0.9 + load * 0.1;
    shared
        .cpu_permille
        .store((cpu.clamp(0.0, 10.0) * 1_000.0) as u32, Ordering::Relaxed);

    let mut telemetry = shared.telemetry.lock();
    telemetry.bands.copy_from_slice(&analysis.bands[..SPECTRUM_BANDS]);
    telemetry.rms_l = analysis.rms_l;
    telemetry.rms_r = analysis.rms_r;
    telemetry.peak_l = analysis.peak_l;
    telemetry.peak_r = analysis.peak_r;
    telemetry.compressor_gr_db = analysis.compressor_gr_db;
    telemetry.limiter_gr_db = analysis.limiter_gr_db;
    telemetry.adaptive_gains_db = analysis.adaptive_gains_db;
    telemetry.cpu_percent = (*cpu * 100.0).min(1000.0);
    telemetry.latency_ms = shared.latency_us.load(Ordering::Relaxed) as f32 / 1_000.0;
    telemetry.xruns = shared.xruns.load(Ordering::Relaxed);
    telemetry.running = shared.running.load(Ordering::Relaxed);
    telemetry.sample_rate = rate;
}

// ---------------------------------------------------------------------------
// Preview sources
// ---------------------------------------------------------------------------

enum Preview {
    Noise {
        left: PinkNoise,
        right: PinkNoise,
    },
    Sweep {
        phase: f32,
        time: f32,
        sample_rate: f32,
    },
    Track {
        data: Arc<Vec<f32>>,
        position: usize,
    },
}

impl Preview {
    fn new(source: PreviewSource, rate: u32) -> Result<Self, String> {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0x5eed_1234);
        Ok(match source {
            PreviewSource::Noise => Preview::Noise {
                left: PinkNoise::new(seed | 1),
                right: PinkNoise::new(seed.rotate_left(7) | 1),
            },
            PreviewSource::Sweep => Preview::Sweep {
                phase: 0.0,
                time: 0.0,
                sample_rate: rate as f32,
            },
            PreviewSource::Track(path) => Preview::Track {
                data: Arc::new(load_track(&path, rate)?),
                position: 0,
            },
        })
    }

    fn fill(&mut self, out: &mut [f32]) {
        match self {
            Preview::Noise { left, right } => {
                for frame in out.chunks_exact_mut(2) {
                    frame[0] = left.next() * 0.18;
                    frame[1] = right.next() * 0.18;
                }
            }
            Preview::Sweep {
                phase,
                time,
                sample_rate,
            } => {
                const DURATION: f32 = 8.0;
                const FADE: f32 = 0.15;
                for frame in out.chunks_exact_mut(2) {
                    let progress = *time / DURATION;
                    let freq = 20.0 * 1_000.0f32.powf(progress);
                    *phase = (*phase + TAU as f32 * freq / *sample_rate) % (TAU as f32);
                    let envelope = (*time / FADE).min((DURATION - *time) / FADE).clamp(0.0, 1.0);
                    let sample = phase.sin() * 0.3 * envelope;
                    frame[0] = sample;
                    frame[1] = sample;
                    *time += 1.0 / *sample_rate;
                    if *time >= DURATION {
                        *time = 0.0;
                    }
                }
            }
            Preview::Track { data, position } => {
                for frame in out.chunks_exact_mut(2) {
                    if data.len() < 2 {
                        frame[0] = 0.0;
                        frame[1] = 0.0;
                        continue;
                    }
                    frame[0] = data[*position] * 0.9;
                    frame[1] = data[*position + 1] * 0.9;
                    *position += 2;
                    if *position >= data.len() {
                        *position = 0;
                    }
                }
            }
        }
    }
}

struct PinkNoise {
    rng: Rng,
    b0: f32,
    b1: f32,
    b2: f32,
    b3: f32,
    b4: f32,
    b5: f32,
    b6: f32,
}

impl PinkNoise {
    fn new(seed: u32) -> Self {
        Self {
            rng: Rng::new(seed),
            b0: 0.0,
            b1: 0.0,
            b2: 0.0,
            b3: 0.0,
            b4: 0.0,
            b5: 0.0,
            b6: 0.0,
        }
    }

    fn next(&mut self) -> f32 {
        let white = self.rng.next();
        self.b0 = 0.99886 * self.b0 + white * 0.0555179;
        self.b1 = 0.99332 * self.b1 + white * 0.0750759;
        self.b2 = 0.96900 * self.b2 + white * 0.153852;
        self.b3 = 0.86650 * self.b3 + white * 0.3104856;
        self.b4 = 0.55000 * self.b4 + white * 0.5329522;
        self.b5 = -0.7616 * self.b5 - white * 0.0168980;
        let pink = self.b0 + self.b1 + self.b2 + self.b3 + self.b4 + self.b5 + self.b6
            + white * 0.5362;
        self.b6 = white * 0.115926;
        pink * 0.11
    }
}

struct Rng(u32);

impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed.max(1))
    }

    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn load_track(path: &str, target_rate: u32) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| format!("Não foi possível abrir o arquivo WAV: {e}"))?;
    let spec = reader.spec();
    if spec.channels == 0 {
        return Err("O arquivo WAV não contém canais.".into());
    }
    let channels = spec.channels as usize;
    let scale = if spec.bits_per_sample == 0 {
        1.0
    } else {
        1.0 / (1u64 << (spec.bits_per_sample.min(32) - 1)) as f32
    };

    let mut interleaved: Vec<f32> = Vec::new();
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for sample in reader.samples::<f32>() {
                interleaved.push(sample.map_err(|e| format!("Falha ao decodificar o WAV: {e}"))?);
            }
        }
        hound::SampleFormat::Int => {
            for sample in reader.samples::<i32>() {
                let value = sample.map_err(|e| format!("Falha ao decodificar o WAV: {e}"))?;
                interleaved.push(value as f32 * scale);
            }
        }
    }
    let frames = interleaved.len() / channels;
    if frames == 0 {
        return Err("O arquivo WAV está vazio.".into());
    }

    let mut stereo = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        let base = frame * channels;
        let left = interleaved[base];
        let right = if channels > 1 {
            interleaved[base + 1]
        } else {
            left
        };
        stereo.push(left);
        stereo.push(right);
    }

    Ok(resample_stereo(&stereo, spec.sample_rate, target_rate))
}

fn resample_stereo(input: &[f32], source_rate: u32, target_rate: u32) -> Vec<f32> {
    let source_frames = input.len() / 2;
    if source_frames == 0 {
        return Vec::new();
    }
    if source_rate == target_rate || source_rate == 0 {
        return input.to_vec();
    }
    let ratio = source_rate as f64 / target_rate as f64;
    let target_frames = ((source_frames as f64) / ratio).floor().max(1.0) as usize;
    let mut output = Vec::with_capacity(target_frames * 2);
    for frame in 0..target_frames {
        let position = frame as f64 * ratio;
        let index = position.floor() as usize;
        let frac = (position - index as f64) as f32;
        let next = (index + 1).min(source_frames - 1);
        for channel in 0..2 {
            let a = input[index * 2 + channel];
            let b = input[next * 2 + channel];
            output.push(a + (b - a) * frac);
        }
    }
    output
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_frames_are_bounded() {
        assert_eq!(frames_for(48_000, 25), 1_200);
        assert_eq!(frames_for(48_000, 0), MIN_FRAMES);
        assert_eq!(frames_for(384_000, 200), MAX_FRAMES);
    }

    #[test]
    fn resampling_preserves_duration() {
        let frames = 480;
        let input: Vec<f32> = (0..frames)
            .flat_map(|i| {
                let x = (i as f32 / 48.0).sin();
                [x, -x]
            })
            .collect();
        let output = resample_stereo(&input, 48_000, 44_100);
        let output_frames = output.len() / 2;
        assert!((output_frames as i64 - 441).abs() <= 2, "{output_frames}");
        assert!(output.iter().all(|x| x.is_finite() && x.abs() <= 1.0));
    }

    #[test]
    fn pink_noise_is_bounded_and_decorrelated() {
        let mut left = PinkNoise::new(1);
        let mut right = PinkNoise::new(2);
        let mut peak = 0.0f32;
        let mut equal = 0;
        for _ in 0..10_000 {
            let l = left.next();
            let r = right.next();
            peak = peak.max(l.abs()).max(r.abs());
            if (l - r).abs() < 1e-6 {
                equal += 1;
            }
        }
        assert!(peak < 1.0);
        assert!(equal < 10);
    }
}
