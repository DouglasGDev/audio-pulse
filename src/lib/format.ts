import type { MessageKey } from "./i18n";

export function formatDb(value: number, digits = 1): string {
  const rounded = Math.abs(value) < 0.05 ? 0 : value;
  return `${rounded > 0 ? "+" : ""}${rounded.toFixed(digits)} dB`;
}

export function formatHz(value: number): string {
  if (value >= 1000) {
    const khz = value / 1000;
    return `${Number.isInteger(khz) ? khz : khz.toFixed(1)} kHz`;
  }
  return `${Math.round(value)} Hz`;
}

export function formatMs(value: number): string {
  return `${value < 10 ? value.toFixed(2) : value.toFixed(1)} ms`;
}

export function formatPercent(value: number, digits = 0): string {
  return `${value.toFixed(digits)}%`;
}

export function engineModeKey(mode: string): MessageKey {
  switch (mode) {
    case "route":
      return "engine.modeRoute";
    case "preview-noise":
      return "engine.modeNoise";
    case "preview-sweep":
      return "engine.modeSweep";
    case "preview-wav":
      return "engine.modeWav";
    default:
      return "engine.modeStopped";
  }
}

/** Tauri accelerators use `CmdOrCtrl`; show the platform-appropriate label. */
export function formatAccelerator(accelerator: string): string {
  const mac =
    typeof navigator !== "undefined" && /mac/i.test(navigator.platform);
  return accelerator
    .replace(/CmdOrCtrl|CommandOrControl/g, mac ? "⌘" : "Ctrl")
    .replace(/Control/g, "Ctrl")
    .replace(/Command/g, mac ? "⌘" : "Win")
    .replace(/Super/g, mac ? "⌘" : "Win")
    .replace(/\+/g, " ");
}