import { isTauri } from "../lib/api";
import { presetTitle, translate, type MessageKey } from "../lib/i18n";
import { useAppStore, useLanguage } from "../store/appStore";
import { Icon } from "./Icons";

function BatteryGlyph({ percent }: { percent: number }) {
  const clamped = Math.max(0, Math.min(100, percent));
  const fill = Math.max(1.5, (clamped / 100) * 13);
  return (
    <svg width="20" height="12" viewBox="0 0 20 12" aria-hidden="true" focusable="false">
      <rect
        x="0.75"
        y="0.75"
        width="16"
        height="10.5"
        rx="2.5"
        fill="none"
        stroke="currentColor"
        strokeOpacity="0.45"
        strokeWidth="1.5"
      />
      <rect x="17" y="4" width="2" height="4" rx="1" fill="currentColor" fillOpacity="0.45" />
      <rect x="2.5" y="2.5" width={fill} height="7" rx="1.25" fill="currentColor" />
    </svg>
  );
}

export function TopBar() {
  const page = useAppStore((state) => state.page);
  const view = useAppStore((state) => state.view);
  const engineBusy = useAppStore((state) => state.engineBusy);
  const toggleBypass = useAppStore((state) => state.toggleBypass);
  const startEngine = useAppStore((state) => state.startEngine);
  const stopEngine = useAppStore((state) => state.stopEngine);
  const patchSettings = useAppStore((state) => state.patchSettings);
  const setPage = useAppStore((state) => state.setPage);
  const language = useLanguage();
  const t = (key: MessageKey) => translate(language, key);

  const running = view?.engine.running ?? false;
  const bypass = view?.params.bypass ?? false;
  const theme = view?.settings.theme ?? "dark";
  const battery = view?.battery ?? null;
  const active = view?.presets.find((preset) => preset.id === view.activePresetId);
  const headphone = view?.presets.find(
    (preset) => preset.id === view.activeHeadphoneId,
  );
  const title = t(`nav.${page}` as MessageKey);
  const batteryTitle = battery
    ? translate(language, "topbar.battery", {
        percent: battery.percent,
        device: battery.device,
      })
    : "";

  return (
    <header className="topbar">
      <div className="topbar-title">
        <h1>{title}</h1>
        {!isTauri && <span className="demo-chip">{t("common.demo")}</span>}
      </div>

      <div className="topbar-actions">
        <button
          type="button"
          className="preset-chip"
          onClick={() => setPage("presets")}
          title={t("presets.activePreset")}
        >
          <Icon name="presets" size={15} />
          <span>{active ? presetTitle(language, active) : t("presets.noneActive")}</span>
        </button>

        {headphone && (
          <button
            type="button"
            className="preset-chip"
            onClick={() => setPage("presets")}
            title={t("presets.activeHeadphone")}
          >
            <Icon name="headphone" size={15} />
            <span>{headphone.model}</span>
          </button>
        )}

        <button
          type="button"
          className={`chip-button ${bypass ? "is-active" : ""}`}
          onClick={() => toggleBypass()}
          aria-pressed={bypass}
          title={t("topbar.bypass")}
        >
          <Icon name="power" size={16} />
          <span className="hide-narrow">{t("topbar.bypass")}</span>
        </button>

        {battery && (
          <span
            className={`battery-chip ${battery.percent <= 20 ? "is-low" : ""}`}
            title={batteryTitle}
            aria-label={batteryTitle}
          >
            <BatteryGlyph percent={battery.percent} />
            <span>{battery.percent}%</span>
          </span>
        )}

        <button
          type="button"
          className="icon-btn"
          onClick={() => patchSettings({ theme: theme === "dark" ? "light" : "dark" })}
          title={t("settings.theme")}
          aria-label={t("settings.theme")}
        >
          <Icon name={theme === "dark" ? "sun" : "moon"} size={18} />
        </button>

        <button
          type="button"
          className={`engine-pill ${running ? "is-running" : ""} ${engineBusy ? "is-busy" : ""}`}
          disabled={engineBusy || !view}
          onClick={() => (running ? stopEngine() : startEngine("route"))}
        >
          <Icon name={running ? "stop" : "play"} size={15} />
          <span>
            {engineBusy
              ? t("engine.starting")
              : running
                ? t("engine.stop")
                : t("engine.startRoute")}
          </span>
        </button>
      </div>
    </header>
  );
}