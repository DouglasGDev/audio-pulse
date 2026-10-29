import {
  EMPTY_TELEMETRY,
  type AppView,
  type DeviceList,
  type EarModelStatus,
  type EngineInfo,
  type EngineMode,
  type ParamsUpdate,
  type Preset,
  type ProcessingParams,
  type Settings,
  type Telemetry,
} from "./types";
import { defaultParams, MOCK_PRESETS } from "./presetFixtures";

export function defaultSettings(): Settings {
  return {
    language: "",
    theme: "dark",
    autostart: false,
    closeToTray: true,
    launchMinimized: false,
    bufferMs: 25,
    deviceProfilesEnabled: true,
    inputDevice: null,
    outputDevice: null,
    shortcuts: {
      toggle_bypass: "CmdOrCtrl+Alt+B",
      preset_music: "CmdOrCtrl+Alt+1",
      preset_cinema: "CmdOrCtrl+Alt+2",
      preset_gaming: "CmdOrCtrl+Alt+3",
      preset_podcast: "CmdOrCtrl+Alt+4",
      toggle_spatial: "CmdOrCtrl+Alt+S",
      volume_up: "CmdOrCtrl+Alt+Up",
      volume_down: "CmdOrCtrl+Alt+Down",
      open_window: "CmdOrCtrl+Alt+O",
    },
    deviceProfiles: {},
  };
}

const mockDevices: DeviceList = {
  inputs: [
    { name: "CABLE Output (VB-Audio Virtual Cable)", isDefault: true, isVirtual: true },
    { name: "Microfone (Realtek Audio)", isDefault: false, isVirtual: false },
  ],
  outputs: [
    { name: "Fones de ouvido (Realtek Audio)", isDefault: true, isVirtual: false },
    { name: "CABLE Input (VB-Audio Virtual Cable)", isDefault: false, isVirtual: true },
    { name: "Alto-falantes (Realtek Audio)", isDefault: false, isVirtual: false },
  ],
  defaultInput: "CABLE Output (VB-Audio Virtual Cable)",
  defaultOutput: "Fones de ouvido (Realtek Audio)",
  virtualCableDetected: true,
  platform: "web",
};

type Listener<T> = (value: T) => void;

const stateListeners = new Set<Listener<AppView>>();
const telemetryListeners = new Set<Listener<Telemetry>>();
const deviceListeners = new Set<Listener<DeviceList>>();

interface MockState {
  params: ProcessingParams;
  settings: Settings;
  presets: Preset[];
  activePresetId: string | null;
  activeHeadphoneId: string | null;
  devices: DeviceList;
  engine: EngineInfo;
  warnings: string[];
}

function clone<T>(value: T): T {
  return structuredClone(value);
}

function initialEngine(): EngineInfo {
  return {
    running: true,
    mode: "route",
    sampleRate: 48000,
    inputDevice: "CABLE Output (VB-Audio Virtual Cable)",
    outputDevice: "Fones de ouvido (Realtek Audio)",
    bufferMs: 25,
    latencyMs: 30,
    cpuPercent: 7.4,
    xruns: 0,
    error: null,
  };
}

const music = MOCK_PRESETS[0];
const state: MockState = {
  params: { ...clone(music.params), volume: 0.72 },
  settings: defaultSettings(),
  presets: clone(MOCK_PRESETS),
  activePresetId: "music",
  activeHeadphoneId: null,
  devices: clone(mockDevices),
  engine: initialEngine(),
  warnings: [],
};

function shapingEqual(a: ProcessingParams, b: ProcessingParams): boolean {
  const left = clone(a);
  const right = clone(b);
  left.volume = 0;
  right.volume = 0;
  left.bypass = false;
  right.bypass = false;
  left.earProfile = { enabled: false, preampDb: 0, filters: [] };
  right.earProfile = { enabled: false, preampDb: 0, filters: [] };
  left.autoEq = { enabled: true, preampDb: 0, filters: [], name: null };
  right.autoEq = { enabled: true, preampDb: 0, filters: [], name: null };
  return JSON.stringify(left) === JSON.stringify(right);
}

