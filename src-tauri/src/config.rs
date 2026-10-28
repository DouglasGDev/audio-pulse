//! Persistent configuration with debounced, atomic JSON writes.

use crate::models::Settings;
use crossbeam_channel::{bounded, Sender};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use audio_pulse_dsp::presets::Preset;
use audio_pulse_dsp::ProcessingParams;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PersistedConfig {
    pub version: u32,
    pub params: ProcessingParams,
    pub settings: Settings,
    pub custom_presets: Vec<Preset>,
    pub active_preset_id: Option<String>,
    /// One-time marker: the master was already migrated onto the Windows
    /// endpoint, so the DSP no longer applies its own master gain.
    #[serde(default)]
    pub volume_sync_initialized: bool,
}

impl Default for PersistedConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            params: ProcessingParams::default(),
            settings: Settings::default(),
            custom_presets: Vec::new(),
            active_preset_id: None,
            volume_sync_initialized: false,
        }
    }
}

pub struct ConfigStore {
    path: PathBuf,
    data: Arc<Mutex<PersistedConfig>>,
    save_tx: Sender<()>,
}

pub struct LoadedConfig {
    pub config: PersistedConfig,
    pub warnings: Vec<String>,
}

impl ConfigStore {
    pub fn load() -> LoadedConfig {
        migrate_legacy_data();
        let path = config_path();
        let mut warnings = Vec::new();
        let config = match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<PersistedConfig>(&text) {
                Ok(mut cfg) => {
                    cfg.version = CONFIG_VERSION;
                    cfg.params = cfg.params.sanitized();
                    cfg.settings = cfg.settings.clone().sanitized();
                    cfg
                }
                Err(err) => {
                    warnings.push(format!(
                        "O arquivo de configuração não pôde ser lido ({err}). Um backup foi criado."
                    ));
                    backup_corrupt(&path);
                    PersistedConfig::default()
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => PersistedConfig::default(),
            Err(err) => {
                warnings.push(format!("Falha ao abrir a configuração: {err}"));
                PersistedConfig::default()
            }
        };
        let store = Self::new(path, config);
        LoadedConfig {
            config: store.snapshot(),
            warnings,
        }
    }

    pub fn from_loaded(loaded: LoadedConfig) -> (Arc<Self>, Vec<String>) {
        let store = Self::new(config_path(), loaded.config);
        (Arc::new(store), loaded.warnings)
    }

    fn new(path: PathBuf, config: PersistedConfig) -> Self {
        let data = Arc::new(Mutex::new(config));
        let (save_tx, save_rx) = bounded::<()>(1);
        let thread_data = Arc::clone(&data);
        let thread_path = path.clone();
        thread::Builder::new()
            .name("audio-pulse-config-saver".into())
            .spawn(move || {
                while save_rx.recv().is_ok() {
                    // Coalesce bursts (slider drags, volume changes).
                    while save_rx
                        .recv_timeout(Duration::from_millis(350))
                        .is_ok()
                    {}
                    let snapshot = thread_data.lock().clone();
                    if let Err(err) = write_atomic(&thread_path, &snapshot) {
                        log::error!("failed to save config: {err}");
                    }
                }
            })
            .ok();
        Self {
            path,
            data,
            save_tx,
        }
    }

    pub fn snapshot(&self) -> PersistedConfig {
        self.data.lock().clone()
    }

    pub fn mutate<R>(&self, f: impl FnOnce(&mut PersistedConfig) -> R) -> R {
        let result = {
            let mut guard = self.data.lock();
            f(&mut guard)
        };
        self.request_save();
        result
    }

    pub fn request_save(&self) {
        let _ = self.save_tx.try_send(());
    }

    pub fn save_now(&self) {
        let snapshot = self.snapshot();
        if let Err(err) = write_atomic(&self.path, &snapshot) {
            log::error!("failed to save config immediately: {err}");
        }
    }
}

const APP_DIR: &str = "AudioPulse";
const LEGACY_APP_DIR: &str = "SonicEnhance";

fn config_base() -> PathBuf {
    dirs::config_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .unwrap_or_else(std::env::temp_dir)
}

pub fn config_path() -> PathBuf {
    config_base().join(APP_DIR).join("config.json")
}

fn migrate_legacy_data() {
    let base = config_base();
    migrate_from(&base.join(LEGACY_APP_DIR), &base.join(APP_DIR));
}

/// Copies settings, the route record and the model cache from the folder used
/// before the Audio Pulse rename. The old folder is left intact, so switching
/// back to an older build keeps working.
fn migrate_from(legacy: &Path, current: &Path) -> bool {
    if current.join("config.json").exists() || !legacy.join("config.json").is_file() {
        return false;
    }
    if let Err(err) = fs::create_dir_all(current) {
        log::warn!("could not create the config folder: {err}");
        return false;
    }
    let mut migrated_config = false;
    for name in ["config.json", "route-default.json"] {
        let source = legacy.join(name);
        if !source.is_file() {
            continue;
        }
        match fs::copy(&source, current.join(name)) {
            Ok(_) => migrated_config |= name == "config.json",
            Err(err) => log::warn!("could not migrate {name}: {err}"),
        }
    }
    let models = legacy.join("models");
    if let Ok(entries) = fs::read_dir(&models) {
        let target = current.join("models");
        if let Err(err) = fs::create_dir_all(&target) {
            log::warn!("could not create the model folder: {err}");
        } else {
            for entry in entries.flatten() {
                if entry.path().is_file() {
                    let _ = fs::copy(entry.path(), target.join(entry.file_name()));
                }
            }
        }
    }
    if migrated_config {
        log::info!("settings migrated from {LEGACY_APP_DIR} to {APP_DIR}");
    }
    migrated_config
}

fn backup_corrupt(path: &PathBuf) {
    if !path.exists() {
        return;
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup = path.with_file_name(format!("config.corrupt-{stamp}.json"));
    if let Err(err) = fs::rename(path, &backup) {
        log::warn!("could not back up corrupt config: {err}");
    }
}

fn write_atomic(path: &PathBuf, config: &PersistedConfig) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "caminho de configuração inválido".to_string())?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    // std::fs::rename replaces an existing file on Windows and POSIX as long
    // as both paths are on the same volume.
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_state_once() {
        let base = std::env::temp_dir().join(format!("audio-pulse-migrate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let legacy = base.join("SonicEnhance");
        let current = base.join("AudioPulse");
        fs::create_dir_all(legacy.join("models")).unwrap();
        fs::write(legacy.join("config.json"), "{\"version\":1}").unwrap();
        fs::write(legacy.join("route-default.json"), "{}").unwrap();
        fs::write(legacy.join("models").join("model.onnx"), b"weights").unwrap();

        assert!(migrate_from(&legacy, &current));
        assert_eq!(
            fs::read_to_string(current.join("config.json")).unwrap(),
            "{\"version\":1}"
        );
        assert!(current.join("route-default.json").is_file());
        assert_eq!(
            fs::read(current.join("models").join("model.onnx")).unwrap(),
            b"weights"
        );

        fs::write(current.join("config.json"), "{\"current\":true}").unwrap();
        assert!(!migrate_from(&legacy, &current));
        assert!(fs::read_to_string(current.join("config.json"))
            .unwrap()
            .contains("current"));

        let _ = fs::remove_dir_all(&base);
    }
}