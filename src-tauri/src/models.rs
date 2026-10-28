//! Data structures exchanged between the Rust backend and the webview.

use serde::{Deserialize, Serialize};
use audio_pulse_dsp::presets::Preset;
use audio_pulse_dsp::ProcessingParams;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// BCP-47 tag. Empty means "detect from the webview locale".
    pub language: String,
    pub theme: String,
    pub autostart: bool,
    pub close_to_tray: bool,
    pub launch_minimized: bool,
    pub buffer_ms: u32,
    pub device_profiles_enabled: bool,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub shortcuts: BTreeMap<String, String>,
    pub device_profiles: BTreeMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        let mut shortcuts = BTreeMap::new();
        shortcuts.insert("toggle_bypass".into(), "CmdOrCtrl+Alt+B".into());
        shortcuts.insert("preset_music".into(), "CmdOrCtrl+Alt+1".into());
        shortcuts.insert("preset_cinema".into(), "CmdOrCtrl+Alt+2".into());
        shortcuts.insert("preset_gaming".into(), "CmdOrCtrl+Alt+3".into());
        shortcuts.insert("preset_podcast".into(), "CmdOrCtrl+Alt+4".into());
        shortcuts.insert("toggle_spatial".into(), "CmdOrCtrl+Alt+S".into());
        shortcuts.insert("volume_up".into(), "CmdOrCtrl+Alt+Up".into());
        shortcuts.insert("volume_down".into(), "CmdOrCtrl+Alt+Down".into());
        shortcuts.insert("open_window".into(), "CmdOrCtrl+Alt+O".into());
        Self {
            language: String::new(),
            theme: "dark".into(),
            autostart: false,
            close_to_tray: true,
            launch_minimized: false,
            buffer_ms: 25,
            device_profiles_enabled: true,
            input_device: None,
            output_device: None,
            shortcuts,
            device_profiles: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn sanitized(mut self) -> Self {
        if !matches!(self.language.as_str(), "" | "pt-BR" | "en") {
            self.language = String::new();
        }
        if !matches!(self.theme.as_str(), "dark" | "light") {
            self.theme = "dark".into();
        }
        self.buffer_ms = self.buffer_ms.clamp(10, 200);
        self
    }

    /// Language used by Rust-drawn UI (tray). The webview resolves empty to the
    /// actual locale, while this fallback covers the tray before it does.
    pub fn resolved_language(&self) -> &'static str {
        match self.language.as_str() {
            "pt-BR" => "pt-BR",
            "en" => "en",
            _ => {
                let lower = std::env::var("LANG")
                    .or_else(|_| std::env::var("LC_ALL"))
                    .unwrap_or_default()
                    .to_lowercase();
                if lower.starts_with("pt") {
                    "pt-BR"
                } else {
                    "en"
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub is_virtual: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceList {
    pub inputs: Vec<DeviceInfo>,
    pub outputs: Vec<DeviceInfo>,
    pub default_input: Option<String>,
    pub default_output: Option<String>,
    pub virtual_cable_detected: bool,
    pub platform: String,
}

impl DeviceList {
    pub fn signature(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for d in &self.inputs {
            parts.push(format!("i:{}:{}", d.name, d.is_default));
        }
        for d in &self.outputs {
            parts.push(format!("o:{}:{}", d.name, d.is_default));
        }
        parts.join("|")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EngineInfo {
    pub running: bool,
    pub mode: String,
    pub sample_rate: u32,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub buffer_ms: u32,
    pub latency_ms: f32,
    pub cpu_percent: f32,
    pub xruns: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParamsUpdate {
    pub params: ProcessingParams,
    pub active_preset_id: Option<String>,
}

/// Battery of the wireless headset that owns the selected output endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatteryInfo {
    pub percent: u8,
    pub device: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppView {
    pub params: ProcessingParams,
    pub settings: Settings,
    pub presets: Vec<Preset>,
    pub active_preset_id: Option<String>,
    pub devices: DeviceList,
    pub engine: EngineInfo,
    pub battery: Option<BatteryInfo>,
    pub warnings: Vec<String>,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Telemetry {
    pub bands: Vec<f32>,
    pub rms_l: f32,
    pub rms_r: f32,
    pub peak_l: f32,
    pub peak_r: f32,
    pub compressor_gr_db: f32,
    pub limiter_gr_db: f32,
    pub adaptive_gains_db: [f32; audio_pulse_dsp::ADAPTIVE_EQ_BANDS],
    pub cpu_percent: f32,
    pub latency_ms: f32,
    pub xruns: u64,
    pub running: bool,
    pub sample_rate: u32,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            bands: vec![0.0; audio_pulse_dsp::SPECTRUM_BANDS],
            rms_l: 0.0,
            rms_r: 0.0,
            peak_l: 0.0,
            peak_r: 0.0,
            compressor_gr_db: 0.0,
            limiter_gr_db: 0.0,
            adaptive_gains_db: [0.0; audio_pulse_dsp::ADAPTIVE_EQ_BANDS],
            cpu_percent: 0.0,
            latency_ms: 0.0,
            xruns: 0,
            running: false,
            sample_rate: 0,
        }
    }
}