import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";

import { mockBackend } from "./mock";
import type {
  AppView,
  DeviceList,
  EarAnalysis,
  EarModelProgress,
  EarModelStatus,
  EarPhoto,
  EngineInfo,
  EngineMode,
  ParamsUpdate,
  ProcessingParams,
  Settings,
  SpatialMode,
  Telemetry,
} from "./types";

export const isTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

type Listener<T> = (value: T) => void;

function subscribe<T>(
  event: string,
  listener: Listener<T>,
  mock: (listener: Listener<T>) => () => void,
): () => void {
  if (!isTauri) {
    return mock(listener);
  }
  let disposed = false;
  let unlisten: UnlistenFn | undefined;
  listen<T>(event, (payload) => listener(payload.payload)).then((fn) => {
    if (disposed) {
      fn();
    } else {
      unlisten = fn;
    }
  });
  return () => {
    disposed = true;
    unlisten?.();
  };
}

export const api = {
  getState(): Promise<AppView> {
    return isTauri ? invoke<AppView>("get_state") : Promise.resolve(mockBackend.getState());
  },

  setParams(params: ProcessingParams): Promise<ParamsUpdate> {
    return isTauri
      ? invoke<ParamsUpdate>("set_params", { params })
      : Promise.resolve(mockBackend.setParams(params));
  },

  setVolume(volume: number): Promise<ParamsUpdate> {
    return isTauri
      ? invoke<ParamsUpdate>("set_volume", { volume })
      : Promise.resolve(mockBackend.setVolume(volume));
  },

  toggleBypass(): Promise<ParamsUpdate> {
    return isTauri
      ? invoke<ParamsUpdate>("toggle_bypass")
      : Promise.resolve(mockBackend.toggleBypass());
  },

  setSpatialMode(mode: SpatialMode): Promise<ParamsUpdate> {
    return isTauri
      ? invoke<ParamsUpdate>("set_spatial_mode", { mode })
      : Promise.resolve(mockBackend.setSpatialMode(mode));
  },

  applyPreset(id: string): Promise<AppView> {
    return isTauri
      ? invoke<AppView>("apply_preset", { id })
      : Promise.resolve(mockBackend.applyPreset(id));
  },

  savePreset(name: string): Promise<AppView> {
    return isTauri
      ? invoke<AppView>("save_preset", { name })
      : Promise.resolve(mockBackend.savePreset(name));
  },

  deletePreset(id: string): Promise<AppView> {
    return isTauri
      ? invoke<AppView>("delete_preset", { id })
      : Promise.resolve(mockBackend.deletePreset(id));
  },

  restoreSoundDefaults(): Promise<AppView> {
    return isTauri
      ? invoke<AppView>("restore_sound_defaults")
      : Promise.resolve(mockBackend.restoreSoundDefaults());
  },

  exportPreset(id: string, path: string): Promise<string> {
    return isTauri
      ? invoke<string>("export_preset", { id, path })
      : Promise.resolve(mockBackend.exportPreset());
  },

  importPreset(path: string): Promise<AppView> {
    return isTauri
      ? invoke<AppView>("import_preset", { path })
      : Promise.resolve(mockBackend.importPreset());
  },

  importAutoEq(path: string): Promise<AppView> {
    return isTauri
      ? invoke<AppView>("import_autoeq", { path })
      : Promise.resolve(mockBackend.importAutoEq());
  },

  refreshDevices(): Promise<DeviceList> {
    return isTauri
      ? invoke<DeviceList>("refresh_devices")
      : Promise.resolve(mockBackend.refreshDevices());
  },

  installVirtualCable(): Promise<AppView> {
    return isTauri
      ? invoke<AppView>("install_virtual_cable")
      : Promise.resolve(mockBackend.installVirtualCable());
  },

  startEngine(mode: EngineMode, wavPath?: string | null): Promise<EngineInfo> {
    return isTauri
      ? invoke<EngineInfo>("start_engine", { mode, wavPath: wavPath ?? null })
      : Promise.resolve(mockBackend.startEngine(mode));
  },

  stopEngine(): Promise<EngineInfo> {
    return isTauri
      ? invoke<EngineInfo>("stop_engine")
      : Promise.resolve(mockBackend.stopEngine());
  },

  updateSettings(settings: Settings): Promise<AppView> {
    return isTauri
      ? invoke<AppView>("update_settings", { settings })
      : Promise.resolve(mockBackend.updateSettings(settings));
  },

  setDeviceProfile(device: string, presetId: string | null): Promise<AppView> {
    return isTauri
      ? invoke<AppView>("set_device_profile", { device, presetId })
      : Promise.resolve(mockBackend.setDeviceProfile(device, presetId));
  },

  onStateChanged(listener: Listener<AppView>): () => void {
    return subscribe("state-changed", listener, mockBackend.onStateChanged);
  },

  onDevicesChanged(listener: Listener<DeviceList>): () => void {
    return subscribe("devices-changed", listener, mockBackend.onDevicesChanged);
  },

  onTelemetry(listener: Listener<Telemetry>): () => void {
    return subscribe("telemetry", listener, mockBackend.onTelemetry);
  },

  earModelStatus(): Promise<EarModelStatus> {
    return isTauri
      ? invoke<EarModelStatus>("ear_model_status")
      : Promise.resolve(mockBackend.earModelStatus());
  },

  downloadEarModel(): Promise<EarModelStatus> {
    return isTauri
      ? invoke<EarModelStatus>("ear_download_model")
      : Promise.reject(new Error("Disponível apenas no app nativo."));
  },

  loadEarPhoto(path: string): Promise<EarPhoto> {
    return isTauri
      ? invoke<EarPhoto>("ear_load_photo", { path })
      : Promise.reject(new Error("Disponível apenas no app nativo."));
  },

  analyzeEar(imageBase64: string): Promise<EarAnalysis> {
    return isTauri
      ? invoke<EarAnalysis>("ear_analyze", { imageBase64 })
      : Promise.reject(new Error("Disponível apenas no app nativo."));
  },

  onEarModelProgress(listener: Listener<EarModelProgress>): () => void {
    return subscribe("ear-model-progress", listener, () => () => {});
  },
};

export async function pickWavFile(): Promise<string | null> {
  if (!isTauri) return null;
  const picked = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "WAV", extensions: ["wav", "wave"] }],
  });
  return typeof picked === "string" ? picked : null;
}

export async function pickPresetFile(): Promise<string | null> {
  if (!isTauri) return null;
  const picked = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Audio Pulse preset", extensions: ["json"] }],
  });
  return typeof picked === "string" ? picked : null;
}

export async function pickAutoEqFile(): Promise<string | null> {
  if (!isTauri) return null;
  const picked = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "AutoEQ ParametricEQ", extensions: ["txt"] }],
  });
  return typeof picked === "string" ? picked : null;
}

export async function pickEarPhoto(): Promise<string | null> {
  if (!isTauri) return null;
  const picked = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Foto", extensions: ["jpg", "jpeg", "png"] }],
  });
  return typeof picked === "string" ? picked : null;
}

export async function choosePresetExportPath(
  suggestedName: string,
): Promise<string | null> {
  if (!isTauri) return null;
  const picked = await save({
    defaultPath: `${suggestedName.replace(/[^\w\-. ]+/g, "_")}.audiopulse.json`,
    filters: [{ name: "Audio Pulse preset", extensions: ["json"] }],
  });
  return picked ?? null;
}