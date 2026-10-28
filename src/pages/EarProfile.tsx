import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ChangeEvent,
} from "react";

import {
  EarCropEditor,
  type EarCropEditorHandle,
} from "../components/EarCropEditor";
import { Icon } from "../components/Icons";
import { Panel } from "../components/Panel";
import { ParametricCurve } from "../components/ParametricCurve";
import { Toggle } from "../components/Controls";
import { api, isTauri, pickEarPhoto } from "../lib/api";
import { formatDb, formatHz, formatPercent } from "../lib/format";
import { translate, type MessageKey } from "../lib/i18n";
import type {
  AutoEqFilter,
  AutoEqParams,
  EarAnalysis,
  EarModelProgress,
  EarModelStatus,
} from "../lib/types";
import { useAppStore, useLanguage } from "../store/appStore";

const WARNING_CODES = new Set([
  "low_contrast",
  "no_basin",
  "no_ear",
  "off_center",
  "dark",
  "overexposed",
]);

function sameFilters(left: AutoEqFilter[], right: AutoEqFilter[]): boolean {
  if (left.length !== right.length) return false;
  return left.every((filter, index) => {
    const other = right[index];
    return (
      filter.kind === other.kind &&
      Math.abs(filter.freq - other.freq) < 0.5 &&
      Math.abs(filter.gainDb - other.gainDb) < 0.06 &&
      Math.abs(filter.q - other.q) < 0.01
    );
  });
}

function profileIsApplied(analysis: EarAnalysis, autoEq: AutoEqParams): boolean {
  return (
    sameFilters(analysis.filters, autoEq.filters) &&
    Math.abs(analysis.preampDb - autoEq.preampDb) < 0.06
  );
}

function megabytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function EarProfile() {
  const view = useAppStore((state) => state.view);
  const earAnalysis = useAppStore((state) => state.earAnalysis);
  const earBackup = useAppStore((state) => state.earBackup);
  const earPhoto = useAppStore((state) => state.earPhoto);
  const setEarAnalysis = useAppStore((state) => state.setEarAnalysis);
  const setEarBackup = useAppStore((state) => state.setEarBackup);
  const setEarPhoto = useAppStore((state) => state.setEarPhoto);
  const patchParams = useAppStore((state) => state.patchParams);
  const toast = useAppStore((state) => state.toast);
  const language = useLanguage();
  const t = (key: MessageKey, vars?: Record<string, string | number>) =>
    translate(language, key, vars);

  const editorRef = useRef<EarCropEditorHandle>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const autoDownloadTried = useRef(false);
  const [model, setModel] = useState<EarModelStatus | null>(null);
  const [progress, setProgress] = useState<EarModelProgress | null>(null);
  const [downloading, setDownloading] = useState(false);
  const [loadingPhoto, setLoadingPhoto] = useState(false);
  const [analyzing, setAnalyzing] = useState(false);
  const [photoReady, setPhotoReady] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    api
      .earModelStatus()
      .then((status) => {
        if (!cancelled) setModel(status);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  const photoError = useCallback(() => {
    setError(translate(language, "ear.badFile"));
  }, [language]);

  const autoEq = view?.params.autoEq;
  const analysis = earAnalysis;
  const applied =
    analysis && autoEq ? profileIsApplied(analysis, autoEq) : false;
  const trustworthy =
    !!analysis &&
    analysis.confidence >= 0.25 &&
    !analysis.warnings.includes("no_basin") &&
    !analysis.warnings.includes("no_ear");
  const confidence = analysis ? Math.round(analysis.confidence * 100) : 0;
  const downloadPercent =
    progress && progress.total > 0
      ? Math.min(100, Math.round((progress.received / progress.total) * 100))
      : 0;

  const showError = (value: unknown) => {
    const message = value instanceof Error ? value.message : String(value);
    setError(message);
    toast(translate(language, "toast.error", { message }), "error");
  };

  const choosePhoto = async () => {
    setError(null);
    if (!isTauri) {
      fileInputRef.current?.click();
      return;
    }
    try {
      const path = await pickEarPhoto();
      if (!path) return;
      setLoadingPhoto(true);
      const photo = await api.loadEarPhoto(path);
      setEarPhoto({
        dataUrl: photo.dataUrl,
        name: path.split(/[\\/]/).pop() ?? null,
      });
      setEarAnalysis(null);
    } catch (value) {
      showError(value);
    } finally {
      setLoadingPhoto(false);
    }
  };

  const onFilePicked = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    if (file.type !== "image/jpeg" && file.type !== "image/png") {
      setError(t("ear.badFile"));
      return;
    }
    setError(null);
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result !== "string") return;
      setEarPhoto({ dataUrl: reader.result, name: file.name });
      setEarAnalysis(null);
    };
    reader.onerror = () => setError(t("ear.badFile"));
    reader.readAsDataURL(file);
  };

  const analyze = async () => {
    const crop = editorRef.current?.exportCrop();
    if (!crop) return;
    setAnalyzing(true);
    setError(null);
    try {
      const result = await api.analyzeEar(crop);
      setEarAnalysis(result);
    } catch (value) {
      showError(value);
    } finally {
      setAnalyzing(false);
    }
  };

  const downloadModel = async () => {
    if (!isTauri || downloading) return;
    setDownloading(true);
    setError(null);
    setProgress(null);
    const unlisten = api.onEarModelProgress(setProgress);
    try {
      const status = await api.downloadEarModel();
      setModel(status);
    } catch (value) {
      showError(value);
    } finally {
      unlisten();
      setDownloading(false);
      setProgress(null);
    }
  };

  useEffect(() => {
    if (!isTauri || !model || model.installed || autoDownloadTried.current) {
      return;
    }
    autoDownloadTried.current = true;
    void downloadModel();
  }, [model]);

  const applyProfile = () => {
    if (!analysis || !view) return;
    if (!earBackup) setEarBackup(structuredClone(view.params.autoEq));
    patchParams((params) => {
      params.autoEq.filters = analysis.filters.map((filter) => ({ ...filter }));
      params.autoEq.preampDb = analysis.preampDb;
      params.autoEq.enabled = true;
    });
    toast(t("ear.toastApplied"), "success");
  };

  const removeProfile = () => {
    patchParams((params) => {
      params.autoEq.enabled = false;
      params.autoEq.preampDb = 0;
      params.autoEq.filters = [];
    });
    toast(t("ear.toastRemoved"), "info");
  };

  const restoreBackup = () => {
    if (!earBackup) return;
    const backup = structuredClone(earBackup);
    patchParams((params) => {
      params.autoEq = backup;
    });
    setEarBackup(null);
    toast(t("ear.toastRestored"), "success");
  };

  if (!view) return <div className="page-loading">{t("common.loading")}</div>;

  return (
    <div className="page ear-page">
      <div className="ear-layout">
        <div className="ear-column">
          <Panel
            title={t("ear.photo")}
            subtitle={earPhoto?.name ?? undefined}
            actions={
              <div className="inline-actions">
                {earPhoto && (
                  <button
                    type="button"
                    className="btn ghost small"
                    onClick={() => void choosePhoto()}
                    disabled={loadingPhoto}
                  >
                    <Icon name="upload" size={14} />
                    {t("ear.change")}
                  </button>
                )}
              </div>
            }
          >
            {earPhoto ? (
              <EarCropEditor
                ref={editorRef}
                src={earPhoto.dataUrl}
                ariaLabel={t("a11y.earPhoto")}
                onReady={setPhotoReady}
                onError={photoError}
              />
            ) : (
              <button
                type="button"
                className="ear-drop"
                onClick={() => void choosePhoto()}
                disabled={loadingPhoto}
              >
                <Icon name="ear" size={38} />
                <strong>
                  {loadingPhoto ? t("common.loading") : t("ear.choose")}
                </strong>
                <span>{t("ear.guide")}</span>
              </button>
            )}

            <input
              ref={fileInputRef}
              type="file"
              accept="image/jpeg,image/png"
              className="ear-file-input"
              onChange={onFilePicked}
            />

            <div className="ear-actions">
              <span className="ear-local-note">{t("ear.localOnly")}</span>
              <button
                type="button"
                className="btn primary"
                onClick={() => void analyze()}
                disabled={!photoReady || !model?.installed || analyzing}
                title={!model?.installed ? t("ear.missingModel") : undefined}
              >
                {analyzing ? t("ear.analyzing") : t("ear.analyze")}
              </button>
            </div>

            {error && <div className="ear-error">{error}</div>}
          </Panel>

          <Panel
            title={t("ear.result")}
            actions={
              analysis && (
                <span className="badge">
                  {formatHz(analysis.resonanceHz)}
                </span>
              )
            }
          >
            {analysis ? (
              <div className="ear-result">
                <div className="ear-overlay">
                  <img
                    src={`data:image/png;base64,${analysis.overlayPng}`}
                    alt={t("a11y.earPhoto")}
                  />
                </div>

                <div className="ear-confidence">
                  <div className="ear-confidence-head">
                    <span>{t("ear.confidence")}</span>
                    <strong>{confidence}%</strong>
                  </div>
                  <div className="ear-confidence-track">
                    <span
                      className={
                        confidence >= 60
                          ? "is-good"
                          : confidence >= 30
                            ? "is-warn"
                            : "is-bad"
                      }
                      style={{ width: `${confidence}%` }}
                    />
                  </div>
                </div>

                <div className="ear-metrics">
                  <div className="metric">
                    <span>{t("ear.conchaDepth")}</span>
                    <strong>
                      {formatPercent(analysis.metrics.conchaDepth * 100)}
                    </strong>
                  </div>
                  <div className="metric">
                    <span>{t("ear.conchaSize")}</span>
                    <strong>
                      {formatPercent(analysis.metrics.conchaSize * 100, 1)}
                    </strong>
                  </div>
                  <div className="metric">
                    <span>{t("ear.helix")}</span>
                    <strong>
                      {formatPercent(analysis.metrics.helixProminence * 100)}
                    </strong>
                  </div>
                  <div className="metric">
                    <span>{t("ear.depthRange")}</span>
                    <strong>
                      {formatPercent(analysis.metrics.depthRange * 100)}
                    </strong>
                  </div>
                </div>

                {analysis.warnings.length > 0 && (
                  <ul className="warning-list">
                    {analysis.warnings.map((code) => (
                      <li key={code}>
                        {WARNING_CODES.has(code)
                          ? t(`ear.warning.${code}` as MessageKey)
                          : code}
                      </li>
                    ))}
                  </ul>
                )}

                <p className="ear-meta">
                  {analysis.model} ·{" "}
                  {t("ear.analyzedIn", { ms: analysis.elapsedMs })}
                </p>
              </div>
            ) : (
              <div className="empty-state">{t("ear.emptyResult")}</div>
            )}
          </Panel>
        </div>

        <div className="ear-column ear-side">
          <Panel
            title={t("ear.model")}
            actions={
              model?.installed ? (
                <span className="badge is-accent">{t("ear.modelInstalled")}</span>
              ) : undefined
            }
          >
            <div className="ear-model">
              <div className="ear-model-text">
                <strong>{model?.name ?? "Depth Anything V2 Small"}</strong>
                <span>
                  {model?.license ?? "Apache-2.0"} ·{" "}
                  {megabytes(
                    model?.installed ? model.bytes : (model?.expectedBytes ?? 0),
                  )}
                </span>
              </div>
              {!model?.installed && (
                <button
                  type="button"
                  className="btn small"
                  onClick={() => void downloadModel()}
                  disabled={!isTauri || downloading || !model}
                >
                  <Icon name="download" size={14} />
                  {downloading
                    ? t("ear.downloading", { percent: downloadPercent })
                    : t("ear.downloadModel")}
                </button>
              )}
            </div>
            {downloading && (
              <div className="ear-progress">
                <span style={{ width: `${downloadPercent}%` }} />
              </div>
            )}
            {!isTauri && <p className="ear-note">{t("ear.nativeOnly")}</p>}
          </Panel>

          <Panel
            title={t("ear.curve")}
            actions={
              analysis && (
                <span className="badge">
                  {t("eq.preamp")} {formatDb(analysis.preampDb)}
                </span>
              )
            }
          >
            {analysis ? (
              <>
                <ParametricCurve
                  filters={analysis.filters}
                  ariaLabel={t("a11y.earCurve")}
                />
                <ul className="filter-list ear-filters">
                  {analysis.filters.map((filter, index) => (
                    <li key={`${filter.freq}-${index}`}>
                      <span className={`filter-kind kind-${filter.kind}`}>PK</span>
                      <span>{formatHz(filter.freq)}</span>
                      <span>{formatDb(filter.gainDb)}</span>
                      <span>Q {filter.q.toFixed(2)}</span>
                    </li>
                  ))}
                </ul>
              </>
            ) : (
              <div className="empty-state">{t("ear.emptyResult")}</div>
            )}
          </Panel>

          <Panel title={applied ? t("ear.applied") : t("ear.apply")}>
            {analysis || earBackup ? (
              <div className="ear-apply">
                {analysis && applied && autoEq ? (
                  <Toggle
                    checked={autoEq.enabled}
                    onChange={(checked) =>
                      patchParams((params) => {
                        params.autoEq.enabled = checked;
                      })
                    }
                    label={t("ear.enabled")}
                    description={`${t("eq.filters", { count: autoEq.filters.length })} · ${formatDb(autoEq.preampDb)}`}
                  />
                ) : analysis ? (
                  <p className="ear-note">
                    {autoEq && autoEq.filters.length > 0
                      ? t("ear.replaces", { count: autoEq.filters.length })
                      : t("ear.replacesNone")}
                  </p>
                ) : (
                  <p className="ear-note">{t("ear.previousSaved")}</p>
                )}

                {analysis && !trustworthy && (
                  <p className="ear-note is-warn">{t("ear.lowConfidence")}</p>
                )}

                <div className="ear-apply-actions">
                  {analysis && !applied && (
                    <button
                      type="button"
                      className="btn primary"
                      onClick={applyProfile}
                      disabled={!trustworthy}
                    >
                      {t("ear.apply")}
                    </button>
                  )}
                  {analysis && applied && (
                    <button
                      type="button"
                      className="btn ghost"
                      onClick={removeProfile}
                    >
                      {t("ear.remove")}
                    </button>
                  )}
                  {earBackup && (
                    <button
                      type="button"
                      className="btn ghost"
                      onClick={restoreBackup}
                    >
                      {t("ear.restore")}
                    </button>
                  )}
                </div>
              </div>
            ) : (
              <div className="empty-state">{t("ear.emptyResult")}</div>
            )}
          </Panel>
        </div>
      </div>
    </div>
  );
}
