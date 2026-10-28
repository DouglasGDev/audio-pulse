import { Icon, type IconName } from "../components/Icons";
import { Panel } from "../components/Panel";
import { Slider, Toggle } from "../components/Controls";
import { SpatialStage } from "../components/SpatialStage";
import { formatPercent } from "../lib/format";
import { translate, type MessageKey } from "../lib/i18n";
import type { SpatialMode } from "../lib/types";
import { useAppStore, useLanguage } from "../store/appStore";

const MODES: {
  value: SpatialMode;
  icon: IconName;
  title: MessageKey;
  description: MessageKey;
}[] = [
  { value: "off", icon: "headphone", title: "spatial.off", description: "spatial.offDesc" },
  {
    value: "binaural",
    icon: "spatial",
    title: "spatial.binaural",
    description: "spatial.binauralDesc",
  },
  {
    value: "hrtf",
    icon: "wave",
    title: "spatial.hrtf",
    description: "spatial.hrtfDesc",
  },
  {
    value: "surround",
    icon: "speaker",
    title: "spatial.surround",
    description: "spatial.surroundDesc",
  },
];

export function Spatial() {
  const view = useAppStore((state) => state.view);
  const setSpatialMode = useAppStore((state) => state.setSpatialMode);
  const patchParams = useAppStore((state) => state.patchParams);
  const language = useLanguage();
  const t = (key: MessageKey) => translate(language, key);

  if (!view) return <div className="page-loading">{t("common.loading")}</div>;
  const params = view.params;
  const spatial = params.spatial;
  const enabled = spatial.mode !== "off";

  return (
    <div className="page spatial">
      <div className="mode-cards">
        {MODES.map((mode) => (
          <button
            key={mode.value}
            type="button"
            className={`mode-card ${spatial.mode === mode.value ? "is-active" : ""}`}
            onClick={() => setSpatialMode(mode.value)}
          >
            <span className="mode-icon">
              <Icon name={mode.icon} size={26} />
            </span>
            <span className="mode-text">
              <strong>{t(mode.title)}</strong>
              <em>{t(mode.description)}</em>
            </span>
            <span className="mode-check">
              {spatial.mode === mode.value && <Icon name="check" size={15} />}
            </span>
          </button>
        ))}
      </div>

      <div className="spatial-grid">
        <Panel
          className="stage-panel"
          title={t("spatial.stage")}
          actions={
            <div className="inline-actions">
              <span className={`badge ${enabled ? "is-accent" : ""}`}>
                {t(MODES.find((mode) => mode.value === spatial.mode)?.title ?? "spatial.off")}
              </span>
              <span className="badge">
                {t("spatial.width")} {spatial.width.toFixed(2)}
              </span>
            </div>
          }
        >
          <div className={`stage-wrap ${enabled ? "is-on" : ""}`}>
            <SpatialStage
              mode={spatial.mode}
              width={spatial.width}
              room={spatial.room}
              active={enabled && view.engine.running && !params.bypass}
              ariaLabel={t("a11y.spatialStage")}
            />
          </div>
        </Panel>

        <div className="spatial-side">
          <Panel title={t("spatial.title")}>
            <div className="stack">
              <Slider
                label={t("spatial.width")}
                value={spatial.width}
                min={0}
                max={1.5}
                step={0.01}
                onChange={(value) => patchParams((draft) => (draft.spatial.width = value))}
                format={(value) => `${value.toFixed(2)}×`}
                disabled={!enabled}
                accent="violet"
              />
              <Slider
                label={t("spatial.room")}
                value={spatial.room}
                min={0}
                max={1}
                step={0.01}
                onChange={(value) => patchParams((draft) => (draft.spatial.room = value))}
                format={(value) => formatPercent(value * 100)}
                disabled={!enabled}
                accent="violet"
              />
            </div>
          </Panel>

          <Panel
            title={t("eq.crossfeed")}
            actions={
              <Toggle
                checked={params.crossfeed.enabled}
                onChange={(checked) =>
                  patchParams((draft) => (draft.crossfeed.enabled = checked))
                }
              />
            }
          >
            <div className="stack">
              <Slider
                label={t("eq.amount")}
                value={params.crossfeed.amount}
                min={0}
                max={1}
                step={0.01}
                onChange={(value) =>
                  patchParams((draft) => (draft.crossfeed.amount = value))
                }
                format={(value) => formatPercent(value * 100)}
                disabled={!params.crossfeed.enabled}
              />
              <Slider
                label={t("eq.delay")}
                value={params.crossfeed.delayMs}
                min={0.05}
                max={1}
                step={0.01}
                onChange={(value) =>
                  patchParams((draft) => (draft.crossfeed.delayMs = value))
                }
                format={(value) => `${value.toFixed(2)} ms`}
                disabled={!params.crossfeed.enabled}
              />
            </div>
          </Panel>

          <Panel
            title={t("eq.loudness")}
            actions={
              <Toggle
                checked={params.loudness.enabled}
                onChange={(checked) =>
                  patchParams((draft) => (draft.loudness.enabled = checked))
                }
              />
            }
          >
            <Slider
              label={t("eq.loudnessIntensity")}
              value={params.loudness.intensity}
              min={0}
              max={1}
              step={0.01}
              onChange={(value) =>
                patchParams((draft) => (draft.loudness.intensity = value))
              }
              format={(value) => formatPercent(value * 100)}
              disabled={!params.loudness.enabled}
              accent="amber"
            />
          </Panel>
        </div>
      </div>
    </div>
  );
}