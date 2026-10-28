//! Tauri commands for the local ear-photo profile.

use tauri::AppHandle;

use super::model::{self, ModelStatus};
use super::pipeline::{self, EarAnalysis, EarPhoto};

#[tauri::command]
pub fn ear_model_status() -> ModelStatus {
    model::status()
}

#[tauri::command]
pub async fn ear_download_model(app: AppHandle) -> Result<ModelStatus, String> {
    tauri::async_runtime::spawn_blocking(move || model::download(&app))
        .await
        .map_err(|err| format!("Falha no download do modelo: {err}"))?
}

#[tauri::command]
pub async fn ear_load_photo(path: String) -> Result<EarPhoto, String> {
    tauri::async_runtime::spawn_blocking(move || pipeline::load_photo(&path))
        .await
        .map_err(|err| format!("Falha ao abrir a foto: {err}"))?
}

#[tauri::command]
pub async fn ear_analyze(image_base64: String) -> Result<EarAnalysis, String> {
    tauri::async_runtime::spawn_blocking(move || pipeline::analyze(&image_base64))
        .await
        .map_err(|err| format!("Falha na análise da foto: {err}"))?
}
