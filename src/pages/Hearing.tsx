import { GainReduction } from "../components/GainReduction";
import { Icon } from "../components/Icons";
import { LevelMeter } from "../components/LevelMeter";
import { Panel } from "../components/Panel";
import { Slider, Toggle } from "../components/Controls";
import { formatPercent } from "../lib/format";
import { translate, type MessageKey } from "../lib/i18n";
import { useAppStore, useLanguage } from "../store/appStore";

function dbfs(value: number): string {
  if (!Number.isFinite(value) || value <= 1e-5) return "−∞ dB";
  return `${(20 * Math.log10(value)).toFixed(1)} dB`;
}

export function Hearing() {
  const view = useAppStore((state) => state.view);
  const telemetry = useAppStore((state) => state.telemetry);
  const patchParams = useAppStore((state) => state.patchParams);
  const applyPreset = useAppStore((state) => state.applyPreset);
  const language = useLanguage();
  const t = (key: MessageKey) => translate(language, key);

  if (!view) return <div className="page-loading">{t("common.loading")}</div>;
  const protection = view.params.loudSoundReduction;
  const protectionPreset = view.presets.find((preset) => preset.id === "protection");
  const protectionActive = view.activePresetId === "protection";

  return (
    <div className="page hearing">
      <Panel
        className="span-2"
        title={t("hearing.loudSoundReduction")}
        actions={
          <Toggle
            checked={protection.enabled}
            onChange={(checked) =>
              patchParams((draft) => (draft.loudSoundReduction.enabled = checked))
            }
          />
        }
      >
        <div className="protection-layout">
          <Slider
            label={t("hearing.intensity")}
            value={protection.intensity}
            min={0}
            max={1}
            step={0.01}
            onChange={(value) =>
              patchParams((draft) => (draft.loudSoundReduction.intensity = value))
            }
            format={(value) => formatPercent(value * 100)}
            disabled={!protection.enabled}
            accent="violet"
          />
          <GainReduction
            label={t("hearing.activation")}
            value={telemetry.protectionGrDb}
            max={18}
          />
        </div>
      </Panel>

      <Panel title={t("metrics.level")}>
        <div className="stack">
          <LevelMeter
            left={telemetry.rmsL}
            right={telemetry.rmsR}
            peakLeft={telemetry.peakL}
            peakRight={telemetry.peakR}
          />
          <div className="stat-row">
            <span>{t("hearing.peak")}</span>
            <strong>{dbfs(Math.max(telemetry.peakL, telemetry.peakR))}</strong>
          </div>
          <div className="stat-row">
            <span>{t("hearing.rms")}</span>
            <strong>{dbfs(Math.max(telemetry.rmsL, telemetry.rmsR))}</strong>
          </div>
        </div>
      </Panel>

      <Panel title={t("hearing.preset")}>
        <div className="stack">
          <div className="stat-row">
            <span>{t("hearing.state")}</span>
            <strong>{protection.enabled ? t("common.on") : t("common.off")}</strong>
          </div>
          <button
            type="button"
            className={`btn ${protectionActive ? "ghost" : "primary"}`}
            disabled={!protectionPreset || protectionActive}
            onClick={() => applyPreset("protection")}
          >
            <Icon name={protectionActive ? "check" : "shield"} size={16} />
            {protectionActive ? t("hearing.presetActive") : t("hearing.applyPreset")}
          </button>
        </div>
      </Panel>
    </div>
  );
}
