import {
  EMPTY_TELEMETRY,
  type AppView,
  type AutoEqFilter,
  type DeviceList,
  type EarModelStatus,
  type EngineInfo,
  type EngineMode,
  type ParamsUpdate,
  type Preset,
  type PresetCategory,
  type ProcessingParams,
  type Settings,
  type Telemetry,
} from "./types";

const f = (
  kind: AutoEqFilter["kind"],
  freq: number,
  gainDb: number,
  q: number,
): AutoEqFilter => ({ kind, freq, gainDb, q });

const pk = (freq: number, gainDb: number, q: number) =>
  f("peaking", freq, gainDb, q);
const ls = (freq: number, gainDb: number, q: number) =>
  f("lowshelf", freq, gainDb, q);
const hs = (freq: number, gainDb: number, q: number) =>
  f("highshelf", freq, gainDb, q);

export function defaultParams(): ProcessingParams {
  return {
    preampDb: 0,
    eqDb: new Array(10).fill(0),
    bass: { enabled: false, intensity: 0.45, cutoffHz: 90, harmonics: 0.35 },
    tone: { midDb: 0, trebleDb: 0 },
    compressor: {
      enabled: false,
      thresholdDb: -18,
      ratio: 2,
      attackMs: 10,
      releaseMs: 150,
      kneeDb: 6,
      makeupDb: 0,
    },
    limiter: { enabled: true, ceilingDb: -0.3, releaseMs: 80 },
    loudness: { enabled: false, intensity: 0.6 },
    loudSoundReduction: { enabled: false, intensity: 0.6 },
    spatial: { mode: "off", width: 1, room: 0.35 },
    crossfeed: { enabled: false, amount: 0.3, delayMs: 0.3 },
    autoEq: { enabled: true, preampDb: 0, filters: [] },
    adaptiveEq: { enabled: false, intensity: 0.55 },
    earProfile: { enabled: false, preampDb: 0, filters: [] },
    volume: 1,
    bypass: false,
  };
}

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

function modePreset(
  id: string,
  name: string,
  category: PresetCategory,
  description: string,
  build: (params: ProcessingParams) => void,
): Preset {
  const params = defaultParams();
  params.limiter.enabled = true;
  build(params);
  return {
    id,
    name,
    category,
    description,
    brand: null,
    model: null,
    builtIn: true,
    params,
  };
}

function headphonePreset(
  id: string,
  brand: string,
  model: string,
  preampDb: number,
  filters: AutoEqFilter[],
): Preset {
  const params = defaultParams();
  params.limiter.enabled = true;
  params.autoEq = { enabled: true, preampDb, filters };
  return {
    id,
    name: `${brand} ${model}`,
    category: "headphone",
    description: `Curva de referência Audio Pulse para ${brand} ${model}.`,
    brand,
    model,
    builtIn: true,
    params,
  };
}

