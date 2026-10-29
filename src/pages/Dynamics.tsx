import { useMemo } from "react";

import { Panel } from "../components/Panel";
import { Slider, Toggle } from "../components/Controls";
import { GainReduction } from "../components/GainReduction";
import { formatDb } from "../lib/format";
import { translate, type MessageKey } from "../lib/i18n";
import type { CompressorParams } from "../lib/types";
import { useAppStore, useLanguage } from "../store/appStore";

function CompressorCurve({ params, ariaLabel }: { params: CompressorParams; ariaLabel: string }) {
  const width = 300;
  const height = 180;
  const pad = 16;
  const span = 60;

  const { line, unity, area } = useMemo(() => {
    const xFor = (db: number) => pad + ((db + span) / span) * (width - pad * 2);
    const yFor = (db: number) => height - pad - ((db + span) / span) * (height - pad * 2);
    const points: { x: number; y: number }[] = [];
    for (let input = -span; input <= 0; input += 0.5) {
      let output = input;
      const threshold = params.thresholdDb;
      const knee = params.kneeDb;
      if (knee > 0.01 && input > threshold - knee / 2 && input < threshold + knee / 2) {
        const x = input - (threshold - knee / 2);
        output = input + (1 / params.ratio - 1) * ((x * x) / (2 * knee));
      } else if (input >= threshold + knee / 2) {
        output = threshold + (input - threshold) / params.ratio;
      }
      output = Math.max(-span, Math.min(0, output + params.makeupDb));
      points.push({ x: xFor(input), y: yFor(output) });
    }
    const path = points
      .map((point, index) => `${index === 0 ? "M" : "L"}${point.x.toFixed(1)},${point.y.toFixed(1)}`)
      .join(" ");
    return {
      line: path,
      unity: `M ${xFor(-span)} ${yFor(-span)} L ${xFor(0)} ${yFor(0)}`,
      area: `${path} L ${xFor(0)} ${yFor(-span)} L ${xFor(-span)} ${yFor(-span)} Z`,
    };
  }, [params.thresholdDb, params.ratio, params.kneeDb, params.makeupDb]);

  return (
    <svg className="compressor-curve" viewBox={`0 0 ${width} ${height}`} role="img" aria-label={ariaLabel}>
      <defs>
        <linearGradient id="comp-fill" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--accent-2)" stopOpacity="0.3" />
          <stop offset="100%" stopColor="var(--accent-2)" stopOpacity="0.03" />
        </linearGradient>
      </defs>
      <rect x={pad} y={pad} width={width - pad * 2} height={height - pad * 2} rx="8" fill="var(--graph-bg)" />
      {[0.25, 0.5, 0.75].map((fraction) => (
        <line
          key={fraction}
          x1={pad}
          x2={width - pad}
          y1={pad + (height - pad * 2) * fraction}
          y2={pad + (height - pad * 2) * fraction}
          stroke="var(--line)"
          strokeWidth="0.6"
          opacity="0.5"
        />
      ))}
      <path d={area} fill="url(#comp-fill)" />
      <path d={unity} fill="none" stroke="var(--line-strong)" strokeWidth="1" strokeDasharray="4 5" />
      <path
        d={line}
        fill="none"
        stroke="var(--accent-2)"
        strokeWidth="2.4"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

export function Dynamics() {
  const view = useAppStore((state) => state.view);
  const telemetry = useAppStore((state) => state.telemetry);
  const patchParams = useAppStore((state) => state.patchParams);
  const language = useLanguage();
  const t = (key: MessageKey) => translate(language, key);

  if (!view) return <div className="page-loading">{t("common.loading")}</div>;
  const params = view.params;
  const compressor = params.compressor;
  const limiter = params.limiter;

  return (
    <div className="page dynamics">
      <Panel
        className="span-2 compressor-panel"
        title={t("dynamics.compressor")}
        actions={
          <Toggle
            checked={compressor.enabled}
            onChange={(checked) =>
              patchParams((draft) => (draft.compressor.enabled = checked))
            }
          />
        }
      >
        <div className="compressor-layout">
          <div className="slider-grid">
            <Slider
              label={t("dynamics.threshold")}
              value={compressor.thresholdDb}
              min={-60}
              max={0}
              step={0.5}
              onChange={(value) =>
                patchParams((draft) => (draft.compressor.thresholdDb = value))
              }
              format={(value) => formatDb(value)}
              disabled={!compressor.enabled}
            />
            <Slider
              label={t("dynamics.ratio")}
              value={compressor.ratio}
              min={1}
              max={20}
              step={0.1}
              onChange={(value) =>
                patchParams((draft) => (draft.compressor.ratio = value))
              }
              format={(value) => `${value.toFixed(1)}:1`}
              disabled={!compressor.enabled}
              accent="violet"
            />
            <Slider
              label={t("dynamics.attack")}
              value={compressor.attackMs}
              min={0.1}
              max={200}
              step={0.1}
              onChange={(value) =>
                patchParams((draft) => (draft.compressor.attackMs = value))
              }
              format={(value) => `${value.toFixed(1)} ms`}
              disabled={!compressor.enabled}
              accent="violet"
            />
            <Slider
              label={t("dynamics.release")}
              value={compressor.releaseMs}
              min={20}
              max={2000}
              step={1}
              onChange={(value) =>
                patchParams((draft) => (draft.compressor.releaseMs = value))
              }
              format={(value) => `${Math.round(value)} ms`}
              disabled={!compressor.enabled}
              accent="violet"
            />
            <Slider
              label={t("dynamics.knee")}
              value={compressor.kneeDb}
              min={0}
              max={24}
              step={0.1}
              onChange={(value) =>
                patchParams((draft) => (draft.compressor.kneeDb = value))
              }
              format={(value) => formatDb(value)}
              disabled={!compressor.enabled}
            />
            <Slider
              label={t("dynamics.makeup")}
              value={compressor.makeupDb}
              min={0}
              max={24}
              step={0.1}
              onChange={(value) =>
                patchParams((draft) => (draft.compressor.makeupDb = value))
              }
              format={(value) => formatDb(value)}
              disabled={!compressor.enabled}
            />
          </div>
          <div className="compressor-side">
            <CompressorCurve params={compressor} ariaLabel={t("a11y.compressorCurve")} />
            <GainReduction
              label={t("dynamics.gainReduction")}
              value={telemetry.compressorGrDb}
              max={12}
            />
          </div>
        </div>
      </Panel>

      <Panel
        title={t("dynamics.limiter")}
        actions={
          <Toggle
            checked={limiter.enabled}
            onChange={(checked) => patchParams((draft) => (draft.limiter.enabled = checked))}
          />
        }
      >
        <div className="stack">
          <Slider
            label={t("dynamics.ceiling")}
            value={limiter.ceilingDb}
            min={-12}
            max={0}
            step={0.1}
            onChange={(value) => patchParams((draft) => (draft.limiter.ceilingDb = value))}
            format={(value) => formatDb(value)}
            disabled={!limiter.enabled}
          />
          <Slider
            label={t("dynamics.release")}
            value={limiter.releaseMs}
            min={10}
            max={1000}
            step={1}
            onChange={(value) => patchParams((draft) => (draft.limiter.releaseMs = value))}
            format={(value) => `${Math.round(value)} ms`}
            disabled={!limiter.enabled}
          />
          <GainReduction
            label={t("dynamics.gainReduction")}
            value={telemetry.limiterGrDb}
            max={6}
          />
        </div>
      </Panel>

      <Panel title={t("metrics.level")}>
        <div className="stack">
          <div className="stat-row">
            <span>{t("dynamics.compressor")}</span>
            <strong>{compressor.enabled ? t("common.on") : t("common.off")}</strong>
          </div>
          <div className="stat-row">
            <span>{t("dynamics.limiter")}</span>
            <strong>{limiter.enabled ? t("common.on") : t("common.off")}</strong>
          </div>
          <div className="stat-row">
            <span>{t("metrics.cpu")}</span>
            <strong>{telemetry.cpuPercent.toFixed(1)}%</strong>
          </div>
          <div className="stat-row">
            <span>{t("metrics.latency")}</span>
            <strong>{telemetry.latencyMs.toFixed(1)} ms</strong>
          </div>
          <div className="stat-row">
            <span>{t("metrics.xruns")}</span>
            <strong>{telemetry.xruns}</strong>
          </div>
        </div>
      </Panel>
    </div>
  );
}