function headphoneEqual(a: ProcessingParams, b: ProcessingParams): boolean {
  const left = clone(a.autoEq);
  const right = clone(b.autoEq);
  left.preampDb = 0;
  right.preampDb = 0;
  return JSON.stringify(left) === JSON.stringify(right);
}

function syncActiveLayers() {
  if (state.activePresetId) {
    const preset = state.presets.find((p) => p.id === state.activePresetId);
    if (!preset || !shapingEqual(preset.params, state.params)) {
      state.activePresetId = null;
    }
  }
  if (state.activeHeadphoneId) {
    const preset = state.presets.find((p) => p.id === state.activeHeadphoneId);
    if (!preset || !headphoneEqual(preset.params, state.params)) {
      state.activeHeadphoneId = null;
    }
  }
}

function currentView(): AppView {
  return {
    params: clone(state.params),
    settings: clone(state.settings),
    presets: clone(state.presets),
    activePresetId: state.activePresetId,
    activeHeadphoneId: state.activeHeadphoneId,
    devices: clone(state.devices),
    engine: clone(state.engine),
    battery: null,
    warnings: [...state.warnings],
    version: "1.0.0",
  };
}

function emitState() {
  for (const listener of stateListeners) {
    listener(currentView());
  }
}

function latencyFor(bufferMs: number, route: boolean): number {
  return (route ? bufferMs * 3 : bufferMs) + 5;
}

function engineModeLabel(mode: EngineMode): string {
  switch (mode) {
    case "route":
      return "route";
    case "noise":
      return "preview-noise";
    case "sweep":
      return "preview-sweep";
    case "wav":
      return "preview-wav";
  }
}

