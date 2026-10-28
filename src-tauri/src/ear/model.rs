//! Cached download of the local depth model (Depth Anything V2 Small, int8).

use crate::config::config_path;
use parking_lot::Mutex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub const MODEL_FILE: &str = "depth-anything-v2-small-int8.onnx";
pub const MODEL_URL: &str =
    "https://huggingface.co/onnx-community/depth-anything-v2-small/resolve/main/onnx/model_int8.onnx";
pub const MODEL_SHA256: &str = "01aa7a23de3f4a0ee1a2bb9997e6918104c85a9f95dea46d27b9b3fb0c6b9001";
pub const MODEL_BYTES: u64 = 27_258_801;
pub const MODEL_NAME: &str = "Depth Anything V2 Small";
pub const MODEL_LICENSE: &str = "Apache-2.0";

static DOWNLOADING: Mutex<bool> = Mutex::new(false);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub installed: bool,
    pub bytes: u64,
    pub expected_bytes: u64,
    pub name: String,
    pub license: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelProgress {
    pub received: u64,
    pub total: u64,
}

pub fn models_dir() -> PathBuf {
    config_path()
        .parent()
        .map(|dir| dir.join("models"))
        .unwrap_or_else(|| PathBuf::from("models"))
}

pub fn model_path() -> PathBuf {
    models_dir().join(MODEL_FILE)
}

pub fn status() -> ModelStatus {
    let bytes = fs::metadata(model_path()).map(|meta| meta.len()).unwrap_or(0);
    ModelStatus {
        installed: bytes == MODEL_BYTES,
        bytes,
        expected_bytes: MODEL_BYTES,
        name: MODEL_NAME.into(),
        license: MODEL_LICENSE.into(),
    }
}

/// Downloads and verifies the model. `progress` is called every ~2 MB with
/// (received, total). The partial file never replaces a working model.
pub fn download_with(mut progress: impl FnMut(u64, u64)) -> Result<ModelStatus, String> {
    {
        let mut busy = DOWNLOADING.lock();
        if *busy {
            return Err("O download do modelo já está em andamento.".into());
        }
        *busy = true;
    }
    let _guard = DownloadGuard;

    let current = status();
    if current.installed {
        return Ok(current);
    }

    let dir = models_dir();
    fs::create_dir_all(&dir)
        .map_err(|err| format!("Não foi possível criar a pasta de modelos: {err}"))?;
    let dest = model_path();
    let part = dir.join(format!("{MODEL_FILE}.part"));

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(900))
        .user_agent(concat!("AudioPulse/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|err| format!("Falha ao preparar o download: {err}"))?;

    let mut response = client
        .get(MODEL_URL)
        .send()
        .map_err(|err| format!("Não foi possível baixar o modelo: {err}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "O servidor do modelo respondeu HTTP {}.",
            response.status()
        ));
    }

    let total = response.content_length().unwrap_or(MODEL_BYTES);
    progress(0, total);
    let mut file = File::create(&part)
        .map_err(|err| format!("Não foi possível gravar o modelo: {err}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut received = 0u64;
    let mut reported = 0u64;
    loop {
        let read = response
            .read(&mut buffer)
            .map_err(|err| format!("O download foi interrompido: {err}"))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|err| format!("Não foi possível gravar o modelo: {err}"))?;
        hasher.update(&buffer[..read]);
        received += read as u64;
        if received - reported >= 2 * 1024 * 1024 {
            reported = received;
            progress(received, total);
        }
    }
    file.flush()
        .map_err(|err| format!("Não foi possível gravar o modelo: {err}"))?;
    drop(file);

    if received != MODEL_BYTES {
        let _ = fs::remove_file(&part);
        return Err(format!(
            "O download terminou incompleto ({received} de {MODEL_BYTES} bytes)."
        ));
    }
    let digest = format!("{:x}", hasher.finalize());
    if digest != MODEL_SHA256 {
        let _ = fs::remove_file(&part);
        return Err("O modelo baixado não confere com a assinatura esperada.".into());
    }
    fs::rename(&part, &dest)
        .map_err(|err| format!("Não foi possível concluir o download: {err}"))?;
    progress(received, total);
    log::info!("depth model installed at {}", dest.display());
    Ok(status())
}

pub fn download(app: &AppHandle) -> Result<ModelStatus, String> {
    download_with(|received, total| {
        let _ = app.emit("ear-model-progress", ModelProgress { received, total });
    })
}

struct DownloadGuard;

impl Drop for DownloadGuard {
    fn drop(&mut self) {
        *DOWNLOADING.lock() = false;
    }
}
