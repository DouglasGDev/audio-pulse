mod audio;
mod commands;
mod config;
pub mod ear;
mod models;
mod provision;
mod shortcuts;
mod state;
mod tray;

use crate::audio::engine::EngineHandle;
use crate::config::ConfigStore;
use crate::state::{emit_view, AppState};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Emitter, Manager, RunEvent, WindowEvent};

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let loaded = ConfigStore::load();
    let mut warnings = loaded.warnings.clone();
    let (store, extra_warnings) = ConfigStore::from_loaded(loaded);
    warnings.extend(extra_warnings);
    let initial = store.snapshot();
    let initial_params = initial.params.clone();
    let initial_settings = initial.settings.clone();
    let engine = EngineHandle::spawn(initial_params);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(shortcut_plugin())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::set_params,
            commands::set_volume,
            commands::toggle_bypass,
            commands::set_spatial_mode,
            commands::apply_preset,
            commands::save_preset,
            commands::delete_preset,
            commands::restore_sound_defaults,
            commands::export_preset,
            commands::import_preset,
            commands::import_autoeq,
            commands::refresh_devices,
            provision::install_virtual_cable,
            commands::start_engine,
            commands::stop_engine,
            commands::update_settings,
            commands::set_device_profile,
            ear::commands::ear_model_status,
            ear::commands::ear_download_model,
            ear::commands::ear_load_photo,
            ear::commands::ear_analyze,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let state = AppState::new(Arc::clone(&store), engine, warnings);
            app.manage(state);

            {
                let state = app.state::<AppState>();
                if let Err(err) = tray::build(&handle, &state) {
                    log::error!("failed to create tray icon: {err}");
                    state.push_warning(format!("Não foi possível criar o ícone da bandeja: {err}"));
                }
                for warning in shortcuts::register(&handle, &initial_settings) {
                    state.push_warning(warning);
                }
                if let Err(message) = commands::apply_autostart(&handle, initial_settings.autostart) {
                    state.push_warning(message);
                }
                let master = state.params().volume;
                crate::audio::system_default::recover_stale(master);
                state.init_volume_sync();
                state.ensure_default_preset();
                state.refresh_battery();
                emit_view(&handle, &state);
            }

            spawn_telemetry(handle.clone());
            spawn_device_watch(handle.clone());
            spawn_volume_watch(handle.clone());
            spawn_route_watch(handle.clone());
            spawn_battery_watch(handle.clone());
            spawn_auto_start(handle.clone());

            let minimized = std::env::args().any(|arg| arg == "--minimized");
            if minimized && initial_settings.launch_minimized {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            tray::on_menu_event(app, event.id().as_ref());
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if let Some(state) = window.try_state::<AppState>() {
                    if state.store.snapshot().settings.close_to_tray {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("falha ao iniciar o Audio Pulse")
        .run(|app, event| match event {
            RunEvent::ExitRequested { .. } => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.store.save_now();
                }
            }
            RunEvent::Exit => {
                if let Some(state) = app.try_state::<AppState>() {
                    state.store.save_now();
                    state.release_system_default();
                    state.engine.shutdown();
                }
            }
            _ => {}
        });
}

fn shortcut_plugin() -> TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            let pressed =
                event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed;
            shortcuts::handle(app, shortcut, pressed);
        })
        .build()
}

fn spawn_telemetry(app: AppHandle) {
    let _ = thread::Builder::new()
        .name("audio-pulse-telemetry".into())
        .spawn(move || loop {
            thread::sleep(Duration::from_millis(33));
            let Some(state) = app.try_state::<AppState>() else {
                break;
            };
            let telemetry = state.engine.shared().telemetry();
            if app.emit("telemetry", telemetry).is_err() {
                break;
            }
        });
}

fn spawn_auto_start(app: AppHandle) {
    let _ = thread::Builder::new()
        .name("audio-pulse-auto-route".into())
        .spawn(move || {
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            state.auto_start_route();
            emit_view(&app, &state);
        });
}

/// Keeps the route alive when a headset is plugged in, disconnected, or when
/// Windows hands the default output to another endpoint.
fn spawn_route_watch(app: AppHandle) {
    let _ = thread::Builder::new()
        .name("audio-pulse-route".into())
        .spawn(move || loop {
            thread::sleep(Duration::from_millis(1500));
            let Some(state) = app.try_state::<AppState>() else {
                break;
            };
            state.supervise_route(&app);
        });
}

fn spawn_device_watch(app: AppHandle) {
    let _ = thread::Builder::new()
        .name("audio-pulse-devices".into())
        .spawn(move || loop {
            thread::sleep(Duration::from_secs(3));
            let Some(state) = app.try_state::<AppState>() else {
                break;
            };
            let list = audio::devices::enumerate();
            let changed = {
                let mut devices = state.devices.write();
                if devices.signature() == list.signature() {
                    false
                } else {
                    *devices = list.clone();
                    true
                }
            };
            if changed {
                let _ = app.emit("devices-changed", list);
            }
        });
}

/// Watches the Windows master so the app slider and the system controller
/// stay the same control, including when the user moves the system one.
fn spawn_volume_watch(app: AppHandle) {
    let _ = thread::Builder::new()
        .name("audio-pulse-volume".into())
        .spawn(move || loop {
            thread::sleep(Duration::from_millis(300));
            let Some(state) = app.try_state::<AppState>() else {
                break;
            };
            state.poll_volume(&app);
        });
}

fn spawn_battery_watch(app: AppHandle) {
    let _ = thread::Builder::new()
        .name("audio-pulse-battery".into())
        .spawn(move || loop {
            thread::sleep(Duration::from_secs(10));
            let Some(state) = app.try_state::<AppState>() else {
                break;
            };
            if state.refresh_battery() {
                let _ = app.emit("state-changed", state.view());
            }
        });
}