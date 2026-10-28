import { create } from "zustand";

import {
  api,
  choosePresetExportPath,
  isTauri,
  pickAutoEqFile,
  pickPresetFile,
} from "../lib/api";
import { detectedLanguage, translate } from "../lib/i18n";
import {
  EMPTY_TELEMETRY,
  type AppView,
  type AutoEqParams,
  type EarAnalysis,
  type EarPhotoSelection,
  type EngineMode,
  type Language,
  type ParamsUpdate,
  type ProcessingParams,
  type Settings,
  type SpatialMode,
  type Telemetry,
  type Toast,
} from "../lib/types";

export type Page =
  | "dashboard"
  | "equalizer"
  | "spatial"
  | "dynamics"
  | "presets"
  | "ear"
  | "settings";

type Tone = Toast["tone"];

interface AppStore {
  view: AppView | null;
  telemetry: Telemetry;
  page: Page;
  toasts: Toast[];
  ready: boolean;
  engineBusy: boolean;
  cableBusy: boolean;
  earAnalysis: EarAnalysis | null;
  earBackup: AutoEqParams | null;
  earPhoto: EarPhotoSelection | null;
  bootstrap: () => Promise<void>;
  setPage: (page: Page) => void;
  applyView: (view: AppView) => void;
  setEarAnalysis: (analysis: EarAnalysis | null) => void;
  setEarBackup: (backup: AutoEqParams | null) => void;
  setEarPhoto: (photo: EarPhotoSelection | null) => void;
  patchParams: (mutate: (params: ProcessingParams) => void) => void;
  setVolume: (volume: number) => void;
  toggleBypass: () => Promise<void>;
  setSpatialMode: (mode: SpatialMode) => Promise<void>;
  applyPreset: (id: string) => Promise<void>;
  savePreset: (name: string) => Promise<boolean>;
  deletePreset: (id: string) => Promise<void>;
  exportPreset: (id: string) => Promise<void>;
  importPreset: () => Promise<void>;
  importAutoEq: () => Promise<void>;
  startEngine: (mode: EngineMode, wavPath?: string | null) => Promise<void>;
  stopEngine: () => Promise<void>;
  patchSettings: (patch: Partial<Settings>) => void;
  resetShortcuts: () => void;
  refreshDevices: () => Promise<void>;
  installVirtualCable: () => Promise<void>;
  setDeviceProfile: (device: string, presetId: string | null) => Promise<void>;
  toast: (message: string, tone?: Tone) => void;
  dismissToast: (id: number) => void;
  language: () => Language;
}

const EAR_STORAGE_KEY = "audiopulse.ear-profile";
const LEGACY_EAR_STORAGE_KEY = "sonicenhance.ear-profile";

interface StoredEarProfile {
  analysis: EarAnalysis | null;
  backup: AutoEqParams | null;
}

function readStoredEarProfile(): StoredEarProfile {
  try {
    const raw =
      window.localStorage.getItem(EAR_STORAGE_KEY) ??
      window.localStorage.getItem(LEGACY_EAR_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as Partial<StoredEarProfile>;
      window.localStorage.setItem(EAR_STORAGE_KEY, raw);
      return {
        analysis:
          parsed.analysis && Array.isArray(parsed.analysis.filters)
            ? parsed.analysis
            : null,
        backup:
          parsed.backup && Array.isArray(parsed.backup.filters)
            ? parsed.backup
            : null,
      };
    }
  } catch {
    // A corrupt or unavailable store must not block the app.
  }
  return { analysis: null, backup: null };
}

function persistEarProfile(
  analysis: EarAnalysis | null,
  backup: AutoEqParams | null,
) {
  try {
    if (!analysis && !backup) {
      window.localStorage.removeItem(EAR_STORAGE_KEY);
    } else {
      window.localStorage.setItem(
        EAR_STORAGE_KEY,
        JSON.stringify({ analysis, backup }),
      );
    }
  } catch {
    // Quota or private mode: the in-memory profile still works.
  }
}

const storedEar = readStoredEarProfile();

let bootstrapPromise: Promise<void> | null = null;
let paramsTimer: number | null = null;
let paramsSeq = 0;
let settingsTimer: number | null = null;
let settingsSeq = 0;
let toastSeq = 0;

function cancelParamsTimer() {
  if (paramsTimer !== null) {
    window.clearTimeout(paramsTimer);
    paramsTimer = null;
  }
  paramsSeq += 1;
}