export const MOCK_PRESETS: Preset[] = [
  modePreset(
    "music",
    "Música",
    "music",
    "Curva equilibrada, graves sutis e compensação moderada.",
    (p) => {
      p.eqDb = [1, 0.8, 0.3, 0, -0.4, 0, 0.3, 0.6, 0.8, 0.5];
      p.bass = { enabled: true, intensity: 0.25, cutoffHz: 85, harmonics: 0.25 };
      p.loudness = { enabled: true, intensity: 0.5 };
    },
  ),
  modePreset(
    "cinema",
    "Cinema",
    "cinema",
    "Palco amplo, LFE reforçado e diálogos centralizados.",
    (p) => {
      p.eqDb = [2.5, 2, 1, 0, -0.5, 0, 1, 1.5, 1, 0];
      p.bass = { enabled: true, intensity: 0.5, cutoffHz: 75, harmonics: 0.4 };
      p.spatial = { mode: "surround", width: 1.1, room: 0.5 };
      p.compressor = {
        enabled: true,
        thresholdDb: -20,
        ratio: 2.2,
        attackMs: 15,
        releaseMs: 200,
        kneeDb: 6,
        makeupDb: 2,
      };
      p.loudness = { enabled: true, intensity: 0.4 };
    },
  ),
  modePreset(
    "gaming",
    "Jogos",
    "gaming",
    "Passos nítidos, palco amplo e dinâmica controlada.",
    (p) => {
      p.eqDb = [0.5, 0.5, 0, -1, 0, 1, 2, 1.5, 0.5, 0];
      p.bass = { enabled: true, intensity: 0.25, cutoffHz: 85, harmonics: 0.2 };
      p.spatial = { mode: "surround", width: 1.3, room: 0.25 };
      p.crossfeed = { enabled: true, amount: 0.15, delayMs: 0.3 };
      p.compressor = {
        enabled: true,
        thresholdDb: -22,
        ratio: 2,
        attackMs: 5,
        releaseMs: 100,
        kneeDb: 6,
        makeupDb: 2,
      };
      p.limiter.ceilingDb = -0.5;
    },
  ),
  modePreset(
    "podcast",
    "Podcast",
    "podcast",
    "Voz à frente, graves reduzidos e nível consistente.",
    (p) => {
      p.eqDb = [-4, -3, -2, -1, 0, 1, 2.5, 2, 1, 0];
      p.compressor = {
        enabled: true,
        thresholdDb: -24,
        ratio: 3,
        attackMs: 8,
        releaseMs: 120,
        kneeDb: 6,
        makeupDb: 4,
      };
      p.loudness = { enabled: true, intensity: 0.7 };
    },
  ),
  modePreset(
    "vocal",
    "Vocal",
    "vocal",
    "Presença e clareza para voz, mantendo os extremos suaves.",
    (p) => {
      p.eqDb = [-2, -1.5, -0.5, 0, 0.8, 1.5, 2.5, 2, 1, 0.5];
      p.compressor = {
        enabled: true,
        thresholdDb: -20,
        ratio: 2,
        attackMs: 12,
        releaseMs: 160,
        kneeDb: 3,
        makeupDb: 1.5,
      };
      p.loudness = { enabled: true, intensity: 0.5 };
    },
  ),
  modePreset("flat", "Flat", "flat", "Sem equalização ou realce.", () => {}),
  modePreset(
    "bass",
    "Bass",
    "bass",
    "Subwoofer virtual e graves amplos.",
    (p) => {
      p.eqDb = [4, 3.5, 2, 0, -1, 0, 0.5, 0.5, 0, 0];
      p.bass = { enabled: true, intensity: 0.85, cutoffHz: 70, harmonics: 0.35 };
      p.loudness = { enabled: true, intensity: 0.4 };
    },
  ),
  modePreset(
    "night",
    "Noturno",
    "night",
    "Faixa dinâmica comprimida para volume baixo.",
    (p) => {
      p.eqDb = [-1, -0.5, 0, 0.5, 0.5, 0.5, 0, 0, 0, 0];
      p.compressor = {
        enabled: true,
        thresholdDb: -30,
        ratio: 5,
        attackMs: 5,
        releaseMs: 250,
        kneeDb: 6,
        makeupDb: 6,
      };
      p.loudness = { enabled: true, intensity: 0.9 };
      p.limiter.ceilingDb = -1;
    },
  ),
  modePreset(
    "protection",
    "Proteção",
    "protection",
    "Graves contidos, dinâmica controlada e teto de pico baixo para sessões longas.",
    (p) => {
      p.preampDb = -2;
      p.compressor = {
        enabled: true,
        thresholdDb: -24,
        ratio: 3,
        attackMs: 5,
        releaseMs: 250,
        kneeDb: 6,
        makeupDb: 0,
      };
      p.limiter.ceilingDb = -3;
    },
  ),
  headphonePreset("hp-sony-wh1000xm4", "Sony", "WH-1000XM4", -6, [
    ls(105, -2.5, 0.7),
    pk(220, 1.5, 1),
    pk(1100, 0.8, 1.2),
    pk(2700, 4, 1.1),
    pk(4300, -2, 2),
    pk(6200, -3, 1.5),
    hs(9000, 2, 0.7),
  ]),
  headphonePreset("hp-sony-wh1000xm5", "Sony", "WH-1000XM5", -5, [
    ls(100, -3, 0.7),
    pk(250, 1, 1),
    pk(3000, 3.5, 1.1),
    pk(6000, -2.5, 1.8),
    hs(10000, 1.5, 0.7),
  ]),
  headphonePreset("hp-bose-qc45", "Bose", "QuietComfort 45", -4, [
    ls(120, -1.5, 0.7),
    pk(350, 1, 1),
    pk(2300, 3, 1.2),
    pk(5000, -1.5, 1.5),
    hs(9000, 1, 0.7),
  ]),
  headphonePreset("hp-bose-qc-ultra", "Bose", "QuietComfort Ultra", -5, [
    ls(90, -2, 0.7),
    pk(400, 1.5, 1),
    pk(2800, 2.5, 1.2),
    pk(5500, -2, 1.5),
    hs(10000, 1.5, 0.7),
  ]),
  headphonePreset("hp-sennheiser-hd600", "Sennheiser", "HD 600", -2, [
    ls(60, 2, 0.7),
    pk(3500, -1.5, 1.5),
    pk(5500, 1, 2),
    hs(10000, -1, 0.7),
  ]),
  headphonePreset("hp-sennheiser-momentum4", "Sennheiser", "Momentum 4", -3, [
    ls(110, 1.5, 0.7),
    pk(900, -1, 1.2),
    pk(2600, 2, 1.2),
    pk(6000, -1.5, 1.5),
    hs(9500, 1, 0.7),
  ]),
  headphonePreset("hp-apple-airpods-max", "Apple", "AirPods Max", -2.5, [
    ls(90, 1, 0.7),
    pk(400, -0.5, 1),
    pk(2500, 1.5, 1.2),
    pk(5500, -1, 1.5),
    hs(9000, 0.5, 0.7),
  ]),
  headphonePreset("hp-apple-airpods-pro2", "Apple", "AirPods Pro (2ª geração)", -3, [
    ls(100, -1, 0.7),
    pk(200, 1, 1),
    pk(3000, 2, 1.2),
    pk(6000, -1, 1.5),
    hs(9000, 0.5, 0.7),
  ]),
  headphonePreset("hp-jbl-tune770nc", "JBL", "Tune 770NC", -3, [
    ls(100, 2, 0.7),
    pk(500, -1, 1),
    pk(2500, 1.5, 1.2),
    hs(8000, 1.5, 0.7),
  ]),
  headphonePreset("hp-samsung-buds2-pro", "Samsung", "Galaxy Buds2 Pro", -4, [
    ls(90, -2.5, 0.7),
    pk(300, 1, 1),
    pk(2800, 2, 1.2),
    pk(5500, -1.5, 1.5),
    hs(10000, 1, 0.7),
  ]),
  headphonePreset("hp-ath-m50x", "Audio-Technica", "ATH-M50x", -3.5, [
    ls(100, -2, 0.7),
    pk(300, 2, 1),
    pk(4000, -3, 2),
    pk(9000, 2, 1.2),
  ]),
  headphonePreset("hp-beyerdynamic-dt770", "Beyerdynamic", "DT 770 Pro", -4, [
    ls(80, 2, 0.7),
    pk(250, -1, 1),
    pk(6000, -3, 2),
    pk(10000, -4, 1.2),
  ]),
];

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
  return JSON.stringify(left) === JSON.stringify(right);
}

