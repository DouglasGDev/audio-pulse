//! Audio device enumeration and virtual-cable detection.

use crate::models::{DeviceInfo, DeviceList};
use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Host};
use std::collections::HashSet;

pub fn enumerate() -> DeviceList {
    let host = cpal::default_host();
    enumerate_host(&host)
}

fn name_of(device: &Device) -> Option<String> {
    device.name().ok().filter(|n| !n.trim().is_empty())
}

fn enumerate_host(host: &Host) -> DeviceList {
    let default_input = host.default_input_device().and_then(|d| name_of(&d));
    let default_output = host.default_output_device().and_then(|d| name_of(&d));

    let mut inputs = Vec::new();
    let mut outputs = Vec::new();
    let mut seen_in = HashSet::new();
    let mut seen_out = HashSet::new();

    if let Ok(devices) = host.input_devices() {
        for device in devices {
            if let Some(name) = name_of(&device) {
                if seen_in.insert(name.clone()) {
                    inputs.push(DeviceInfo {
                        is_default: default_input.as_deref() == Some(name.as_str()),
                        is_virtual: is_virtual_name(&name),
                        name,
                    });
                }
            }
        }
    }
    if let Ok(devices) = host.output_devices() {
        for device in devices {
            if let Some(name) = name_of(&device) {
                if seen_out.insert(name.clone()) {
                    outputs.push(DeviceInfo {
                        is_default: default_output.as_deref() == Some(name.as_str()),
                        is_virtual: is_virtual_name(&name),
                        name,
                    });
                }
            }
        }
    }

    sort_devices(&mut inputs);
    sort_devices(&mut outputs);
    let virtual_cable_detected = inputs.iter().any(|d| d.is_virtual);

    DeviceList {
        inputs,
        outputs,
        default_input,
        default_output,
        virtual_cable_detected,
        platform: std::env::consts::OS.to_string(),
    }
}

fn sort_devices(devices: &mut [DeviceInfo]) {
    devices.sort_by(|a, b| {
        b.is_default
            .cmp(&a.is_default)
            .then_with(|| b.is_virtual.cmp(&a.is_virtual))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

pub fn is_virtual_name(name: &str) -> bool {
    let name = name.to_lowercase();
    [
        "cable output",
        "cable input",
        "vb-audio",
        "voicemeeter",
        "blackhole",
        "soundflower",
        "loopback",
        "monitor of",
        "virtual audio",
        "virtual cable",
        "pulseaudio",
        "pipewire",
        "sound source",
        "null sink",
    ]
    .iter()
    .any(|needle| name.contains(needle))
}

pub fn find_input(host: &Host, name: &str) -> Result<Device, String> {
    host.input_devices()
        .map_err(|e| e.to_string())?
        .find(|d| name_of(d).as_deref() == Some(name))
        .ok_or_else(|| format!("Dispositivo de captura não encontrado: {name}"))
}

pub fn find_output(host: &Host, name: &str) -> Result<Device, String> {
    host.output_devices()
        .map_err(|e| e.to_string())?
        .find(|d| name_of(d).as_deref() == Some(name))
        .ok_or_else(|| format!("Dispositivo de saída não encontrado: {name}"))
}

/// Pick a practical output configuration, preferring stereo 48 kHz f32.
pub fn choose_output_config(device: &Device) -> Result<cpal::SupportedStreamConfig, String> {
    let default = device
        .default_output_config()
        .map_err(|e| format!("Configuração de saída indisponível: {e}"))?;

    let mut best: Option<(cpal::SupportedStreamConfig, i32)> = None;
    if let Ok(ranges) = device.supported_output_configs() {
        for range in ranges {
            let format = range.sample_format();
            if !is_supported_format(format) {
                continue;
            }
            let channels = range.channels();
            let min = range.min_sample_rate().0;
            let max = range.max_sample_rate().0;
            let mut score = 0;
            if channels >= 2 {
                score += 40;
            }
            if channels == 2 {
                score += 20;
            }
            if format == cpal::SampleFormat::F32 {
                score += 50;
            }
            let rate = if (min..=max).contains(&48_000) {
                score += 30;
                48_000
            } else if (min..=max).contains(&44_100) {
                score += 20;
                44_100
            } else if (min..=max).contains(&default.sample_rate().0) {
                default.sample_rate().0
            } else {
                min.max(44_100)
            };
            let chosen_channels = if channels >= 2 { 2 } else { channels };
            let cfg = range.with_sample_rate(cpal::SampleRate(rate));
            let supported = cpal::SupportedStreamConfig::new(
                chosen_channels,
                cfg.sample_rate(),
                *cfg.buffer_size(),
                cfg.sample_format(),
            );
            if best.as_ref().map(|(_, s)| score > *s).unwrap_or(true) {
                best = Some((supported, score));
            }
        }
    }

    let chosen = best
        .map(|(cfg, _)| cfg)
        .unwrap_or_else(|| default.clone());
    let format = chosen.sample_format();
    if !is_supported_format(format) {
        return Err(format!(
            "O dispositivo usa um formato de amostra não suportado ({format:?})."
        ));
    }
    Ok(chosen)
}

/// Pick an input configuration, preferring the engine rate and stereo f32.
pub fn choose_input_config(
    device: &Device,
    preferred_rate: u32,
) -> Result<cpal::SupportedStreamConfig, String> {
    let default = device
        .default_input_config()
        .map_err(|e| format!("Configuração de entrada indisponível: {e}"))?;

    let mut best: Option<(cpal::SupportedStreamConfig, i32)> = None;
    if let Ok(ranges) = device.supported_input_configs() {
        for range in ranges {
            let format = range.sample_format();
            if !is_supported_format(format) {
                continue;
            }
            let channels = range.channels();
            let min = range.min_sample_rate().0;
            let max = range.max_sample_rate().0;
            let mut score = 0;
            if format == cpal::SampleFormat::F32 {
                score += 40;
            }
            if channels >= 2 {
                score += 25;
            }
            if (min..=max).contains(&preferred_rate) {
                score += 30;
            }
            if (min..=max).contains(&48_000) {
                score += 10;
            }
            let rate = if (min..=max).contains(&preferred_rate) {
                preferred_rate
            } else if (min..=max).contains(&48_000) {
                48_000
            } else if (min..=max).contains(&44_100) {
                44_100
            } else {
                default.sample_rate().0.clamp(min, max)
            };
            let chosen_channels = if channels >= 2 { 2 } else { channels };
            let cfg = range.with_sample_rate(cpal::SampleRate(rate));
            let supported = cpal::SupportedStreamConfig::new(
                chosen_channels,
                cfg.sample_rate(),
                *cfg.buffer_size(),
                cfg.sample_format(),
            );
            if best
                .as_ref()
                .map(|(_, s)| score > *s)
                .unwrap_or(true)
            {
                best = Some((supported, score));
            }
        }
    }

    if let Some((config, _)) = best {
        return Ok(config);
    }
    let format = default.sample_format();
    if !is_supported_format(format) {
        return Err(format!(
            "O dispositivo usa um formato de amostra não suportado ({format:?})."
        ));
    }
    Ok(default)
}

pub fn is_supported_format(format: cpal::SampleFormat) -> bool {
    matches!(
        format,
        cpal::SampleFormat::F32
            | cpal::SampleFormat::I16
            | cpal::SampleFormat::U16
            | cpal::SampleFormat::I32
    )
}