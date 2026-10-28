import { Segmented, Slider, Toggle } from "../components/Controls";
import { Icon } from "../components/Icons";
import { Panel } from "../components/Panel";
import { ShortcutInput } from "../components/ShortcutInput";
import { isTauri, pickWavFile } from "../lib/api";
import { formatHz, formatMs } from "../lib/format";
import { presetTitle, translate, type MessageKey } from "../lib/i18n";
import type { Theme } from "../lib/types";
import { useAppStore, useLanguage } from "../store/appStore";

const SHORTCUT_ACTIONS: { id: string; label: MessageKey }[] = [
  { id: "toggle_bypass", label: "shortcut.toggle_bypass" },
  { id: "preset_music", label: "shortcut.preset_music" },
  { id: "preset_cinema", label: "shortcut.preset_cinema" },
  { id: "preset_gaming", label: "shortcut.preset_gaming" },
  { id: "preset_podcast", label: "shortcut.preset_podcast" },
  { id: "toggle_spatial", label: "shortcut.toggle_spatial" },
  { id: "volume_up", label: "shortcut.volume_up" },
  { id: "volume_down", label: "shortcut.volume_down" },
  { id: "open_window", label: "shortcut.open_window" },
];

export function Settings() {
  const view = useAppStore((state) => state.view);
  const patchSettings = useAppStore((state) => state.patchSettings);
  const resetShortcuts = useAppStore((state) => state.resetShortcuts);
  const refreshDevices = useAppStore((state) => state.refreshDevices);
  const installVirtualCable = useAppStore((state) => state.installVirtualCable);
  const setDeviceProfile = useAppStore((state) => state.setDeviceProfile);
  const startEngine = useAppStore((state) => state.startEngine);
  const engineBusy = useAppStore((state) => state.engineBusy);
  const cableBusy = useAppStore((state) => state.cableBusy);
  const language = useLanguage();
  const t = (key: MessageKey, vars?: Record<string, string | number>) =>
    translate(language, key, vars);

  if (!view) return <div className="page-loading">{t("common.loading")}</div>;
  const settings = view.settings;
  const devices = view.devices;
  const engine = view.engine;
  const physicalOutputs = devices.outputs.filter((device) => !device.isVirtual);

  const chooseWav = async () => {
    const path = await pickWavFile();
    if (path) await startEngine("wav", path);
  };

  return (
    <div className="page settings">
      <div className="settings-grid">
        <Panel title={t("settings.appearance")}>
          <div className="field">
            <span>{t("settings.theme")}</span>
            <Segmented<Theme>
              value={settings.theme}
              onChange={(theme) => patchSettings({ theme })}
              ariaLabel={t("settings.theme")}
              options={[
                {
                  value: "dark",
                  label: t("settings.dark"),
                  icon: <Icon name="moon" size={14} />,
                },
                {
                  value: "light",
                  label: t("settings.light"),
                  icon: <Icon name="sun" size={14} />,
                },
              ]}
            />
          </div>
          <div className="field">
            <span>{t("settings.language")}</span>
            <Segmented
              value={settings.language}
              onChange={(value) => patchSettings({ language: value })}
              ariaLabel={t("settings.language")}
              options={[
                { value: "", label: t("common.auto") },
                { value: "pt-BR", label: "Português" },
                { value: "en", label: "English" },
              ]}
            />
          </div>
        </Panel>

        <Panel title={t("settings.system")}>
          <Toggle
            checked={settings.autostart}
            onChange={(autostart) => patchSettings({ autostart })}
            label={t("settings.autostart")}
          />
          <Toggle
            checked={settings.launchMinimized}
            onChange={(launchMinimized) => patchSettings({ launchMinimized })}
            label={t("settings.launchMinimized")}
          />
          <Toggle
            checked={settings.closeToTray}
            onChange={(closeToTray) => patchSettings({ closeToTray })}
            label={t("settings.closeToTray")}
          />
        </Panel>

        <Panel className="span-2" title={t("settings.audio")}>
          <div className="audio-grid">
            <div className="audio-col">
              <Slider
                label={t("settings.buffer")}
                value={settings.bufferMs}
                min={10}
                max={200}
                step={1}
                onChange={(bufferMs) => patchSettings({ bufferMs })}
                format={(value) => formatMs(value)}
              />
              <div className="field column">
                <span>{t("settings.input")}</span>
                <select
                  className="select"
                  value={settings.inputDevice ?? ""}
                  onChange={(event) =>
                    patchSettings({ inputDevice: event.target.value || null })
                  }
                >
                  <option value="">{t("settings.systemDefault")}</option>
                  {devices.inputs.map((device) => (
                    <option key={device.name} value={device.name}>
                      {device.name}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field column">
                <span>{t("settings.output")}</span>
                <select
                  className="select"
                  value={settings.outputDevice ?? ""}
                  onChange={(event) =>
                    patchSettings({ outputDevice: event.target.value || null })
                  }
                >
                  <option value="">{t("settings.systemDefault")}</option>
                  {devices.outputs.map((device) => (
                    <option key={device.name} value={device.name}>
                      {device.name}
                    </option>
                  ))}
                </select>
              </div>
              <div className="device-row">
                <span
                  className={`status-chip ${devices.virtualCableDetected ? "is-good" : "is-warn"}`}
                >
                  <Icon name="cable" size={14} />
                  {t("settings.virtualCable")}:{" "}
                  {devices.virtualCableDetected
                    ? t("settings.detected")
                    : t("settings.missing")}
                </span>
                {!devices.virtualCableDetected && devices.platform !== "linux" && (
                  <button
                    type="button"
                    className="btn primary small"
                    disabled={cableBusy}
                    onClick={() => installVirtualCable()}
                  >
                    <Icon name={cableBusy ? "refresh" : "download"} size={14} />
                    {cableBusy
                      ? t("settings.installingCable")
                      : t("settings.installCable")}
                  </button>
                )}
                <button type="button" className="btn ghost small" onClick={() => refreshDevices()}>
                  <Icon name="refresh" size={14} />
                  {t("common.refresh")}
                </button>
              </div>
            </div>

            <div className="audio-col">
              <div className="readout-grid">
                <div className="stat-row">
                  <span>{t("metrics.sampleRate")}</span>
                  <strong>{engine.sampleRate ? formatHz(engine.sampleRate) : "—"}</strong>
                </div>
                <div className="stat-row">
                  <span>{t("metrics.latency")}</span>
                  <strong>{engine.latencyMs ? formatMs(engine.latencyMs) : "—"}</strong>
                </div>
                <div className="stat-row">
                  <span>{t("metrics.xruns")}</span>
                  <strong>{engine.xruns}</strong>
                </div>
                <div className="stat-row">
                  <span>{t("metrics.cpu")}</span>
                  <strong>{engine.cpuPercent.toFixed(1)}%</strong>
                </div>
              </div>

              <div className="preview-block">
                <span className="field-label">{t("settings.preview")}</span>
                <div className="preview-grid">
                  <button
                    type="button"
                    className={`btn ghost ${engine.mode === "preview-noise" ? "is-active" : ""}`}
                    disabled={engineBusy}
                    onClick={() => startEngine("noise")}
                  >
                    <Icon name="wave" size={15} />
                    {t("engine.modeNoise")}
                  </button>
                  <button
                    type="button"
                    className={`btn ghost ${engine.mode === "preview-sweep" ? "is-active" : ""}`}
                    disabled={engineBusy}
                    onClick={() => startEngine("sweep")}
                  >
                    <Icon name="sparkle" size={15} />
                    {t("engine.modeSweep")}
                  </button>
                  <button
                    type="button"
                    className={`btn ghost ${engine.mode === "preview-wav" ? "is-active" : ""}`}
                    disabled={engineBusy || !isTauri}
                    onClick={chooseWav}
                  >
                    <Icon name="upload" size={15} />
                    {t("engine.modeWav")}
                  </button>
                </div>
              </div>
            </div>
          </div>
        </Panel>

        <Panel
          className="span-2"
          title={t("settings.profiles")}
          actions={
            <Toggle
              checked={settings.deviceProfilesEnabled}
              onChange={(enabled) => patchSettings({ deviceProfilesEnabled: enabled })}
            />
          }
        >
          {physicalOutputs.length === 0 ? (
            <div className="empty-state">{t("common.none")}</div>
          ) : (
            <div className="profile-list">
              {physicalOutputs.map((device) => (
                <div key={device.name} className="profile-row">
                  <span className="profile-name" title={device.name}>
                    <Icon name="headphone" size={15} />
                    {device.name}
                    {device.isDefault && (
                      <span className="tag">{t("common.default")}</span>
                    )}
                  </span>
                  <select
                    className="select compact"
                    disabled={!settings.deviceProfilesEnabled}
                    value={settings.deviceProfiles[device.name] ?? ""}
                    onChange={(event) =>
                      setDeviceProfile(device.name, event.target.value || null)
                    }
                  >
                    <option value="">{t("common.none")}</option>
                    {view.presets.map((preset) => (
                      <option key={preset.id} value={preset.id}>
                        {presetTitle(language, preset)}
                      </option>
                    ))}
                  </select>
                </div>
              ))}
            </div>
          )}
        </Panel>

        <Panel
          className="span-2"
          title={t("settings.shortcuts")}
          actions={
            <button type="button" className="btn ghost small" onClick={resetShortcuts}>
              <Icon name="refresh" size={14} />
              {t("settings.resetShortcuts")}
            </button>
          }
        >
          <div className="shortcut-grid">
            {SHORTCUT_ACTIONS.map((action) => (
              <div key={action.id} className="shortcut-row">
                <span>{t(action.label)}</span>
                <ShortcutInput
                  value={settings.shortcuts[action.id] ?? ""}
                  onChange={(accelerator) =>
                    patchSettings({
                      shortcuts: { ...settings.shortcuts, [action.id]: accelerator },
                    })
                  }
                  hint={t("settings.shortcutPress")}
                  invalidHint={t("toast.shortcutInvalid")}
                />
              </div>
            ))}
          </div>
        </Panel>

        <Panel title={t("settings.about")}>
          <div className="about">
            <span className="brand-mark">
              <Icon name="logo" size={30} />
            </span>
            <div className="about-text">
              <strong>Audio Pulse</strong>
              <span>
                {t("common.version")} {view.version} · {platformLabel(devices.platform)}
              </span>
            </div>
          </div>
          <div className="stat-row">
            <span>{t("common.platform")}</span>
            <strong>{platformLabel(devices.platform)}</strong>
          </div>
        </Panel>
      </div>
    </div>
  );
}

function platformLabel(platform: string): string {
  switch (platform) {
    case "windows":
      return "Windows";
    case "macos":
      return "macOS";
    case "linux":
      return "Linux";
    case "web":
      return "Web";
    default:
      return platform || "—";
  }
}