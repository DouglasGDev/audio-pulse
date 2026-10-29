//! Tauri commands exposed to the React frontend.

use crate::models::{AppView, DeviceList, EngineInfo, ParamsUpdate, Settings};
use crate::shortcuts;
use crate::state::{emit_view, AppState};
use serde::{Deserialize, Serialize};
use audio_pulse_dsp::autoeq::parse_autoeq_text;
use audio_pulse_dsp::{Preset, PresetCategory, ProcessingParams, SpatialMode, PRESET_FORMAT, PRESET_FORMAT_VERSION};

// Presets exported before the Audio Pulse rename stay importable.
const LEGACY_PRESET_FORMAT: &str = "sonicenhance-preset";
use std::fs;
use std::path::Path;
use tauri::{AppHandle, State};

#[derive(Serialize, Deserialize)]
struct PresetFile {
    format: String,
    version: u32,
    preset: Preset,
}

#[tauri::command]
pub fn get_state(state: State<'_, AppState>) -> AppView {
    state.view()
}

#[tauri::command]
pub fn set_params(app: AppHandle, state: State<'_, AppState>, params: ProcessingParams) -> ParamsUpdate {
    let params = state.set_params(params);
    state.sync_tray(&app);
    ParamsUpdate {
        params,
        active_preset_id: state.store.snapshot().active_preset_id,
    }
}

#[tauri::command]
pub fn set_volume(
    app: AppHandle,
    state: State<'_, AppState>,
    volume: f32,
) -> ParamsUpdate {
    let params = state.set_volume(volume);
    state.sync_tray(&app);
    ParamsUpdate {
        params,
        active_preset_id: state.store.snapshot().active_preset_id,
    }
}

#[tauri::command]
pub fn toggle_bypass(app: AppHandle, state: State<'_, AppState>) -> ParamsUpdate {
    let params = state.toggle_bypass();
    state.sync_tray(&app);
    ParamsUpdate {
        params,
        active_preset_id: state.store.snapshot().active_preset_id,
    }
}

#[tauri::command]
pub fn set_spatial_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: String,
) -> Result<ParamsUpdate, String> {
    let mode = match mode.as_str() {
        "off" => SpatialMode::Off,
        "binaural" => SpatialMode::Binaural,
        "hrtf" => SpatialMode::Hrtf,
        "surround" => SpatialMode::Surround,
        other => return Err(format!("Modo espacial desconhecido: {other}")),
    };
    let params = state.set_spatial_mode(mode);
    state.sync_tray(&app);
    Ok(ParamsUpdate {
        params,
        active_preset_id: state.store.snapshot().active_preset_id,
    })
}