export const mockBackend = {
  getState(): AppView {
    return currentView();
  },

  setParams(params: ProcessingParams): ParamsUpdate {
    state.params = clone(params);
    syncActiveLayers();
    emitState();
    return {
      params: clone(state.params),
      activePresetId: state.activePresetId,
      activeHeadphoneId: state.activeHeadphoneId,
    };
  },

  setVolume(volume: number): ParamsUpdate {
    state.params.volume = Math.min(1, Math.max(0, volume));
    emitState();
    return {
      params: clone(state.params),
      activePresetId: state.activePresetId,
      activeHeadphoneId: state.activeHeadphoneId,
    };
  },

  toggleBypass(): ParamsUpdate {
    state.params.bypass = !state.params.bypass;
    emitState();
    return {
      params: clone(state.params),
      activePresetId: state.activePresetId,
      activeHeadphoneId: state.activeHeadphoneId,
    };
  },

  setSpatialMode(mode: ProcessingParams["spatial"]["mode"]): ParamsUpdate {
    state.params.spatial.mode = mode;
    syncActiveLayers();
    emitState();
    return {
      params: clone(state.params),
      activePresetId: state.activePresetId,
      activeHeadphoneId: state.activeHeadphoneId,
    };
  },

  applyPreset(id: string): AppView {
    const preset = state.presets.find((p) => p.id === id);
    if (!preset) {
      throw new Error(`Preset não encontrado: ${id}`);
    }
    const headphone = preset.category === "headphone";
    const params = clone(state.params);
    if (headphone) {
      params.autoEq = clone(preset.params.autoEq);
      state.activeHeadphoneId = id;
    } else {
      const applied = clone(preset.params);
      applied.volume = state.params.volume;
      applied.bypass = state.params.bypass;
      applied.earProfile = clone(state.params.earProfile);
      applied.autoEq = clone(state.params.autoEq);
      Object.assign(params, applied);
      state.activePresetId = id;
    }
    state.params = params;
    emitState();
    return currentView();
  },

  restoreSoundDefaults(): AppView {
    const preset = state.presets.find((p) => p.id === "music");
    const params = clone(preset ? preset.params : defaultParams());
    params.volume = state.params.volume;
    params.bypass = false;
    params.earProfile = { enabled: false, preampDb: 0, filters: [] };
    params.autoEq = { enabled: true, preampDb: 0, filters: [], name: null };
    state.params = params;
    state.activePresetId = "music";
    state.activeHeadphoneId = null;
    emitState();
    return currentView();
  },

  savePreset(name: string): AppView {
    const params = clone(state.params);
    params.earProfile = { enabled: false, preampDb: 0, filters: [] };
    params.autoEq = { enabled: true, preampDb: 0, filters: [], name: null };
    const preset: Preset = {
      id: `custom-${Date.now()}`,
      name: name.trim() || "Preset",
      category: "custom",
      description: "Preset personalizado.",
      brand: null,
      model: null,
      builtIn: false,
      params,
    };
    state.presets.push(preset);
    state.activePresetId = preset.id;
    emitState();
    return currentView();
  },

  deletePreset(id: string): AppView {
    state.presets = state.presets.filter((p) => !(p.id === id && !p.builtIn));
    if (state.activePresetId === id) {
      state.activePresetId = null;
    }
    if (state.activeHeadphoneId === id) {
      state.activeHeadphoneId = null;
    }
    emitState();
    return currentView();
  },

  updateSettings(settings: Settings): AppView {
    const previous = state.settings;
    state.settings = clone(settings);
    if (
      state.engine.running &&
      (previous.bufferMs !== settings.bufferMs ||
        previous.outputDevice !== settings.outputDevice ||
        previous.inputDevice !== settings.inputDevice)
    ) {
      state.engine.bufferMs = settings.bufferMs;
      state.engine.latencyMs = latencyFor(settings.bufferMs, state.engine.mode === "route");
    }
    emitState();
    return currentView();
  },

  setDeviceProfile(device: string, presetId: string | null): AppView {
    if (presetId) {
      state.settings.deviceProfiles[device] = presetId;
    } else {
      delete state.settings.deviceProfiles[device];
    }
    emitState();
    return currentView();
  },

  refreshDevices(): DeviceList {
    state.devices = clone(mockDevices);
    for (const listener of deviceListeners) {
      listener(clone(state.devices));
    }
    return clone(state.devices);
  },

  installVirtualCable(): AppView {
    throw new Error("Disponível apenas no aplicativo.");
  },

  startEngine(mode: EngineMode): EngineInfo {
    const route = mode === "route";
    state.engine = {
      ...state.engine,
      running: true,
      mode: engineModeLabel(mode),
      sampleRate: 48000,
      inputDevice: route ? state.devices.defaultInput : null,
      outputDevice: state.settings.outputDevice ?? state.devices.defaultOutput,
      bufferMs: state.settings.bufferMs,
      latencyMs: latencyFor(state.settings.bufferMs, route),
      cpuPercent: route ? 7.4 : 4.2,
      xruns: 0,
      error: null,
    };
    emitState();
    return clone(state.engine);
  },

  stopEngine(): EngineInfo {
    state.engine = {
      ...state.engine,
      running: false,
      mode: "stopped",
      sampleRate: 0,
      inputDevice: null,
      latencyMs: 0,
      cpuPercent: 0,
    };
    emitState();
    return clone(state.engine);
  },

  importPreset(): AppView {
    throw new Error("Disponível apenas no aplicativo.");
  },

  importAutoEq(): AppView {
    throw new Error("Disponível apenas no aplicativo.");
  },

  exportPreset(): string {
    throw new Error("Disponível apenas no aplicativo.");
  },

  earModelStatus(): EarModelStatus {
    return {
      installed: false,
      bytes: 0,
      expectedBytes: 99_060_839,
      name: "Depth Anything V2 Small",
      license: "Apache-2.0",
    };
  },

  onStateChanged(listener: Listener<AppView>): () => void {
    stateListeners.add(listener);
    return () => stateListeners.delete(listener);
  },

  onDevicesChanged(listener: Listener<DeviceList>): () => void {
    deviceListeners.add(listener);
    return () => deviceListeners.delete(listener);
  },

  onTelemetry(listener: Listener<Telemetry>): () => void {
    telemetryListeners.add(listener);
    if (!telemetryTimer) {
      telemetryTimer = window.setInterval(publishTelemetry, 33);
    }
    return () => {
      telemetryListeners.delete(listener);
      if (telemetryListeners.size === 0 && telemetryTimer) {
        window.clearInterval(telemetryTimer);
        telemetryTimer = null;
      }
    };
  },
};

