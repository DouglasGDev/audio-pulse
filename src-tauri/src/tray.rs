//! System tray icon and its quick-control menu.

use crate::state::{emit_view, show_window, AppState};
use audio_pulse_dsp::{PresetCategory, SpatialMode};
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

struct Labels {
    bypass: &'static str,
    volume_up: &'static str,
    volume_down: &'static str,
    spatial: &'static str,
    spatial_off: &'static str,
    spatial_binaural: &'static str,
    spatial_hrtf: &'static str,
    spatial_surround: &'static str,
    presets: &'static str,
    open: &'static str,
    quit: &'static str,
}

fn labels(language: &str) -> Labels {
    if language == "pt-BR" {
        Labels {
            bypass: "Bypass",
            volume_up: "Aumentar volume",
            volume_down: "Diminuir volume",
            spatial: "Áudio espacial",
            spatial_off: "Desligado",
            spatial_binaural: "Binaural",
            spatial_hrtf: "HRTF dinâmico",
            spatial_surround: "Surround 7.1",
            presets: "Presets",
            open: "Abrir Audio Pulse",
            quit: "Sair",
        }
    } else {
        Labels {
            bypass: "Bypass",
            volume_up: "Volume up",
            volume_down: "Volume down",
            spatial: "Spatial audio",
            spatial_off: "Off",
            spatial_binaural: "Binaural",
            spatial_hrtf: "Dynamic HRTF",
            spatial_surround: "Surround 7.1",
            presets: "Presets",
            open: "Open Audio Pulse",
            quit: "Quit",
        }
    }
}

fn preset_label(id: &str, fallback: &str, language: &str) -> String {
    let pt = language == "pt-BR";
    match id {
        "music" => (if pt { "Música" } else { "Music" }).into(),
        "cinema" => "Cinema".into(),
        "gaming" => (if pt { "Jogos" } else { "Gaming" }).into(),
        "podcast" => "Podcast".into(),
        "vocal" => (if pt { "Voz" } else { "Vocal" }).into(),
        "flat" => (if pt { "Plano" } else { "Flat" }).into(),
        "bass" => (if pt { "Graves" } else { "Bass" }).into(),
        "night" => (if pt { "Noturno" } else { "Night" }).into(),
        _ => fallback.to_string(),
    }
}

fn build_menu(app: &AppHandle, state: &AppState) -> tauri::Result<Menu<tauri::Wry>> {
    let snapshot = state.store.snapshot();
    let language = snapshot.settings.resolved_language();
    let labels = labels(language);

    let header = MenuItem::with_id(app, "header", "Audio Pulse", false, None::<&str>)?;
    let bypass = CheckMenuItem::with_id(
        app,
        "bypass",
        labels.bypass,
        true,
        snapshot.params.bypass,
        None::<&str>,
    )?;
    let volume_up = MenuItem::with_id(
        app,
        "volume_up",
        format!("{}  {}", labels.volume_up, "↑"),
        true,
        Some("CmdOrCtrl+Alt+Up"),
    )?;
    let volume_down = MenuItem::with_id(
        app,
        "volume_down",
        format!("{}  {}", labels.volume_down, "↓"),
        true,
        Some("CmdOrCtrl+Alt+Down"),
    )?;

    let spatial_off = CheckMenuItem::with_id(
        app,
        "spatial:off",
        labels.spatial_off,
        true,
        snapshot.params.spatial.mode == SpatialMode::Off,
        None::<&str>,
    )?;
    let spatial_binaural = CheckMenuItem::with_id(
        app,
        "spatial:binaural",
        labels.spatial_binaural,
        true,
        snapshot.params.spatial.mode == SpatialMode::Binaural,
        None::<&str>,
    )?;
    let spatial_hrtf = CheckMenuItem::with_id(
        app,
        "spatial:hrtf",
        labels.spatial_hrtf,
        true,
        snapshot.params.spatial.mode == SpatialMode::Hrtf,
        None::<&str>,
    )?;
    let spatial_surround = CheckMenuItem::with_id(
        app,
        "spatial:surround",
        labels.spatial_surround,
        true,
        snapshot.params.spatial.mode == SpatialMode::Surround,
        None::<&str>,
    )?;
    let spatial = Submenu::with_items(
        app,
        labels.spatial,
        true,
        &[
            &spatial_off,
            &spatial_binaural,
            &spatial_hrtf,
            &spatial_surround,
        ],
    )?;

    let mut preset_items: Vec<MenuItem<tauri::Wry>> = Vec::new();
    for preset in audio_pulse_dsp::builtin_presets() {
        if preset.category == PresetCategory::Headphone || preset.category == PresetCategory::Custom
        {
            continue;
        }
        let label = preset_label(&preset.id, &preset.name, language);
        let item = MenuItem::with_id(
            app,
            format!("preset:{}", preset.id),
            label,
            true,
            None::<&str>,
        )?;
        preset_items.push(item);
    }
    let preset_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        preset_items.iter().map(|item| item as _).collect();
    let presets = Submenu::with_items(app, labels.presets, true, &preset_refs)?;

    let separator = PredefinedMenuItem::separator(app)?;
    let open = MenuItem::with_id(app, "open", labels.open, true, Some("CmdOrCtrl+Alt+O"))?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;

    Menu::with_items(
        app,
        &[
            &header,
            &bypass,
            &volume_up,
            &volume_down,
            &spatial,
            &presets,
            &separator,
            &open,
            &quit,
        ],
    )
}

pub fn build(app: &AppHandle, state: &AppState) -> tauri::Result<()> {
    if app.tray_by_id("main").is_some() {
        return refresh(app, state);
    }
    let menu = build_menu(app, state)?;
    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Audio Pulse");
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

pub fn refresh(app: &AppHandle, state: &AppState) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id("main") {
        let menu = build_menu(app, state)?;
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}

pub fn on_menu_event(app: &AppHandle, id: &str) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    match id {
        "bypass" => {
            state.toggle_bypass();
        }
        "volume_up" => {
            if let Err(err) = crate::state::run_action(app, &state, "volume_up") {
                state.push_warning(err);
            }
            return;
        }
        "volume_down" => {
            if let Err(err) = crate::state::run_action(app, &state, "volume_down") {
                state.push_warning(err);
            }
            return;
        }
        "spatial:off" => {
            state.set_spatial_mode(SpatialMode::Off);
        }
        "spatial:binaural" => {
            state.set_spatial_mode(SpatialMode::Binaural);
        }
        "spatial:hrtf" => {
            state.set_spatial_mode(SpatialMode::Hrtf);
        }
        "spatial:surround" => {
            state.set_spatial_mode(SpatialMode::Surround);
        }
        "open" => {
            show_window(app);
            return;
        }
        "quit" => {
            app.exit(0);
            return;
        }
        other if other.starts_with("preset:") => {
            if let Err(err) = state.apply_preset(&other["preset:".len()..]) {
                state.push_warning(err);
            }
        }
        _ => return,
    }
    emit_view(app, &state);
}