#[tauri::command]
pub fn apply_preset(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<AppView, String> {
    state.apply_preset(&id)?;
    emit_view(&app, &state);
    Ok(state.view())
}

#[tauri::command]
pub fn save_preset(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<AppView, String> {
    state.save_preset(&name)?;
    emit_view(&app, &state);
    Ok(state.view())
}

#[tauri::command]
pub fn delete_preset(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<AppView, String> {
    state.delete_preset(&id)?;
    emit_view(&app, &state);
    Ok(state.view())
}

#[tauri::command]
pub fn restore_sound_defaults(app: AppHandle, state: State<'_, AppState>) -> Result<AppView, String> {
    state.restore_sound_defaults()?;
    emit_view(&app, &state);
    Ok(state.view())
}

#[tauri::command]
pub fn export_preset(
    state: State<'_, AppState>,
    id: String,
    path: String,
) -> Result<String, String> {
    let snapshot = state.store.snapshot();
    let preset = audio_pulse_dsp::find_builtin(&id)
        .or_else(|| {
            snapshot
                .custom_presets
                .iter()
                .find(|preset| preset.id == id)
                .cloned()
        })
        .ok_or_else(|| format!("Preset não encontrado: {id}"))?;
    let file = PresetFile {
        format: PRESET_FORMAT.to_string(),
        version: PRESET_FORMAT_VERSION,
        preset,
    };
    let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    write_atomic(Path::new(&path), &json)?;
    Ok(path)
}

#[tauri::command]
pub fn import_preset(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<AppView, String> {
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("Não foi possível ler o arquivo: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("Arquivo de preset inválido: {e}"))?;

    let preset_value = if value.get("format").is_some() {
        let format = value.get("format").and_then(|v| v.as_str()).unwrap_or("");
        if format != PRESET_FORMAT && format != LEGACY_PRESET_FORMAT {
            return Err("O arquivo não é um preset do Audio Pulse.".into());
        }
        let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
        if version > PRESET_FORMAT_VERSION {
            return Err("Este preset foi criado por uma versão mais recente do Audio Pulse.".into());
        }
        value
            .get("preset")
            .cloned()
            .ok_or_else(|| "O arquivo não contém um preset.".to_string())?
    } else {
        value
    };

    let mut preset: Preset = serde_json::from_value(preset_value)
        .map_err(|e| format!("Preset inválido: {e}"))?;
    let fallback_name = Path::new(&path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Preset importado")
        .to_string();
    if preset.name.trim().is_empty() {
        preset.name = fallback_name;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    preset.id = format!("custom-{stamp}");
    preset.built_in = false;
    preset.category = PresetCategory::Custom;
    preset.params = preset.params.sanitized();

    state.store.mutate(|config| config.custom_presets.push(preset));
    emit_view(&app, &state);
    Ok(state.view())
}

#[tauri::command]
pub fn import_autoeq(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<AppView, String> {
    let text = fs::read_to_string(&path)
        .map_err(|e| format!("Não foi possível ler o arquivo AutoEQ: {e}"))?;
    let parsed = parse_autoeq_text(&text)?;
    if parsed.filters.is_empty() {
        return Err("Nenhum filtro PK/LS/HS foi encontrado no arquivo.".into());
    }
    let mut params = state.params();
    params.auto_eq.enabled = true;
    params.auto_eq.preamp_db = parsed.preamp_db;
    params.auto_eq.filters = parsed.filters;
    state.set_params_declick(params);
    state.store.mutate(|config| config.active_preset_id = None);
    emit_view(&app, &state);
    Ok(state.view())
}

#[tauri::command]
pub fn refresh_devices(state: State<'_, AppState>) -> DeviceList {
    state.refresh_devices()
}

#[tauri::command]
pub fn start_engine(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: String,
    wav_path: Option<String>,
) -> Result<EngineInfo, String> {
    let info = state.start_engine(&mode, wav_path)?;
    emit_view(&app, &state);
    Ok(info)
}

#[tauri::command]
pub fn stop_engine(app: AppHandle, state: State<'_, AppState>) -> EngineInfo {
    let info = state.stop_engine();
    emit_view(&app, &state);
    info
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<AppView, String> {
    let previous = state.store.snapshot().settings;
    let next = settings.sanitized();

    if let Err(message) = apply_autostart(&app, next.autostart) {
        state.push_warning(message);
    }
    state.store.mutate(|config| config.settings = next.clone());
    for warning in shortcuts::register(&app, &next) {
        state.push_warning(warning);
    }
    state.restart_if_needed(&previous, &next);
    emit_view(&app, &state);
    Ok(state.view())
}

#[tauri::command]
pub fn set_device_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    device: String,
    preset_id: Option<String>,
) -> AppView {
    state.set_device_profile(&device, preset_id);
    emit_view(&app, &state);
    state.view()
}

fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Caminho de destino inválido.".to_string())?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, contents).map_err(|e| e.to_string())?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

pub(crate) fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let current = manager.is_enabled().unwrap_or(false);
    if current == enabled {
        return Ok(());
    }
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|e| format!("Não foi possível atualizar a inicialização automática: {e}"))
}