function currentView(): AppView {
  return {
    params: clone(state.params),
    settings: clone(state.settings),
    presets: clone(state.presets),
    activePresetId: state.activePresetId,
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
    if (state.activePresetId) {
      const preset = state.presets.find((p) => p.id === state.activePresetId);
      if (!preset || !shapingEqual(preset.params, state.params)) {
        state.activePresetId = null;
      }
    }
    emitState();
    return { params: clone(state.params), activePresetId: state.activePresetId };
  },

  setVolume(volume: number): ParamsUpdate {
    state.params.volume = Math.min(1, Math.max(0, volume));
    emitState();
    return { params: clone(state.params), activePresetId: state.activePresetId };
  },

  toggleBypass(): ParamsUpdate {
    state.params.bypass = !state.params.bypass;
    emitState();
    return { params: clone(state.params), activePresetId: state.activePresetId };
  },

  setSpatialMode(mode: ProcessingParams["spatial"]["mode"]): ParamsUpdate {
    state.params.spatial.mode = mode;
    if (state.activePresetId) {
      const preset = state.presets.find((p) => p.id === state.activePresetId);
      if (!preset || !shapingEqual(preset.params, state.params)) {
        state.activePresetId = null;
      }
    }
    emitState();
    return { params: clone(state.params), activePresetId: state.activePresetId };
  },

  applyPreset(id: string): AppView {
    const preset = state.presets.find((p) => p.id === id);
    if (!preset) {
      throw new Error(`Preset não encontrado: ${id}`);
    }
    const params = clone(preset.params);
    params.volume = state.params.volume;
    params.bypass = state.params.bypass;
    params.earProfile = clone(state.params.earProfile);
    state.params = params;
    state.activePresetId = id;
    emitState();
    return currentView();
  },

  restoreSoundDefaults(): AppView {
    const preset = state.presets.find((p) => p.id === "music");
    const params = clone(preset ? preset.params : defaultParams());
    params.volume = state.params.volume;
    params.bypass = false;
    params.earProfile = { enabled: false, preampDb: 0, filters: [] };
    state.params = params;
    state.activePresetId = "music";
    emitState();
    return currentView();
  },

  savePreset(name: string): AppView {
    const params = clone(state.params);
    params.earProfile = { enabled: false, preampDb: 0, filters: [] };
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