import { AdaptiveMeter } from "../components/AdaptiveMeter";
import { EqCurve } from "../components/EqCurve";
import { Icon } from "../components/Icons";
import { Panel } from "../components/Panel";
import { Slider, Toggle, VerticalSlider } from "../components/Controls";
import { isTauri } from "../lib/api";
import { formatDb, formatHz, formatPercent } from "../lib/format";
import { translate, type MessageKey } from "../lib/i18n";
import { ADAPTIVE_EQ_MAX_DB, EQ_FREQUENCIES } from "../lib/types";
import { useAppStore, useLanguage } from "../store/appStore";

export function Equalizer() {
  const view = useAppStore((state) => state.view);
  const patchParams = useAppStore((state) => state.patchParams);
  const importAutoEq = useAppStore((state) => state.importAutoEq);
  const language = useLanguage();
  const t = (key: MessageKey, vars?: Record<string, string | number>) =>
    translate(language, key, vars);

  if (!view) return <div className="page-loading">{t("common.loading")}</div>;
  const params = view.params;
  const autoEq = params.autoEq;

  const setBand = (index: number, gainDb: number) => {
    patchParams((draft) => {
      draft.eqDb[index] = Math.max(-12, Math.min(12, gainDb));
    });
  };

  return (
    <div className="page equalizer">
      <div className="eq-layout">
        <div className="eq-column">
          <Panel
            className="eq-graph-panel"
            title={t("eq.graphCaption")}
            actions={
              <span className={`badge ${autoEq.enabled && autoEq.filters.length > 0 ? "is-accent" : ""}`}>
                {t("eq.autoEq")} · {t("eq.filters", { count: autoEq.filters.length })}
              </span>
            }
          >
            <EqCurve params={params} onBandGain={setBand} ariaLabel={t("a11y.eqCurve")} />
            <div className="eq-graph-foot">
              <Slider
                label={t("eq.preamp")}
                value={params.preampDb}
                min={-24}
                max={12}
                step={0.5}
                onChange={(value) => patchParams((draft) => (draft.preampDb = value))}
                format={(value) => formatDb(value)}
              />
              <button
                type="button"
                className="btn ghost small"
                onClick={() =>
                  patchParams((draft) => {
                    draft.eqDb = new Array(10).fill(0);
                  })
                }
              >
                <Icon name="refresh" size={14} />
                {t("eq.reset")}
              </button>
            </div>
          </Panel>

          <Panel
            title={t("stage.eq")}
            actions={
              <span className="badge">{formatHz(EQ_FREQUENCIES[0])} — {formatHz(EQ_FREQUENCIES[EQ_FREQUENCIES.length - 1])}</span>
            }
          >
            <div className="band-sliders">
              {EQ_FREQUENCIES.map((freq, index) => (
                <VerticalSlider
                  key={freq}
                  value={params.eqDb[index] ?? 0}
                  min={-12}
                  max={12}
                  step={0.5}
                  onChange={(value) => setBand(index, value)}
                  label={formatHz(freq)}
                  sublabel={`${formatDb(params.eqDb[index] ?? 0)}`}
                  format={(value) => `${value > 0 ? "+" : ""}${value.toFixed(1)}`}
                />
              ))}
            </div>
          </Panel>
        </div>

        <div className="eq-column eq-side">
          <Panel
            title={t("eq.autoEq")}
            actions={
              <div className="inline-actions">
                <button
                  type="button"
                  className="btn ghost small"
                  onClick={() => importAutoEq()}
                  disabled={!isTauri}
                  title={t("eq.importAutoEq")}
                >
                  <Icon name="upload" size={14} />
                </button>
                <button
                  type="button"
                  className="btn ghost small"
                  onClick={() =>
                    patchParams((draft) => {
                      draft.autoEq.filters = [];
                      draft.autoEq.preampDb = 0;
                      draft.autoEq.enabled = false;
                      draft.autoEq.name = null;
                    })
                  }
                  disabled={autoEq.filters.length === 0}
                  title={t("eq.clear")}
                >
                  <Icon name="trash" size={14} />
                </button>
              </div>
            }
          >
            <div className="autoeq">
              <Toggle
                checked={autoEq.enabled}
                onChange={(checked) => patchParams((draft) => (draft.autoEq.enabled = checked))}
                label={t("eq.autoEq")}
                description={`${[
                  autoEq.name,
                  t("eq.filters", { count: autoEq.filters.length }),
                  formatDb(autoEq.preampDb),
                ]
                  .filter(Boolean)
                  .join(" · ")}`}
              />
              {autoEq.filters.length > 0 && (
                <ul className="filter-list">
                  {autoEq.filters.slice(0, 8).map((filter, index) => (
                    <li key={`${filter.freq}-${index}`}>
                      <span className={`filter-kind kind-${filter.kind}`}>
                        {filter.kind === "peaking" ? "PK" : filter.kind === "lowshelf" ? "LS" : "HS"}
                      </span>
                      <span>{formatHz(filter.freq)}</span>
                      <span>{formatDb(filter.gainDb)}</span>
                      <span>Q {filter.q.toFixed(2)}</span>
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </Panel>

          <Panel title={t("eq.tone")}>
            <div className="stack">
              <Slider
                label={t("eq.mid")}
                value={params.tone.midDb}
                min={-12}
                max={12}
                step={0.5}
                onChange={(value) => patchParams((draft) => (draft.tone.midDb = value))}
                format={(value) => formatDb(value)}
                accent="violet"
              />
              <Slider
                label={t("eq.treble")}
                value={params.tone.trebleDb}
                min={-12}
                max={12}
                step={0.5}
                onChange={(value) => patchParams((draft) => (draft.tone.trebleDb = value))}
                format={(value) => formatDb(value)}
                accent="violet"
              />
            </div>
          </Panel>

          <Panel
            title={t("eq.bass")}
            actions={
              <Toggle
                checked={params.bass.enabled}
                onChange={(checked) => patchParams((draft) => (draft.bass.enabled = checked))}
              />
            }
          >
            <div className="stack">
              <Slider
                label={t("eq.intensity")}
                value={params.bass.intensity}
                min={0}
                max={1}
                step={0.01}
                onChange={(value) => patchParams((draft) => (draft.bass.intensity = value))}
                format={(value) => formatPercent(value * 100)}
                disabled={!params.bass.enabled}
                accent="amber"
              />
              <Slider
                label={t("eq.cutoff")}
                value={params.bass.cutoffHz}
                min={30}
                max={250}
                step={1}
                onChange={(value) => patchParams((draft) => (draft.bass.cutoffHz = value))}
                format={(value) => formatHz(value)}
                disabled={!params.bass.enabled}
                accent="amber"
              />
              <Slider
                label={t("eq.harmonics")}
                value={params.bass.harmonics}
                min={0}
                max={1}
                step={0.01}
                onChange={(value) => patchParams((draft) => (draft.bass.harmonics = value))}
                format={(value) => formatPercent(value * 100)}
                disabled={!params.bass.enabled}
                accent="amber"
              />
            </div>
          </Panel>

          <Panel
            title={t("eq.adaptive")}
            actions={
              <Toggle
                checked={params.adaptiveEq.enabled}
                onChange={(checked) =>
                  patchParams((draft) => (draft.adaptiveEq.enabled = checked))
                }
              />
            }
          >
            <div className="stack">
              <Slider
                label={t("eq.intensity")}
                value={params.adaptiveEq.intensity}
                min={0}
                max={1}
                step={0.01}
                onChange={(value) =>
                  patchParams((draft) => (draft.adaptiveEq.intensity = value))
                }
                format={(value) => formatPercent(value * 100)}
                disabled={!params.adaptiveEq.enabled}
              />
              <div className="adaptive-head">
                <span>{t("eq.adaptiveLive")}</span>
                <span>±{ADAPTIVE_EQ_MAX_DB} dB</span>
              </div>
              <AdaptiveMeter active={params.adaptiveEq.enabled} />
            </div>
          </Panel>

          <div className="eq-mini-grid">
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
              />
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
                  accent="violet"
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
                  accent="violet"
                />
              </div>
            </Panel>
          </div>
        </div>
      </div>
    </div>
  );
}