let telemetryTimer: number | null = null;
const smoothed = new Array(56).fill(0);

function publishTelemetry() {
  if (telemetryListeners.size === 0) {
    return;
  }
  const time = performance.now() / 1000;
  const running = state.engine.running;
  const bypassed = state.params.bypass;
  const energy = running ? 0.55 + state.params.volume * 0.4 : 0;
  const telemetry: Telemetry = {
    ...EMPTY_TELEMETRY,
    bands: EMPTY_TELEMETRY.bands.slice(),
    adaptiveGainsDb: EMPTY_TELEMETRY.adaptiveGainsDb.slice(),
  };

  for (let index = 0; index < 56; index += 1) {
    const freq = 30 * Math.pow(600, index / 55);
    let level = 0;
    if (running) {
      const low = 0.6 * Math.exp(-Math.pow(Math.log(freq / 90) / 0.7, 2));
      const mid = 0.42 * Math.exp(-Math.pow(Math.log(freq / 900) / 0.6, 2));
      const high = 0.3 * Math.exp(-Math.pow(Math.log(freq / 7000) / 0.55, 2));
      const shimmer =
        0.08 * Math.sin(time * 2.3 + index * 0.7) +
        0.05 * Math.sin(time * 5.1 - index * 1.3);
      level = Math.max(0, (low + mid + high) * energy + shimmer * energy);
      if (bypassed) {
        level *= 0.9;
      }
    }
    smoothed[index] = smoothed[index] * 0.62 + level * 0.38;
    telemetry.bands[index] = Math.min(1, Math.max(0, smoothed[index]));
  }

  if (running) {
    telemetry.rmsL = 0.12 * energy + 0.02 * Math.sin(time * 1.7);
    telemetry.rmsR = 0.12 * energy + 0.02 * Math.cos(time * 1.9);
    telemetry.peakL = 0.38 * energy;
    telemetry.peakR = 0.36 * energy;
    telemetry.compressorGrDb =
      state.params.compressor.enabled ? 1.2 + Math.sin(time * 0.8) * 1.1 : 0;
    const protection = state.params.loudSoundReduction;
    telemetry.protectionGrDb =
      protection.enabled && protection.intensity > 0.01
        ? (0.8 + Math.abs(Math.sin(time * 1.1)) * 2.4) * protection.intensity
        : 0;
    telemetry.limiterGrDb = 0.4 + Math.sin(time * 1.4) * 0.35;
    if (state.params.adaptiveEq.enabled) {
      const intensity = state.params.adaptiveEq.intensity;
      telemetry.adaptiveGainsDb = [-2.1, -0.4, 1.1, -1.3, 0.7].map(
        (base, index) => (base + Math.sin(time * 0.9 + index * 1.4) * 0.6) * intensity,
      );
    } else {
      telemetry.adaptiveGainsDb = EMPTY_TELEMETRY.adaptiveGainsDb.slice();
    }
    telemetry.cpuPercent = state.engine.cpuPercent + Math.sin(time * 0.6) * 0.8;
    telemetry.latencyMs = state.engine.latencyMs;
    telemetry.xruns = state.engine.xruns;
  }
  telemetry.running = running;
  telemetry.sampleRate = state.engine.sampleRate;

  for (const listener of telemetryListeners) {
    listener(telemetry);
  }
}