//! Global keyboard shortcuts.

use crate::models::Settings;
use crate::state::{run_action, AppState};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

/// Re-registers every configured shortcut, returning human readable problems.
pub fn register(app: &AppHandle, settings: &Settings) -> Vec<String> {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    let mut warnings = Vec::new();
    for (action, accelerator) in &settings.shortcuts {
        let accelerator = accelerator.trim();
        if accelerator.is_empty() {
            continue;
        }
        match accelerator.parse::<Shortcut>() {
            Ok(shortcut) => {
                if let Err(err) = manager.register(shortcut) {
                    warnings.push(format!(
                        "Não foi possível registrar o atalho “{accelerator}” ({action}): {err}"
                    ));
                }
            }
            Err(err) => warnings.push(format!("Atalho inválido “{accelerator}” ({action}): {err}")),
        }
    }
    warnings
}

fn action_for(app: &AppHandle, shortcut: &Shortcut) -> Option<String> {
    let state = app.try_state::<AppState>()?;
    let settings = state.store.snapshot().settings;
    settings.shortcuts.iter().find_map(|(action, accelerator)| {
        let parsed = accelerator.trim().parse::<Shortcut>().ok()?;
        (parsed == *shortcut).then(|| action.clone())
    })
}

pub fn handle(app: &AppHandle, shortcut: &Shortcut, pressed: bool) {
    if !pressed {
        return;
    }
    let Some(action) = action_for(app, shortcut) else {
        return;
    };
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if let Err(err) = run_action(app, &state, &action) {
        state.push_warning(err);
    }
}
