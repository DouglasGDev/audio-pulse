//! Keeps the Windows default playback endpoint on the virtual cable while the
//! route runs, so every application feeds the DSP chain, and restores the
//! previous endpoint when the route stops.

use crate::models::DeviceList;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedDefault {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Engaged {
    pub previous: SavedDefault,
    pub cable: SavedDefault,
    /// Whether the master volume lives on the Windows endpoint. Then the route
    /// output is pinned at unity and the previous default receives the master
    /// back when the route stops.
    #[serde(default)]
    pub sync_volume: bool,
}

/// Render endpoint that receives the system audio: the playback side of the
/// virtual cable (CABLE Input on Windows, BlackHole on macOS).
pub fn pick_cable_output(devices: &DeviceList) -> Option<String> {
    let virtual_outputs: Vec<&crate::models::DeviceInfo> =
        devices.outputs.iter().filter(|d| d.is_virtual).collect();
    virtual_outputs
        .iter()
        .find(|d| d.name.to_lowercase().contains("cable input"))
        .or_else(|| {
            virtual_outputs
                .iter()
                .find(|d| d.name.to_lowercase().contains("cable"))
        })
        .or_else(|| virtual_outputs.first())
        .map(|d| d.name.clone())
}

/// Switches the default output to the cable. `cable_volume` is written before
/// the switch so the first captured buffer already carries the master level.
pub fn engage(
    cable_output_name: &str,
    physical_output_name: Option<&str>,
    cable_volume: f32,
    sync_volume: bool,
) -> Result<Option<Engaged>, String> {
    imp::engage(cable_output_name, physical_output_name, cable_volume, sync_volume)
}

/// Restores the previous default. When `volume` is set, it is written to the
/// restored endpoint before the switch, so direct playback keeps the master.
pub fn restore(engaged: &Engaged, volume: Option<f32>) -> Result<(), String> {
    imp::restore(engaged, volume)
}

/// Undo an endpoint switch left behind by a crash or forced shutdown.
pub fn recover_stale(master_volume: f32) {
    let path = state_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return;
    };
    match serde_json::from_str::<Engaged>(&text) {
        Ok(engaged) => {
            let volume = engaged.sync_volume.then_some(master_volume);
            // The record is only dropped once the endpoint is really back;
            // a transient COM failure keeps it for the next launch.
            if let Err(err) = restore(&engaged, volume) {
                log::warn!("could not restore default audio endpoint: {err}");
            }
        }
        Err(err) => {
            log::warn!("ignored corrupt route-default.json: {err}");
            let _ = std::fs::remove_file(&path);
        }
    }
}

fn state_path() -> PathBuf {
    crate::config::config_path().with_file_name("route-default.json")
}

#[cfg(windows)]
pub(crate) fn persist(engaged: &Engaged) {
    let path = state_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(err) = serde_json::to_string_pretty(engaged)
        .map_err(|e| e.to_string())
        .and_then(|json| std::fs::write(&path, json).map_err(|e| e.to_string()))
    {
        log::warn!("could not persist default audio endpoint: {err}");
    }
}

#[cfg(windows)]
pub(crate) fn clear_persisted() {
    let _ = std::fs::remove_file(state_path());
}

#[cfg(windows)]
mod imp {
    use super::{clear_persisted, persist, Engaged, SavedDefault};
    use crate::audio::win_endpoint::Session;

    fn saved(endpoint: crate::audio::win_endpoint::Endpoint) -> SavedDefault {
        SavedDefault {
            id: endpoint.id,
            name: endpoint.name,
        }
    }

    pub fn engage(
        cable_output_name: &str,
        physical_output_name: Option<&str>,
        cable_volume: f32,
        sync_volume: bool,
    ) -> Result<Option<Engaged>, String> {
        let session = Session::new()?;
        let Some(cable) = session.endpoint_by_name(cable_output_name)? else {
            return Err(format!(
                "A saída \"{cable_output_name}\" não apareceu no Windows."
            ));
        };
        let previous = session.default_endpoint()?;
        let cable_state = session.state_by_name(&cable.name)?;
        session.apply_volume_by_id(&cable.id, Some(cable_volume), Some(false))?;
        if previous.id == cable.id {
            // A previous session left the cable as the default and its saved
            // endpoint is gone. Fall back to the output the route feeds, so
            // quitting still hands the speakers back.
            let Some(physical) = physical_output_name
                .filter(|name| !name.eq_ignore_ascii_case(&cable.name))
                .and_then(|name| session.endpoint_by_name(name).ok().flatten())
            else {
                return Ok(None);
            };
            let engaged = Engaged {
                previous: saved(physical),
                cable: saved(cable),
                sync_volume,
            };
            persist(&engaged);
            log::info!(
                "system default output stays on {}; restore target: {}",
                engaged.cable.name,
                engaged.previous.name
            );
            return Ok(Some(engaged));
        }
        if let Err(err) = session.switch_default(&cable.id) {
            if let Some(state) = cable_state {
                let _ = session.apply_volume_by_id(
                    &cable.id,
                    Some(state.scalar),
                    Some(state.muted),
                );
            }
            let _ = session.switch_default(&previous.id);
            return Err(err);
        }
        let engaged = Engaged {
            previous: saved(previous),
            cable: saved(cable),
            sync_volume,
        };
        persist(&engaged);
        log::info!(
            "system default output: {} -> {}",
            engaged.previous.name,
            engaged.cable.name
        );
        Ok(Some(engaged))
    }

    pub fn restore(engaged: &Engaged, volume: Option<f32>) -> Result<(), String> {
        let session = Session::new()?;
        if !session.is_default(&engaged.cable.id) {
            // The user picked another output meanwhile; leave it alone.
            clear_persisted();
            return Ok(());
        }
        let target = session
            .endpoint_by_id(&engaged.previous.id)
            .or_else(|| {
                session
                    .endpoint_by_name(&engaged.previous.name)
                    .ok()
                    .flatten()
            })
            .ok_or_else(|| {
                format!(
                    "Não foi possível restaurar a saída \"{}\".",
                    engaged.previous.name
                )
            })?;
        if let Some(volume) = volume {
            let (scalar, mute) = if volume <= 0.0005 {
                (None, true)
            } else {
                (Some(volume), false)
            };
            session.apply_volume_by_id(&target.id, scalar, Some(mute))?;
        }
        session.switch_default(&target.id)?;
        clear_persisted();
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Engaged;

    pub fn engage(
        _cable_output_name: &str,
        _physical_output_name: Option<&str>,
        _cable_volume: f32,
        _sync_volume: bool,
    ) -> Result<Option<Engaged>, String> {
        Ok(None)
    }

    pub fn restore(_engaged: &Engaged, _volume: Option<f32>) -> Result<(), String> {
        Ok(())
    }
}