export const useAppStore = create<AppStore>((set, get) => {
  const lang = () => {
    const view = get().view;
    return detectedLanguage(view ? view.settings.language : "");
  };

  const fail = (error: unknown) => {
    const message = error instanceof Error ? error.message : String(error);
    get().toast(translate(lang(), "toast.error", { message }), "error");
  };

  const applyUpdate = (update: ParamsUpdate) => {
    const view = get().view;
    if (!view) return;
    set({
      view: {
        ...view,
        params: update.params,
        activePresetId: update.activePresetId,
      },
    });
  };

  const flushSettings = async () => {
    if (settingsTimer !== null) {
      window.clearTimeout(settingsTimer);
      settingsTimer = null;
    }
    const view = get().view;
    if (!view) return;
    const seq = settingsSeq;
    try {
      const next = await api.updateSettings(view.settings);
      if (seq === settingsSeq) set({ view: next });
    } catch (error) {
      fail(error);
    }
  };

  return {
    view: null,
    telemetry: EMPTY_TELEMETRY,
    page: "dashboard",
    toasts: [],
    ready: false,
    engineBusy: false,
    cableBusy: false,
    earAnalysis: storedEar.analysis,
    earBackup: storedEar.backup,
    earPhoto: null,

    language: lang,

    bootstrap: () => {
      if (bootstrapPromise) return bootstrapPromise;
      bootstrapPromise = (async () => {
        api.onStateChanged((view) => get().applyView(view));
        api.onDevicesChanged((devices) => {
          const view = get().view;
          if (view) set({ view: { ...view, devices } });
        });
        api.onTelemetry((telemetry) => set({ telemetry }));
        const view = await api.getState();
        set({ view, ready: true });
      })().catch((error) => {
        bootstrapPromise = null;
        const message = error instanceof Error ? error.message : String(error);
        get().toast(message, "error");
      });
      return bootstrapPromise;
    },

    setPage: (page) => set({ page }),

    setEarAnalysis: (analysis) => {
      set({ earAnalysis: analysis });
      persistEarProfile(analysis, get().earBackup);
    },

    setEarBackup: (backup) => {
      set({ earBackup: backup });
      persistEarProfile(get().earAnalysis, backup);
    },

    setEarPhoto: (photo) => set({ earPhoto: photo }),

    applyView: (view) => {
      const current = get().view;
      if (current && paramsTimer !== null) {
        set({ view: { ...view, params: current.params } });
      } else {
        set({ view });
      }
    },

    patchParams: (mutate) => {
      const view = get().view;
      if (!view) return;
      const params = structuredClone(view.params);
      mutate(params);
      if (paramsTimer !== null) window.clearTimeout(paramsTimer);
      paramsSeq += 1;
      const seq = paramsSeq;
      set({ view: { ...view, params } });
      paramsTimer = window.setTimeout(async () => {
        paramsTimer = null;
        try {
          const update = await api.setParams(get().view?.params ?? params);
          if (seq === paramsSeq) applyUpdate(update);
        } catch (error) {
          fail(error);
        }
      }, 50);
    },

    setVolume: (volume) => {
      get().patchParams((params) => {
        params.volume = Math.min(1, Math.max(0, volume));
      });
    },

    toggleBypass: async () => {
      const view = get().view;
      if (view) {
        set({
          view: {
            ...view,
            params: { ...view.params, bypass: !view.params.bypass },
          },
        });
      }
      try {
        applyUpdate(await api.toggleBypass());
      } catch (error) {
        fail(error);
      }
    },

    setSpatialMode: async (mode) => {
      if (get().view?.params.spatial.mode === mode) return;
      try {
        applyUpdate(await api.setSpatialMode(mode));
      } catch (error) {
        fail(error);
      }
    },

    applyPreset: async (id) => {
      cancelParamsTimer();
      try {
        const view = await api.applyPreset(id);
        set({ view });
        get().toast(translate(lang(), "toast.presetApplied"), "success");
      } catch (error) {
        fail(error);
      }
    },

    savePreset: async (name) => {
      try {
        const view = await api.savePreset(name);
        set({ view });
        get().toast(translate(lang(), "toast.saved"), "success");
        return true;
      } catch (error) {
        fail(error);
        return false;
      }
    },

    deletePreset: async (id) => {
      try {
        const view = await api.deletePreset(id);
        set({ view });
        get().toast(translate(lang(), "toast.deleted"), "success");
      } catch (error) {
        fail(error);
      }
    },

    exportPreset: async (id) => {
      if (!isTauri) {
        get().toast(translate(lang(), "toast.unavailableInBrowser"), "info");
        return;
      }
      const view = get().view;
      const preset = view?.presets.find((item) => item.id === id);
      if (!preset) {
        get().toast(translate(lang(), "toast.exportEmpty"), "error");
        return;
      }
      try {
        const path = await choosePresetExportPath(preset.name);
        if (!path) return;
        await api.exportPreset(id, path);
        get().toast(translate(lang(), "toast.exported"), "success");
      } catch (error) {
        fail(error);
      }
    },

    importPreset: async () => {
      if (!isTauri) {
        get().toast(translate(lang(), "toast.unavailableInBrowser"), "info");
        return;
      }
      try {
        const path = await pickPresetFile();
        if (!path) return;
        const view = await api.importPreset(path);
        set({ view });
        get().toast(translate(lang(), "toast.imported"), "success");
      } catch (error) {
        fail(error);
      }
    },

    importAutoEq: async () => {
      if (!isTauri) {
        get().toast(translate(lang(), "toast.unavailableInBrowser"), "info");
        return;
      }
      try {
        const path = await pickAutoEqFile();
        if (!path) return;
        cancelParamsTimer();
        const view = await api.importAutoEq(path);
        set({ view });
        get().toast(translate(lang(), "toast.autoEqImported"), "success");
      } catch (error) {
        fail(error);
      }
    },

    startEngine: async (mode, wavPath) => {
      if (get().engineBusy) return;
      set({ engineBusy: true });
      try {
        await flushSettings();
        const info = await api.startEngine(mode, wavPath);
        const view = get().view;
        if (view) set({ view: { ...view, engine: info } });
        get().toast(translate(lang(), "toast.engineStarted"), "success");
      } catch (error) {
        fail(error);
        try {
          const view = await api.getState();
          set({ view });
        } catch {
          // The toast already carries the failure.
        }
      } finally {
        set({ engineBusy: false });
      }
    },

    stopEngine: async () => {
      if (get().engineBusy) return;
      set({ engineBusy: true });
      try {
        const info = await api.stopEngine();
        const view = get().view;
        if (view) set({ view: { ...view, engine: info } });
        get().toast(translate(lang(), "toast.engineStopped"), "info");
      } catch (error) {
        fail(error);
      } finally {
        set({ engineBusy: false });
      }
    },

    patchSettings: (patch) => {
      const view = get().view;
      if (!view) return;
      const settings = { ...view.settings, ...patch };
      set({ view: { ...view, settings } });
      if (settingsTimer !== null) window.clearTimeout(settingsTimer);
      settingsSeq += 1;
      const seq = settingsSeq;
      settingsTimer = window.setTimeout(async () => {
        settingsTimer = null;
        const current = get().view;
        if (!current) return;
        try {
          const next = await api.updateSettings(current.settings);
          if (seq === settingsSeq) set({ view: next });
        } catch (error) {
          fail(error);
        }
      }, 240);
    },

    resetShortcuts: () => {
      const view = get().view;
      if (!view) return;
      get().patchSettings({
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
      });
    },

    refreshDevices: async () => {
      try {
        const devices = await api.refreshDevices();
        const view = get().view;
        if (view) set({ view: { ...view, devices } });
      } catch (error) {
        fail(error);
      }
    },

    installVirtualCable: async () => {
      if (!isTauri) {
        get().toast(translate(lang(), "toast.unavailableInBrowser"), "info");
        return;
      }
      if (get().cableBusy) return;
      set({ cableBusy: true });
      try {
        const view = await api.installVirtualCable();
        set({ view });
        get().toast(translate(lang(), "toast.cableInstalled"), "success");
      } catch (error) {
        fail(error);
        try {
          const view = await api.getState();
          set({ view });
        } catch {
          // The toast already carries the failure.
        }
      } finally {
        set({ cableBusy: false });
      }
    },

    setDeviceProfile: async (device, presetId) => {
      try {
        const view = await api.setDeviceProfile(device, presetId);
        set({ view });
      } catch (error) {
        fail(error);
      }
    },

    toast: (message, tone = "info") => {
      const id = ++toastSeq;
      set((state) => ({ toasts: [...state.toasts, { id, message, tone }] }));
      window.setTimeout(() => {
        set((state) => ({ toasts: state.toasts.filter((item) => item.id !== id) }));
      }, 4200);
    },

    dismissToast: (id) => {
      set((state) => ({ toasts: state.toasts.filter((item) => item.id !== id) }));
    },
  };
});

export function useLanguage(): Language {
  return useAppStore((state) =>
    detectedLanguage(state.view ? state.view.settings.language : ""),
  );
}