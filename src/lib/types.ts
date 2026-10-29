export type SpatialMode = "off" | "binaural" | "hrtf" | "surround";
export type FilterKind = "peaking" | "lowshelf" | "highshelf";
export type Theme = "dark" | "light";
export type Language = "pt-BR" | "en";

export interface AutoEqFilter {
  kind: FilterKind;
  freq: number;
  gainDb: number;
  q: number;
}

export interface BassParams {
  enabled: boolean;
  intensity: number;
  cutoffHz: number;
  harmonics: number;
}

export interface ToneParams {
  midDb: number;
  trebleDb: number;
}

export interface CompressorParams {
  enabled: boolean;
  thresholdDb: number;
  ratio: number;
  attackMs: number;
  releaseMs: number;
  kneeDb: number;
  makeupDb: number;
}

export interface LimiterParams {
  enabled: boolean;
  ceilingDb: number;
  releaseMs: number;
}

export interface LoudnessParams {
  enabled: boolean;
  intensity: number;
}

export interface LoudSoundReductionParams {
  enabled: boolean;
  intensity: number;
}

export interface SpatialParams {
  mode: SpatialMode;
  width: number;
  room: number;
}

export interface CrossfeedParams {
  enabled: boolean;
  amount: number;
  delayMs: number;
}

export interface AutoEqParams {
  enabled: boolean;
  preampDb: number;
  filters: AutoEqFilter[];
}

export interface AdaptiveEqParams {
  enabled: boolean;
  intensity: number;
}

export interface EarProfileParams {
  enabled: boolean;
  preampDb: number;
  filters: AutoEqFilter[];
}

export interface ProcessingParams {
  preampDb: number;
  eqDb: number[];
  bass: BassParams;
  tone: ToneParams;
  compressor: CompressorParams;
  limiter: LimiterParams;
  loudness: LoudnessParams;
  loudSoundReduction: LoudSoundReductionParams;
  spatial: SpatialParams;
  crossfeed: CrossfeedParams;
  autoEq: AutoEqParams;
  adaptiveEq: AdaptiveEqParams;
  earProfile: EarProfileParams;
  volume: number;
  bypass: boolean;
}

export interface ParamsUpdate {
  params: ProcessingParams;
  activePresetId: string | null;
}

export type PresetCategory =
  | "music"
  | "cinema"
  | "gaming"
  | "podcast"
  | "vocal"
  | "flat"
  | "bass"
  | "night"
  | "protection"
  | "headphone"
  | "custom";

export interface Preset {
  id: string;
  name: string;
  category: PresetCategory;
  description: string;
  brand: string | null;
  model: string | null;
  builtIn: boolean;
  params: ProcessingParams;
}

export interface Settings {
  language: string;
  theme: Theme;
  autostart: boolean;
  closeToTray: boolean;
  launchMinimized: boolean;
  bufferMs: number;
  deviceProfilesEnabled: boolean;
  inputDevice: string | null;
  outputDevice: string | null;
  shortcuts: Record<string, string>;
  deviceProfiles: Record<string, string>;
}

export interface DeviceInfo {
  name: string;
  isDefault: boolean;
  isVirtual: boolean;
}

export interface DeviceList {
  inputs: DeviceInfo[];
  outputs: DeviceInfo[];
  defaultInput: string | null;
  defaultOutput: string | null;
  virtualCableDetected: boolean;
  platform: string;
}

export interface EngineInfo {
  running: boolean;
  mode: string;
  sampleRate: number;
  inputDevice: string | null;
  outputDevice: string | null;
  bufferMs: number;
  latencyMs: number;
  cpuPercent: number;
  xruns: number;
  error: string | null;
}

export interface BatteryInfo {
  percent: number;
  device: string;
}

export interface AppView {
  params: ProcessingParams;
  settings: Settings;
  presets: Preset[];
  activePresetId: string | null;
  devices: DeviceList;
  engine: EngineInfo;
  battery: BatteryInfo | null;
  warnings: string[];
  version: string;
}

export interface Telemetry {
  bands: number[];
  rmsL: number;
  rmsR: number;
  peakL: number;
  peakR: number;
  compressorGrDb: number;
  limiterGrDb: number;
  protectionGrDb: number;
  adaptiveGainsDb: number[];
  cpuPercent: number;
  latencyMs: number;
  xruns: number;
  running: boolean;
  sampleRate: number;
}

export interface EarPhoto {
  dataUrl: string;
  width: number;
  height: number;
}

export interface EarPhotoSelection {
  dataUrl: string;
  name: string | null;
}

export interface EarModelStatus {
  installed: boolean;
  bytes: number;
  expectedBytes: number;
  name: string;
  license: string;
}

export interface EarModelProgress {
  received: number;
  total: number;
}

export interface EarMetrics {
  conchaDepth: number;
  conchaSize: number;
  helixProminence: number;
  depthRange: number;
  brightness: number;
}

export interface EarAnalysis {
  filters: AutoEqFilter[];
  preampDb: number;
  resonanceHz: number;
  confidence: number;
  metrics: EarMetrics;
  warnings: string[];
  overlayPng: string;
  model: string;
  elapsedMs: number;
}

export type EngineMode = "route" | "noise" | "sweep" | "wav";

export interface Toast {
  id: number;
  message: string;
  tone: "info" | "success" | "error";
}

export const EQ_FREQUENCIES = [
  31, 62, 125, 250, 500, 1000, 2000, 4000, 8000, 16000,
] as const;

export const ADAPTIVE_EQ_FREQUENCIES = [80, 300, 1000, 3500, 11000] as const;
export const ADAPTIVE_EQ_MAX_DB = 3;

export const EMPTY_TELEMETRY: Telemetry = {
  bands: Array.from({ length: 56 }, () => 0),
  rmsL: 0,
  rmsR: 0,
  peakL: 0,
  peakR: 0,
  compressorGrDb: 0,
  protectionGrDb: 0,
  limiterGrDb: 0,
  adaptiveGainsDb: Array.from({ length: 5 }, () => 0),
  cpuPercent: 0,
  latencyMs: 0,
  xruns: 0,
  running: false,
  sampleRate: 0,
};