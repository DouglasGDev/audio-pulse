//! Application state and the high-level operations shared by commands, the
//! tray and global shortcuts.

use crate::audio::battery;
use crate::audio::devices;
use crate::audio::engine::{EngineHandle, EngineStart, PreviewSource};
use crate::audio::system_default::{self, Engaged};
use crate::audio::system_volume;
use crate::config::ConfigStore;
use crate::models::{AppView, BatteryInfo, DeviceList, EngineInfo, Settings};
use crate::tray;
use parking_lot::{Mutex, RwLock};
use audio_pulse_dsp::presets::{builtin_presets, find_builtin};
use audio_pulse_dsp::{EarProfileParams, PresetCategory, ProcessingParams, SpatialMode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const ROUTE_OUTPUT_WARNING: &str =
    "Nenhum fone disponível: conecte um fone para a rota continuar.";
const ROUTE_INPUT_WARNING: &str =
    "Cabo virtual não encontrado: instale ou reative o cabo em Ajustes → Áudio.";
const ROUTE_RECOVERY_INTERVAL: Duration = Duration::from_millis(1800);
use tauri::{AppHandle, Emitter, Manager};

#[derive(Default)]
struct VolumeSync {
    enabled: bool,
    last_endpoint: Option<String>,
    last_written: Option<f32>,
    suppress_until: Option<Instant>,
    retry_at: Option<Instant>,
    last_pin_check: Option<Instant>,
}

pub struct AppState {
    pub store: Arc<ConfigStore>,
    pub engine: EngineHandle,
    pub devices: RwLock<DeviceList>,
    warnings: Mutex<Vec<String>>,
    last_start: Mutex<Option<EngineStart>>,
    system_default: Mutex<Option<Engaged>>,
    engage_lock: Mutex<()>,
    route_op: Mutex<()>,
    auto_route_pending: AtomicBool,
    last_recovery: Mutex<Option<Instant>>,
    volume_sync: Mutex<VolumeSync>,
    battery: RwLock<Option<BatteryInfo>>,
    tray_signature: Mutex<String>,
}

impl AppState {
    pub fn new(store: Arc<ConfigStore>, engine: EngineHandle, warnings: Vec<String>) -> Self {
        let state = Self {
            store,
            engine,
            devices: RwLock::new(DeviceList::default()),
            warnings: Mutex::new(Vec::new()),
            last_start: Mutex::new(None),
            system_default: Mutex::new(None),
            engage_lock: Mutex::new(()),
            route_op: Mutex::new(()),
            auto_route_pending: AtomicBool::new(false),
            last_recovery: Mutex::new(None),
            volume_sync: Mutex::new(VolumeSync::default()),
            battery: RwLock::new(None),
            tray_signature: Mutex::new(String::new()),
        };
        state.refresh_devices();
        for warning in warnings {
            state.push_warning(warning);
        }
        state
    }

    pub fn params(&self) -> ProcessingParams {
        self.store.snapshot().params
    }

    pub fn push_warning(&self, message: String) {
        let mut warnings = self.warnings.lock();
        if !warnings.iter().any(|existing| existing == &message) {
            warnings.push(message);
        }
    }

    pub fn view(&self) -> AppView {
        let snapshot = self.store.snapshot();
        let mut presets = builtin_presets();
        presets.extend(snapshot.custom_presets.clone());
        AppView {
            params: snapshot.params,
            settings: snapshot.settings,
            presets,
            active_preset_id: snapshot.active_preset_id,
            devices: self.devices.read().clone(),
            engine: self.engine.info(),
            battery: self.battery.read().clone(),
            warnings: self.warnings.lock().clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn refresh_devices(&self) -> DeviceList {
        let list = devices::enumerate();
        *self.devices.write() = list.clone();
        list
    }

    pub fn set_params(&self, params: ProcessingParams) -> ProcessingParams {
        self.commit_params(params, false)
    }

    /// Discrete changes (preset, bypass, spatial mode) replace the whole chain
    /// at once; the DSP fades around the swap instead of stepping.
    pub fn set_params_declick(&self, params: ProcessingParams) -> ProcessingParams {
        self.commit_params(params, true)
    }

    fn commit_params(&self, params: ProcessingParams, declick: bool) -> ProcessingParams {
        let params = params.sanitized();
        let previous_volume = self.store.snapshot().params.volume;
        let snapshot = self.store.snapshot();
        let still_matches = match snapshot.active_preset_id.as_deref() {
            Some(id) => {
                let preset = find_builtin(id).or_else(|| {
                    snapshot
                        .custom_presets
                        .iter()
                        .find(|preset| preset.id == id)
                        .cloned()
                });
                preset
                    .map(|preset| same_shaping(&preset.params, &params))
                    .unwrap_or(false)
            }
            None => true,
        };
        self.store.mutate(|config| {
            config.params = params.clone();
            if !still_matches {
                log::debug!("active preset cleared after manual change");
                config.active_preset_id = None;
            }
        });
        if declick {
            self.engine.set_params_declick(&params);
        } else {
            self.engine.set_params(&params);
        }
        if (previous_volume - params.volume).abs() > f32::EPSILON {
            self.write_master_to_system(params.volume);
        }
        params
    }

    pub fn set_volume(&self, volume: f32) -> ProcessingParams {
        let mut params = self.params();
        params.volume = volume;
        self.set_params(params)
    }

    /// Windows keeps the master control: the level is mirrored onto the
    /// default endpoint, and `pin_output_level` decides whether the DSP or
    /// that endpoint applies the gain.
    pub fn init_volume_sync(&self) {
        if !system_volume::supported() {
            return;
        }
        let snapshot = self.store.snapshot();
        let endpoint = match system_volume::read_default() {
            Ok(endpoint) => endpoint,
            Err(err) => {
                log::warn!("system volume unavailable: {err}");
                self.volume_sync.lock().retry_at = Some(Instant::now() + Duration::from_secs(2));
                return;
            }
        };

        if !snapshot.volume_sync_initialized {
            // Before the sync, both the DSP gain and the endpoint scalar were
            // applied. Fold them into the endpoint so the level does not jump.
            let master = system_volume::migrated_master(snapshot.params.volume, &endpoint);
            if let Err(err) = system_volume::write_default(master) {
                log::warn!("could not hand the master volume to Windows: {err}");
                self.volume_sync.lock().retry_at = Some(Instant::now() + Duration::from_secs(2));
                return;
            }
            self.store
                .mutate(|config| config.volume_sync_initialized = true);
            self.apply_master_locally(master);
            log::info!(
                "master volume on the Windows endpoint: {:.0}% ({:.0}% x {:.0}%)",
                master * 100.0,
                snapshot.params.volume * 100.0,
                endpoint.effective() * 100.0
            );
        } else {
            // The user may have moved the Windows slider while we were closed.
            self.apply_master_locally(endpoint.effective());
        }

        let master = self.params().volume;
        {
            let mut sync = self.volume_sync.lock();
            sync.enabled = true;
            sync.retry_at = None;
            sync.last_endpoint = Some(endpoint.id);
            sync.last_written = Some(master);
            sync.suppress_until = Some(Instant::now() + Duration::from_millis(500));
        }
        self.engine.set_hardware_volume(true);
    }

    /// Mirrors the endpoint into the app when the user moves the Windows
    /// controller, and mirrors the app back after our own writes settle.
    pub fn poll_volume(&self, app: &AppHandle) {
        if !system_volume::supported() {
            return;
        }
        let now = Instant::now();
        let (enabled, retrying) = {
            let sync = self.volume_sync.lock();
            (
                sync.enabled,
                sync.retry_at.is_some_and(|at| now >= at),
            )
        };
        if !enabled {
            if retrying {
                self.init_volume_sync();
                if self.volume_sync.lock().enabled {
                    emit_view(app, self);
                }
            }
            return;
        }

        let Ok(endpoint) = system_volume::read_default() else {
            return;
        };
        let route_active = matches!(*self.last_start.lock(), Some(EngineStart::Route { .. }));
        if route_active {
            let cable = system_default::pick_cable_output(&self.devices.read().clone());
            let on_cable = cable
                .as_deref()
                .is_some_and(|cable| endpoint.name.eq_ignore_ascii_case(cable));
            if !on_cable {
                // While routing, the master lives in the DSP. If Windows moved
                // the default (headset plugged in), do not mirror or raise the
                // new endpoint; the route supervisor takes the cable back.
                self.engine.set_hardware_volume(false);
                return;
            }
        }
        let master = self.params().volume;
        let mut sync = self.volume_sync.lock();
        if sync.retry_at.is_some_and(|at| now >= at) {
            drop(sync);
            self.write_master_to_system(master);
            return;
        }
        if sync.last_endpoint.as_deref() != Some(endpoint.id.as_str()) {
            // Another endpoint took the default role: it now owns the master.
            sync.last_endpoint = Some(endpoint.id);
            sync.last_written = None;
            sync.last_pin_check = Some(now);
            drop(sync);
            self.write_master_to_system(master);
            self.pin_output_level();
            return;
        }
        let suppressed = sync.suppress_until.is_some_and(|until| now < until);
        let reference = sync.last_written.unwrap_or(master);
        drop(sync);

        if !suppressed {
            let effective = endpoint.effective();
            if system_volume::differs(effective, reference) {
                let changed = self.apply_master_locally(effective);
                self.volume_sync.lock().last_written = Some(effective);
                if changed {
                    emit_view(app, self);
                }
            }
        }

        let should_pin = {
            let mut sync = self.volume_sync.lock();
            if sync
                .last_pin_check
                .is_none_or(|last| now.duration_since(last) >= Duration::from_millis(1500))
            {
                sync.last_pin_check = Some(now);
                true
            } else {
                false
            }
        };
        if should_pin {
            self.pin_output_level();
        }
    }

    fn write_master_to_system(&self, volume: f32) {
        if !system_volume::supported() || !self.volume_sync.lock().enabled {
            return;
        }
        match system_volume::write_default(volume) {
            Ok(()) => {
                let mut sync = self.volume_sync.lock();
                sync.last_written = Some(volume.clamp(0.0, 1.0));
                sync.suppress_until = Some(Instant::now() + Duration::from_millis(400));
                sync.retry_at = None;
            }
            Err(err) => {
                log::warn!("could not write the master volume to Windows: {err}");
                self.volume_sync.lock().retry_at = Some(Instant::now() + Duration::from_secs(2));
            }
        }
    }

    /// Applies a level learned from Windows without echoing it back.
    fn apply_master_locally(&self, volume: f32) -> bool {
        let volume = volume.clamp(0.0, 1.0);
        let mut params = self.params();
        if (params.volume - volume).abs() <= f32::EPSILON {
            return false;
        }
        params.volume = volume;
        self.store.mutate(|config| config.params = params.clone());
        self.engine.set_params(&params);
        true
    }

    /// While routing the DSP owns the master and the physical output stays at
    /// unity; otherwise the Windows endpoint is the master.
    fn pin_output_level(&self) {
        if !self.volume_sync.lock().enabled {
            return;
        }
        let info = self.engine.info();
        if !info.running {
            return;
        }
        let Some(output) = info.output_device.as_deref() else {
            return;
        };
        if info.mode == "route" {
            // Keep the DSP gain even if Windows briefly moved the default
            // away, so reclaiming the cable never produces a level jump.
            self.engine.set_hardware_volume(false);
            if !devices::is_virtual_name(output) {
                if let Err(err) = system_volume::normalize_endpoint(output, 1.0) {
                    log::debug!("volume pin for {output}: {err}");
                }
            }
            return;
        }
        self.engine.set_hardware_volume(true);
        if devices::is_virtual_name(output) {
            return;
        }
        let Ok(default) = system_volume::read_default() else {
            return;
        };
        if default.name.eq_ignore_ascii_case(output) {
            return;
        }
        if let Err(err) = system_volume::normalize_endpoint(output, self.params().volume) {
            log::debug!("volume pin for {output}: {err}");
        }
    }

    pub fn refresh_battery(&self) -> bool {
        let info = self
            .selected_physical_output()
            .and_then(|name| battery::read(&name));
        let mut slot = self.battery.write();
        if *slot == info {
            return false;
        }
        *slot = info;
        true
    }

    fn selected_physical_output(&self) -> Option<String> {
        let snapshot = self.store.snapshot();
        let list = self.devices.read();
        snapshot
            .settings
            .output_device
            .clone()
            .filter(|name| !devices::is_virtual_name(name))
            .or_else(|| list.default_output.clone())
            .filter(|name| !devices::is_virtual_name(name))
    }

    pub fn toggle_bypass(&self) -> ProcessingParams {
        let mut params = self.params();
        params.bypass = !params.bypass;
        self.set_params_declick(params)
    }

    pub fn set_spatial_mode(&self, mode: SpatialMode) -> ProcessingParams {
        let mut params = self.params();
        params.spatial.mode = mode;
        self.set_params_declick(params)
    }

    pub fn cycle_spatial(&self) -> ProcessingParams {
        let mode = self.params().spatial.mode.cycle();
        self.set_spatial_mode(mode)
    }

    pub fn apply_preset(&self, id: &str) -> Result<(), String> {
        let snapshot = self.store.snapshot();
        let preset = find_builtin(id)
            .or_else(|| {
                snapshot
                    .custom_presets
                    .iter()
                    .find(|preset| preset.id == id)
                    .cloned()
            })
            .ok_or_else(|| format!("Preset não encontrado: {id}"))?;
        let mut params = snapshot.params.clone();
        params.apply_processing_from(&preset.params);
        let params = params.sanitized();
        self.store.mutate(|config| {
            config.params = params.clone();
            config.active_preset_id = Some(id.to_string());
        });
        self.engine.set_params_declick(&params);
        Ok(())
    }

    pub fn save_preset(&self, name: &str) -> Result<String, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Informe um nome para o preset.".into());
        }
        let snapshot = self.store.snapshot();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let id = format!("custom-{stamp}");
        let mut params = snapshot.params.sanitized();
        // The ear profile belongs to the user, not to the preset.
        params.ear_profile = EarProfileParams::default();
        let preset = audio_pulse_dsp::Preset {
            id: id.clone(),
            name: name.to_string(),
            category: PresetCategory::Custom,
            description: "Preset personalizado.".into(),
            brand: None,
            model: None,
            built_in: false,
            params,
        };
        self.store.mutate(|config| {
            config.custom_presets.push(preset);
            config.active_preset_id = Some(id.clone());
        });
        Ok(id)
    }

    /// Returns the sound to the app's factory state: the default preset and no
    /// ear profile. The master volume belongs to Windows and is left alone.
    pub fn restore_sound_defaults(&self) -> Result<ProcessingParams, String> {
        let (preset_id, params) = factory_sound_params(self.params().volume)?;
        self.store.mutate(|config| {
            config.params = params.clone();
            config.active_preset_id = Some(preset_id);
        });
        self.engine.set_params_declick(&params);
        Ok(params)
    }

    pub fn delete_preset(&self, id: &str) -> Result<(), String> {
        let snapshot = self.store.snapshot();
        if !snapshot.custom_presets.iter().any(|preset| preset.id == id) {
            return Err("Apenas presets personalizados podem ser excluídos.".into());
        }
        self.store.mutate(|config| {
            config.custom_presets.retain(|preset| preset.id != id);
            if config.active_preset_id.as_deref() == Some(id) {
                config.active_preset_id = None;
            }
        });
        Ok(())
    }

    pub fn set_device_profile(&self, device: &str, preset_id: Option<String>) {
        self.store.mutate(|config| match preset_id {
            Some(id) => {
                config
                    .settings
                    .device_profiles
                    .insert(device.to_string(), id);
            }
            None => {
                config.settings.device_profiles.remove(device);
            }
        });
    }

    pub fn start_engine(
        &self,
        mode: &str,
        wav_path: Option<String>,
    ) -> Result<EngineInfo, String> {
        let _op = self.route_op.lock();
        self.start_engine_inner(mode, wav_path)
    }

    fn start_engine_inner(
        &self,
        mode: &str,
        wav_path: Option<String>,
    ) -> Result<EngineInfo, String> {
        if mode != "route" {
            self.auto_route_pending.store(false, Ordering::Relaxed);
        }
        let snapshot = self.store.snapshot();
        let settings = snapshot.settings.clone();

        if mode == "route" {
            let output = settings.output_device.as_deref().ok_or_else(|| {
                "Selecione o fone como saída em Ajustes → Áudio para iniciar a rota.".to_string()
            })?;
            if devices::is_virtual_name(output) {
                return Err("A saída do cabo virtual não pode ser o destino: selecione o \
                            fone em Ajustes → Áudio."
                    .into());
            }
        } else {
            // Previews replace the route; give system audio back to the
            // physical output before opening the stream, otherwise a
            // "system default" output would resolve to the cable itself.
            self.release_system_default();
        }

        if mode == "route" && settings.device_profiles_enabled {
            let output_key = settings
                .output_device
                .clone()
                .or_else(|| self.devices.read().default_output.clone());
            if let Some(key) = output_key {
                if let Some(preset_id) = settings.device_profiles.get(&key) {
                    if snapshot.active_preset_id.as_deref() != Some(preset_id.as_str()) {
                        if let Err(err) = self.apply_preset(preset_id) {
                            self.push_warning(err);
                        }
                    }
                }
            }
        }

        let output = settings.output_device.clone();
        let buffer_ms = settings.buffer_ms;
        let request = match mode {
            "route" => EngineStart::Route {
                input: settings.input_device.clone(),
                output,
                buffer_ms,
            },
            "noise" => EngineStart::Preview {
                output,
                buffer_ms,
                source: PreviewSource::Noise,
            },
            "sweep" => EngineStart::Preview {
                output,
                buffer_ms,
                source: PreviewSource::Sweep,
            },
            "wav" => EngineStart::Preview {
                output,
                buffer_ms,
                source: PreviewSource::Track(
                    wav_path.ok_or_else(|| "Selecione um arquivo WAV primeiro.".to_string())?,
                ),
            },
            other => return Err(format!("Modo de áudio desconhecido: {other}")),
        };
        if let Err(err) = self.engine.start(request.clone()) {
            self.release_system_default();
            return Err(err);
        }
        *self.last_start.lock() = Some(request);
        self.auto_route_pending.store(false, Ordering::Relaxed);
        if mode == "route" {
            self.clear_route_warnings();
        }
        if mode == "route" {
            self.engage_system_default();
        }
        self.pin_output_level();
        Ok(self.engine.info())
    }

    pub fn stop_engine(&self) -> EngineInfo {
        let _op = self.route_op.lock();
        self.auto_route_pending.store(false, Ordering::Relaxed);
        *self.last_start.lock() = None;
        self.engine.stop();
        self.release_system_default();
        self.engine.info()
    }

    /// Sends every system sound through the virtual cable while the route is
    /// running; without this, apps keep playing straight to the headphone and
    /// never reach the DSP chain.
    fn engage_system_default(&self) {
        if !self.engine.info().running {
            return;
        }
        let _engage = self.engage_lock.lock();
        let devices = self.devices.read().clone();
        let Some(cable) = system_default::pick_cable_output(&devices) else {
            return;
        };
        let cable_is_default = system_volume::read_default()
            .map(|default| default.name.eq_ignore_ascii_case(&cable))
            .unwrap_or(false);
        let already = self.system_default.lock().is_some();
        if already && cable_is_default {
            return;
        }
        if already {
            // Windows moved the default. Drop the stale restore target so the
            // new engage() snapshots whatever the user is hearing right now.
            let _ = self.system_default.lock().take();
        }
        let sync = self.volume_sync.lock().enabled;
        let master = self.params().volume.clamp(0.0, 1.0);
        let cable_volume = if sync { master } else { 1.0 };
        let output = self.store.snapshot().settings.output_device;
        match system_default::engage(&cable, output.as_deref(), cable_volume, sync) {
            Ok(Some(engaged)) => {
                log::info!("system audio routed through {}", engaged.cable.name);
                if sync {
                    let mut state = self.volume_sync.lock();
                    state.last_endpoint = Some(engaged.cable.id.clone());
                    state.last_written = Some(master);
                    state.suppress_until = Some(Instant::now() + Duration::from_millis(400));
                }
                *self.system_default.lock() = Some(engaged);
            }
            Ok(None) => {}
            Err(err) => self.push_warning(format!(
                "Áudio do sistema: {err} Se necessário, selecione o cabo como saída padrão \
                 nas configurações de som do Windows."
            )),
        }
    }

    pub fn release_system_default(&self) {
        let _engage = self.engage_lock.lock();
        let engaged = self.system_default.lock().take();
        let Some(engaged) = engaged else {
            return;
        };
        let volume = engaged
            .sync_volume
            .then(|| self.params().volume.clamp(0.0, 1.0));
        match system_default::restore(&engaged, volume) {
            Ok(()) => {
                let mut sync = self.volume_sync.lock();
                sync.last_endpoint = Some(engaged.previous.id.clone());
                if let Some(volume) = volume {
                    sync.last_written = Some(volume);
                    sync.suppress_until = Some(Instant::now() + Duration::from_millis(400));
                }
            }
            Err(err) => self.push_warning(format!("Áudio do sistema: {err}")),
        }
    }

    /// A fresh install starts with a flat curve and no preset, which sounds
    /// exactly like no processing at all.
    pub fn ensure_default_preset(&self) {
        let snapshot = self.store.snapshot();
        let active_matches = match snapshot.active_preset_id.as_deref() {
            Some(id) => find_builtin(id)
                .or_else(|| {
                    snapshot
                        .custom_presets
                        .iter()
                        .find(|preset| preset.id == id)
                        .cloned()
                })
                .map(|preset| same_shaping(&preset.params, &snapshot.params))
                .unwrap_or(false),
            None => false,
        };
        if snapshot.active_preset_id.is_some() && !active_matches {
            self.store.mutate(|config| config.active_preset_id = None);
        }
        let snapshot = self.store.snapshot();
        if snapshot.active_preset_id.is_some() {
            return;
        }
        if snapshot.params.is_neutral() {
            if let Err(err) = self.apply_preset("music") {
                self.push_warning(err);
            }
            return;
        }
        // A null id with a curve identical to a built-in preset means the id
        // was lost (older config or an interrupted save); show the preset
        // instead of an untitled custom curve.
        let matched = builtin_presets()
            .into_iter()
            .find(|preset| same_shaping(&preset.params, &snapshot.params));
        if let Some(preset) = matched {
            self.store
                .mutate(|config| config.active_preset_id = Some(preset.id));
        }
    }

    /// Route mode is the whole point of the app: when cables and devices are
    /// already configured, start it with the window instead of waiting for a
    /// click that many users never make.
    pub fn auto_start_route(&self) {
        let _op = self.route_op.lock();
        self.auto_route_pending.store(true, Ordering::Relaxed);
        self.try_auto_route();
    }

    /// Keeps the route alive across headset plug/unplug and Windows default
    /// changes. Called by a slow thread; route lifecycle operations are
    /// serialized by `route_op`.
    pub fn supervise_route(&self, app: &AppHandle) {
        let Some(_op) = self.route_op.try_lock() else {
            return;
        };
        let before = self.engine.info();
        let warnings_before = self.warnings.lock().len();

        if self.auto_route_pending.load(Ordering::Relaxed) {
            self.try_auto_route();
        }
        let route_requested = matches!(*self.last_start.lock(), Some(EngineStart::Route { .. }));
        if !route_requested {
            if self.system_default.lock().is_some() {
                self.release_system_default();
            }
        } else {
            let info = self.engine.info();
            if info.running && info.mode == "route" {
                self.follow_system_default(&info);
            } else {
                self.recover_route();
            }
        }

        let after = self.engine.info();
        let warnings_after = self.warnings.lock().len();
        if before.running != after.running
            || before.mode != after.mode
            || before.output_device != after.output_device
            || before.error != after.error
            || warnings_before != warnings_after
        {
            emit_view(app, self);
        }
    }

    fn try_auto_route(&self) {
        if self.engine.info().running || !self.auto_route_pending.load(Ordering::Relaxed) {
            return;
        }
        let devices = self.devices.read().clone();
        if !devices.virtual_cable_detected {
            return;
        }
        let settings = self.store.snapshot().settings;
        let Some(output) = pick_route_output(&devices, settings.output_device.as_deref()) else {
            return;
        };
        let Some(input) = pick_route_input(&devices, settings.input_device.as_deref()) else {
            return;
        };
        self.apply_route_devices(&input, &output);
        if let Err(err) = self.start_engine_inner("route", None) {
            log::debug!("automatic route waiting: {err}");
        }
    }

    fn recover_route(&self) {
        let now = Instant::now();
        {
            let mut last = self.last_recovery.lock();
            if last.is_some_and(|at| now.duration_since(at) < ROUTE_RECOVERY_INTERVAL) {
                return;
            }
            *last = Some(now);
        }

        let devices = self.refresh_devices();
        let settings = self.store.snapshot().settings;
        if system_default::pick_cable_output(&devices).is_none() {
            self.release_system_default();
            self.push_warning(ROUTE_INPUT_WARNING.into());
            return;
        }
        let output = pick_route_output(&devices, settings.output_device.as_deref());
        let input = pick_route_input(&devices, settings.input_device.as_deref());
        let (Some(output), Some(input)) = (output, input) else {
            self.release_system_default();
            if devices.virtual_cable_detected {
                self.push_warning(ROUTE_OUTPUT_WARNING.into());
            } else {
                self.push_warning(ROUTE_INPUT_WARNING.into());
            }
            return;
        };
        self.apply_route_devices(&input, &output);
        match self.start_engine_inner("route", None) {
            Ok(_) => {
                log::info!("route recovered with output {output}");
                self.clear_route_warnings();
            }
            Err(err) => {
                self.release_system_default();
                self.push_warning(format!("A rota foi interrompida: {err}"));
            }
        }
    }

    fn follow_system_default(&self, info: &EngineInfo) {
        let devices = self.devices.read().clone();
        let output_present = info.output_device.as_deref().is_some_and(|output| {
            devices
                .outputs
                .iter()
                .any(|device| device.name == output && !device.is_virtual)
        });
        let Some(cable) = system_default::pick_cable_output(&devices) else {
            // The cable vanished; recover now instead of waiting for an error
            // callback that may never fire.
            self.engine.stop();
            *self.last_recovery.lock() = None;
            self.recover_route();
            return;
        };
        if !output_present {
            self.engine.stop();
            *self.last_recovery.lock() = None;
            self.recover_route();
            return;
        }
        let Ok(default) = system_volume::read_default() else {
            return;
        };
        if default.name.eq_ignore_ascii_case(&cable) {
            return;
        }
        if !devices::is_virtual_name(&default.name)
            && info.output_device.as_deref() != Some(default.name.as_str())
            && self.adopt_output(&default.name)
        {
            return;
        }
        self.engage_system_default();
    }

    /// Windows moved the default to a physical output, typically because a
    /// headset just connected. Switch the route to it before taking the
    /// default back to the cable.
    fn adopt_output(&self, name: &str) -> bool {
        if devices::is_virtual_name(name) {
            return false;
        }
        let present = self
            .devices
            .read()
            .outputs
            .iter()
            .any(|device| device.name == name && !device.is_virtual);
        if !present {
            return false;
        }
        if self.store.snapshot().settings.output_device.as_deref() == Some(name) {
            return false;
        }
        self.store
            .mutate(|config| config.settings.output_device = Some(name.to_string()));
        log::info!("route output follows the system default: {name}");
        match self.start_engine_inner("route", None) {
            Ok(_) => true,
            Err(err) => {
                self.push_warning(format!("Não foi possível usar \"{name}\" como saída: {err}"));
                false
            }
        }
    }

    fn apply_route_devices(&self, input: &str, output: &str) -> bool {
        let mut changed = false;
        self.store.mutate(|config| {
            if config.settings.input_device.as_deref() != Some(input) {
                config.settings.input_device = Some(input.to_string());
                changed = true;
            }
            if config.settings.output_device.as_deref() != Some(output) {
                config.settings.output_device = Some(output.to_string());
                changed = true;
            }
        });
        changed
    }

    fn clear_route_warnings(&self) {
        let mut warnings = self.warnings.lock();
        warnings
            .retain(|warning| warning != ROUTE_OUTPUT_WARNING && warning != ROUTE_INPUT_WARNING);
    }

    /// Restarts the running engine when a device or buffer setting changes.
    pub fn restart_if_needed(&self, previous: &Settings, next: &Settings) {
        let _op = self.route_op.lock();
        let changed = previous.output_device != next.output_device
            || previous.input_device != next.input_device
            || previous.buffer_ms != next.buffer_ms;
        if !changed || !self.engine.info().running {
            return;
        }
        let Some(last) = self.last_start.lock().clone() else {
            return;
        };
        let request = match last {
            EngineStart::Route { .. } => {
                let output_valid = next
                    .output_device
                    .as_deref()
                    .map(|name| !devices::is_virtual_name(name))
                    .unwrap_or(false);
                if !output_valid {
                    self.engine.stop();
                    self.release_system_default();
                    self.push_warning(
                        "A rota foi interrompida: selecione o fone como saída em Ajustes → Áudio."
                            .into(),
                    );
                    return;
                }
                EngineStart::Route {
                    input: next.input_device.clone(),
                    output: next.output_device.clone(),
                    buffer_ms: next.buffer_ms,
                }
            }
            EngineStart::Preview { source, .. } => EngineStart::Preview {
                output: next.output_device.clone(),
                buffer_ms: next.buffer_ms,
                source,
            },
        };
        match self.engine.start(request.clone()) {
            Ok(()) => {
                *self.last_start.lock() = Some(request);
                self.auto_route_pending.store(false, Ordering::Relaxed);
                self.clear_route_warnings();
                self.pin_output_level();
            }
            Err(err) => {
                self.release_system_default();
                self.push_warning(format!("Não foi possível reiniciar o motor: {err}"));
            }
        }
    }

    fn current_tray_signature(&self) -> String {
        let snapshot = self.store.snapshot();
        let spatial = match snapshot.params.spatial.mode {
            SpatialMode::Off => "off",
            SpatialMode::Binaural => "binaural",
            SpatialMode::Hrtf => "hrtf",
            SpatialMode::Surround => "surround",
        };
        format!(
            "{}|{}|{}|{}",
            snapshot.params.bypass,
            spatial,
            snapshot.settings.resolved_language(),
            snapshot.active_preset_id.as_deref().unwrap_or("")
        )
    }

    pub fn sync_tray(&self, app: &AppHandle) {
        let signature = self.current_tray_signature();
        let mut last = self.tray_signature.lock();
        if *last == signature {
            return;
        }
        *last = signature;
        drop(last);
        let _ = tray::refresh(app, self);
    }
}

pub fn emit_view(app: &AppHandle, state: &AppState) {
    let _ = app.emit("state-changed", state.view());
    state.sync_tray(app);
}

/// Factory sound: the Music preset, no bypass and no ear profile. Master
/// volume is Windows-owned, so the caller passes the current value back in.
fn factory_sound_params(volume: f32) -> Result<(String, ProcessingParams), String> {
    let preset = find_builtin("music").ok_or("Preset padrão não encontrado.")?;
    let mut params = ProcessingParams::default();
    params.apply_processing_from(&preset.params);
    params.volume = volume;
    params.bypass = false;
    params.ear_profile = EarProfileParams::default();
    Ok((preset.id.clone(), params.sanitized()))
}

/// Compares only the sound-shaping fields; master volume, bypass and the ear
/// profile remain user-owned and never invalidate the selected preset.
fn same_shaping(left: &ProcessingParams, right: &ProcessingParams) -> bool {
    let mut left = left.clone();
    let mut right = right.clone();
    left.volume = 0.0;
    right.volume = 0.0;
    left.bypass = false;
    right.bypass = false;
    left.ear_profile = EarProfileParams::default();
    right.ear_profile = EarProfileParams::default();
    left == right
}

/// The physical output the route should feed: the saved choice while it
/// exists, otherwise the Windows default, otherwise the first physical output.
fn pick_route_output(devices: &DeviceList, preferred: Option<&str>) -> Option<String> {
    let physical = |name: &str| {
        devices
            .outputs
            .iter()
            .any(|device| device.name == name && !device.is_virtual)
    };
    if let Some(name) = preferred {
        if physical(name) {
            return Some(name.to_string());
        }
    }
    if let Some(default) = devices.default_output.as_deref() {
        if physical(default) {
            return Some(default.to_string());
        }
    }
    devices
        .outputs
        .iter()
        .find(|device| !device.is_virtual)
        .map(|device| device.name.clone())
}

/// The capture side of a virtual cable; never a microphone.
fn pick_route_input(devices: &DeviceList, preferred: Option<&str>) -> Option<String> {
    let virtual_input = |name: &str| {
        devices
            .inputs
            .iter()
            .any(|device| device.name == name && device.is_virtual)
    };
    if let Some(name) = preferred {
        if virtual_input(name) {
            return Some(name.to_string());
        }
    }
    devices
        .inputs
        .iter()
        .find(|device| device.is_virtual)
        .map(|device| device.name.clone())
}

pub fn show_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Executes a shortcut or tray action by its stable identifier.
pub fn run_action(app: &AppHandle, state: &AppState, action: &str) -> Result<(), String> {
    match action {
        "toggle_bypass" => {
            state.toggle_bypass();
        }
        "toggle_spatial" => {
            state.cycle_spatial();
        }
        "preset_music" => state.apply_preset("music")?,
        "preset_cinema" => state.apply_preset("cinema")?,
        "preset_gaming" => state.apply_preset("gaming")?,
        "preset_podcast" => state.apply_preset("podcast")?,
        "volume_up" => {
            let volume = (state.params().volume + 0.05).min(1.0);
            state.set_volume(volume);
        }
        "volume_down" => {
            let volume = (state.params().volume - 0.05).max(0.0);
            state.set_volume(volume);
        }
        "open_window" => {
            show_window(app);
            return Ok(());
        }
        other => return Err(format!("Ação desconhecida: {other}")),
    }
    let _ = app.emit("state-changed", state.view());
    state.sync_tray(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::DeviceInfo;
    use audio_pulse_dsp::{AutoEqFilter, FilterKind};

    fn device(name: &str, is_default: bool, is_virtual: bool) -> DeviceInfo {
        DeviceInfo {
            name: name.into(),
            is_default,
            is_virtual,
        }
    }

    fn device_list(
        outputs: Vec<DeviceInfo>,
        inputs: Vec<DeviceInfo>,
        default_output: Option<&str>,
    ) -> DeviceList {
        DeviceList {
            outputs,
            inputs,
            default_output: default_output.map(str::to_string),
            ..DeviceList::default()
        }
    }

    #[test]
    fn route_output_keeps_the_saved_physical_device() {
        let list = device_list(
            vec![
                device("CABLE Input", false, true),
                device("Headset", true, false),
                device("Speakers", false, false),
            ],
            vec![],
            Some("Headset"),
        );
        assert_eq!(
            pick_route_output(&list, Some("Speakers")).as_deref(),
            Some("Speakers")
        );
    }

    #[test]
    fn route_output_never_uses_the_cable() {
        let list = device_list(
            vec![
                device("CABLE Input", true, true),
                device("Speakers", false, false),
            ],
            vec![],
            Some("CABLE Input"),
        );
        assert_eq!(
            pick_route_output(&list, Some("CABLE Input")).as_deref(),
            Some("Speakers")
        );
    }

    #[test]
    fn route_output_falls_back_to_the_windows_default() {
        let list = device_list(
            vec![
                device("Headset", false, false),
                device("Speakers", true, false),
            ],
            vec![],
            Some("Speakers"),
        );
        assert_eq!(pick_route_output(&list, None).as_deref(), Some("Speakers"));
    }

    #[test]
    fn route_output_disappears_without_a_physical_device() {
        let list = device_list(vec![device("CABLE Input", true, true)], vec![], Some("CABLE Input"));
        assert_eq!(pick_route_output(&list, Some("Headset")), None);
    }

    #[test]
    fn route_input_never_picks_a_microphone() {
        let list = device_list(
            vec![],
            vec![
                device("Microphone", true, false),
                device("CABLE Output", false, true),
            ],
            None,
        );
        assert_eq!(
            pick_route_input(&list, Some("Microphone")).as_deref(),
            Some("CABLE Output")
        );
    }

    #[test]
    fn route_input_requires_a_virtual_cable() {
        let list = device_list(vec![], vec![device("Microphone", true, false)], None);
        assert_eq!(pick_route_input(&list, None), None);
    }

    #[test]
    fn protection_toggle_is_part_of_the_preset_shape() {
        let preset = find_builtin("protection").unwrap().params;
        let mut current = ProcessingParams::default();
        current.apply_processing_from(&preset);
        assert!(current.loud_sound_reduction.enabled);
        assert!(same_shaping(&preset, &current));

        current.loud_sound_reduction.enabled = false;
        assert!(
            !same_shaping(&preset, &current),
            "disabling hearing protection must stop matching the Protection preset"
        );
    }

    #[test]
    fn factory_sound_is_the_music_preset_without_an_ear_profile() {
        let music = find_builtin("music").unwrap().params;
        let (preset_id, params) = factory_sound_params(0.115).unwrap();
        assert_eq!(preset_id, "music");
        assert!((params.volume - 0.115).abs() < 1e-6, "volume is Windows-owned");
        assert!(!params.bypass);
        assert!(!params.ear_profile.enabled);
        assert!(params.ear_profile.filters.is_empty());
        assert!(same_shaping(&music, &params));
    }

    #[test]
    fn ear_profile_never_breaks_preset_matching() {
        let preset = find_builtin("music").unwrap().params;
        let mut current = ProcessingParams::default();
        current.apply_processing_from(&preset);
        current.ear_profile = EarProfileParams {
            enabled: true,
            preamp_db: -1.0,
            filters: vec![AutoEqFilter {
                kind: FilterKind::Peaking,
                freq: 3_500.0,
                gain_db: -2.0,
                q: 1.15,
            }],
        };
        assert!(
            same_shaping(&preset, &current),
            "the ear profile is independent from the selected preset"
        );
    }
}