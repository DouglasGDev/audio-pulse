import { useEffect, useState, type CSSProperties } from "react";

import { LevelMeter } from "../components/LevelMeter";
import { Icon } from "../components/Icons";
import { Panel } from "../components/Panel";
import { Spectrum } from "../components/Spectrum";
import { Toggle } from "../components/Controls";
import { formatHz, formatPercent, engineModeKey } from "../lib/format";
import { presetTitle, translate, type MessageKey } from "../lib/i18n";
import { useAppStore, useLanguage, type Page } from "../store/appStore";

function Metric({ label, value, tone }: { label: string; value: string; tone?: "warn" }) {
  return (
    <div className={`metric ${tone ? `is-${tone}` : ""}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

export function Dashboard() {
  const view = useAppStore((state) => state.view);
  const telemetry = useAppStore((state) => state.telemetry);
  const engineBusy = useAppStore((state) => state.engineBusy);
  const cableBusy = useAppStore((state) => state.cableBusy);
  const startEngine = useAppStore((state) => state.startEngine);
  const stopEngine = useAppStore((state) => state.stopEngine);
  const setPage = useAppStore((state) => state.setPage);
  const applyPreset = useAppStore((state) => state.applyPreset);
  const setVolume = useAppStore((state) => state.setVolume);
  const toggleBypass = useAppStore((state) => state.toggleBypass);
  const installVirtualCable = useAppStore((state) => state.installVirtualCable);
  const language = useLanguage();
  const t = (key: MessageKey, vars?: Record<string, string | number>) =>
    translate(language, key, vars);

  const [lastVolume, setLastVolume] = useState(0.7);

  const params = view?.params;
  const engine = view?.engine;
  const devices = view?.devices;
  const running = engine?.running ?? false;
  const bypass = params?.bypass ?? false;
  const volume = params?.volume ?? 1;

  useEffect(() => {
    if (volume > 0.001) setLastVolume(volume);
  }, [volume]);

  if (!view || !params || !engine || !devices) {
    return <div className="page-loading">{t("common.loading")}</div>;
  }

  const modePresets = view.presets.filter(
    (preset) => preset.builtIn && preset.category !== "headphone",
  );

  const stages: { key: MessageKey; on: boolean; page: Page }[] = [
    {
      key: "stage.preamp",
      on: Math.abs(params.preampDb) > 0.01,
      page: "equalizer",
    },
    {
      key: "stage.autoeq",
      on: params.autoEq.enabled && params.autoEq.filters.length > 0,
      page: "equalizer",
    },
    {
      key: "stage.eq",
      on: params.eqDb.some((gain) => Math.abs(gain) > 0.05),
      page: "equalizer",
    },
    {
      key: "stage.tone",
      on: Math.abs(params.tone.midDb) > 0.01 || Math.abs(params.tone.trebleDb) > 0.01,
      page: "equalizer",
    },
    { key: "stage.bass", on: params.bass.enabled, page: "equalizer" },
    { key: "stage.spatial", on: params.spatial.mode !== "off", page: "spatial" },
    { key: "stage.crossfeed", on: params.crossfeed.enabled, page: "equalizer" },
    { key: "stage.adaptive", on: params.adaptiveEq.enabled, page: "equalizer" },
    { key: "stage.compressor", on: params.compressor.enabled, page: "dynamics" },
    { key: "stage.volume", on: volume > 0.001, page: "dashboard" },
    { key: "stage.loudness", on: params.loudness.enabled, page: "equalizer" },
    { key: "stage.limiter", on: params.limiter.enabled, page: "dynamics" },
  ];

  const status = bypass
    ? t("status.bypassed")
    : running
      ? t("status.running")
      : t("status.stopped");

  const inputName =
    engine.inputDevice ??
    view.settings.inputDevice ??
    devices.defaultInput ??
    t("dashboard.systemDefault");
  const outputName =
    engine.outputDevice ??
    view.settings.outputDevice ??
    devices.defaultOutput ??
    t("dashboard.systemDefault");

  return (
    <div className="page dashboard">
      <section className={`engine-hero ${running ? "is-running" : ""}`}>
        <div className="hero-glow" />
        <div className="hero-body">
          <div className="hero-status">
            <span className={`status-dot ${running ? "is-live" : ""}`} />
            <span className="hero-state">{status}</span>
            <span className="badge">{t(engineModeKey(engine.mode))}</span>
            {bypass && <span className="badge is-warn">{t("status.bypassed")}</span>}
          </div>

          <div className="route">
            <div className="route-node">
              <span className="route-icon">
                <Icon name="cable" size={18} />
              </span>
              <span className="route-label">{t("dashboard.source")}</span>
              <strong title={inputName}>{inputName}</strong>
            </div>
            <span className="route-arrow">
              <Icon name="arrow-right" size={18} />
            </span>
            <div className="route-node">
              <span className="route-icon">
                <Icon name="headphone" size={18} />
              </span>
              <span className="route-label">{t("dashboard.output")}</span>
              <strong title={outputName}>{outputName}</strong>
            </div>
          </div>

          <div className="hero-actions">
            <button
              type="button"
              className={`btn ${running ? "" : "primary"}`}
              disabled={engineBusy}
              onClick={() => (running ? stopEngine() : startEngine("route"))}
            >
              <Icon name={running ? "stop" : "play"} size={16} />
              {engineBusy
                ? t("engine.starting")
                : running
                  ? t("engine.stop")
                  : t("engine.startRoute")}
            </button>
            <button
              type="button"
              className="btn ghost"
              onClick={() => startEngine("noise")}
              disabled={engineBusy}
              title={t("engine.modeNoise")}
            >
              <Icon name="wave" size={16} />
              <span className="hide-narrow">{t("engine.modeNoise")}</span>
            </button>
            <button
              type="button"
              className="btn ghost"
              onClick={() => startEngine("sweep")}
              disabled={engineBusy}
              title={t("engine.modeSweep")}
            >
              <Icon name="sparkle" size={16} />
              <span className="hide-narrow">{t("engine.modeSweep")}</span>
            </button>
            {!devices.virtualCableDetected && devices.platform !== "linux" && (
              <button
                type="button"
                className="hint-chip is-warn"
                disabled={cableBusy}
                onClick={() => installVirtualCable()}
              >
                <Icon name="cable" size={15} />
                {cableBusy ? t("settings.installingCable") : t("engine.installCable")}
              </button>
            )}
          </div>
        </div>

        <div className="hero-metrics">
          <Metric label={t("metrics.cpu")} value={formatPercent(telemetry.cpuPercent, 1)} />
          <Metric
            label={t("metrics.latency")}
            value={`${(telemetry.latencyMs || engine.latencyMs).toFixed(1)} ms`}
          />
          <Metric
            label={t("metrics.xruns")}
            value={String(telemetry.xruns || engine.xruns)}
            tone={(telemetry.xruns || engine.xruns) > 0 ? "warn" : undefined}
          />
          <Metric
            label={t("metrics.sampleRate")}
            value={engine.sampleRate ? formatHz(engine.sampleRate) : "—"}
          />
        </div>
      </section>

      <div className="dash-grid">
        <Panel
          className="spectrum-panel"
          title={t("dashboard.signal")}
          actions={
            <span className={`live-pill ${running ? "is-live" : ""}`}>
              <span className="status-dot" />
              {running ? t("status.running") : t("status.stopped")}
            </span>
          }
        >
          <div className="spectrum-wrap">
            <Spectrum />
          </div>
          <div className="spectrum-foot">
            <LevelMeter
              left={telemetry.rmsL}
              right={telemetry.rmsR}
              peakLeft={telemetry.peakL}
              peakRight={telemetry.peakR}
            />
          </div>
        </Panel>

        <div className="dash-side">
          <Panel title={t("volume.master")}>
            <div className="volume-block">
              <button
                type="button"
                className="volume-icon"
                onClick={() => setVolume(volume > 0.001 ? 0 : lastVolume || 0.7)}
                title={volume > 0.001 ? t("common.off") : t("common.on")}
              >
                <Icon name={volume <= 0.001 ? "mute" : "volume"} size={22} />
              </button>
              <div className="volume-readout">
                <strong>{Math.round(volume * 100)}</strong>
                <span>%</span>
              </div>
            </div>
            <input
              type="range"
              className="range range-cyan range-lg"
              min={0}
              max={1}
              step={0.005}
              value={volume}
              aria-label={t("volume.master")}
              style={{ "--fill": `${volume * 100}%` } as CSSProperties}
              onChange={(event) => setVolume(Number(event.target.value))}
            />
            <div className="master-toggles">
              <Toggle
                checked={bypass}
                onChange={() => toggleBypass()}
                label={t("topbar.bypass")}
              />
            </div>
          </Panel>

          <Panel title={t("dashboard.quickPresets")}>
            <div className="quick-grid">
              {modePresets.map((preset) => (
                <button
                  key={preset.id}
                  type="button"
                  className={`quick-preset ${view.activePresetId === preset.id ? "is-active" : ""}`}
                  onClick={() => applyPreset(preset.id)}
                  title={presetTitle(language, preset)}
                >
                  <span className="quick-dot" />
                  <span>{presetTitle(language, preset)}</span>
                </button>
              ))}
            </div>
          </Panel>
        </div>
      </div>

      <Panel
        title={t("dashboard.chain")}
        actions={
          <span className={`live-pill ${running ? "is-live" : ""}`}>
            {t("metrics.level")} {Math.round(telemetry.rmsL * 100)}%
          </span>
        }
      >
        <div className={`chain ${bypass ? "is-bypassed" : ""}`}>
          {stages.map((stage) => (
            <button
              key={stage.key}
              type="button"
              className={`stage ${stage.on ? "is-on" : ""}`}
              onClick={() => setPage(stage.page)}
            >
              <span className="stage-dot" />
              <span>{t(stage.key)}</span>
              <em>{stage.on ? t("common.on") : t("common.off")}</em>
            </button>
          ))}
        </div>
      </Panel>

      {view.warnings.length > 0 && (
        <Panel title={t("dashboard.warnings")} className="warnings-panel">
          <ul className="warning-list">
            {view.warnings.map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
        </Panel>
      )}
    </div>
  );
}