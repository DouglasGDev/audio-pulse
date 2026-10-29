import { useState } from "react";

import { Icon, type IconName } from "../components/Icons";
import { Modal } from "../components/Modal";
import { isTauri } from "../lib/api";
import { presetDescription, presetTitle, translate, type MessageKey } from "../lib/i18n";
import type { Preset, PresetCategory } from "../lib/types";
import { useAppStore, useLanguage } from "../store/appStore";

const CATEGORY_ICONS: Record<PresetCategory, IconName> = {
  music: "wave",
  cinema: "speaker",
  gaming: "spatial",
  podcast: "headphone",
  vocal: "wave",
  flat: "equalizer",
  bass: "speaker",
  night: "moon",
  protection: "shield",
  headphone: "headphone",
  custom: "sparkle",
};

export function Presets() {
  const view = useAppStore((state) => state.view);
  const applyPreset = useAppStore((state) => state.applyPreset);
  const savePreset = useAppStore((state) => state.savePreset);
  const deletePreset = useAppStore((state) => state.deletePreset);
  const exportPreset = useAppStore((state) => state.exportPreset);
  const importPreset = useAppStore((state) => state.importPreset);
  const importAutoEq = useAppStore((state) => state.importAutoEq);
  const language = useLanguage();
  const t = (key: MessageKey, vars?: Record<string, string | number>) =>
    translate(language, key, vars);

  const [saving, setSaving] = useState(false);
  const [name, setName] = useState("");
  const [toDelete, setToDelete] = useState<Preset | null>(null);

  if (!view) return <div className="page-loading">{t("common.loading")}</div>;

  const modes = view.presets.filter(
    (preset) => preset.builtIn && preset.category !== "headphone",
  );
  const headphones = view.presets.filter((preset) => preset.category === "headphone");
  const custom = view.presets.filter((preset) => !preset.builtIn);

  const submitSave = async () => {
    if (!name.trim()) return;
    const saved = await savePreset(name.trim());
    if (saved) {
      setName("");
      setSaving(false);
    }
  };

  return (
    <div className="page presets">
      <div className="page-toolbar">
        <div className="toolbar-group">
          <button type="button" className="btn primary" onClick={() => setSaving(true)}>
            <Icon name="plus" size={16} />
            {t("presets.saveCurrent")}
          </button>
          <button type="button" className="btn ghost" onClick={() => importPreset()}>
            <Icon name="upload" size={16} />
            {t("presets.import")}
          </button>
        </div>
        <button
          type="button"
          className="btn ghost"
          onClick={() => importAutoEq()}
          disabled={!isTauri}
        >
          <Icon name="sparkle" size={16} />
          {t("presets.importAutoEq")}
        </button>
      </div>

      <section className="preset-section">
        <header className="section-head">
          <h2>{t("presets.modes")}</h2>
          <span className="badge">{modes.length}</span>
        </header>
        <div className="mode-preset-grid">
          {modes.map((preset) => {
            const active = view.activePresetId === preset.id;
            return (
              <button
                key={preset.id}
                type="button"
                className={`preset-card mode-preset ${active ? "is-active" : ""}`}
                onClick={() => applyPreset(preset.id)}
              >
                <span className="preset-icon">
                  <Icon name={CATEGORY_ICONS[preset.category]} size={22} />
                </span>
                <span className="preset-name">{presetTitle(language, preset)}</span>
                <span className="preset-desc">{presetDescription(language, preset)}</span>
                <span className="preset-foot">
                  <span className="tag">{categoryLabelText(t, preset)}</span>
                  {active && (
                    <span className="active-tag">
                      <Icon name="check" size={12} />
                      {t("common.active")}
                    </span>
                  )}
                </span>
              </button>
            );
          })}
        </div>
      </section>

      <section className="preset-section">
        <header className="section-head">
          <h2>{t("presets.headphones")}</h2>
          <span className="badge">{headphones.length}</span>
        </header>
        <div className="headphone-grid">
          {headphones.map((preset) => {
            const active = view.activePresetId === preset.id;
            return (
              <button
                key={preset.id}
                type="button"
                className={`preset-card headphone-card ${active ? "is-active" : ""}`}
                onClick={() => applyPreset(preset.id)}
              >
                <span className="brand-mark small">
                  <Icon name="headphone" size={20} />
                </span>
                <span className="preset-body">
                  <span className="preset-brand">{preset.brand}</span>
                  <span className="preset-name">{preset.model}</span>
                  <span className="preset-desc">
                    {t("presets.filtersCount", { count: preset.params.autoEq.filters.length })}
                  </span>
                </span>
                {active && (
                  <span className="active-tag">
                    <Icon name="check" size={12} />
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </section>

      <section className="preset-section">
        <header className="section-head">
          <h2>{t("presets.custom")}</h2>
          <span className="badge">{custom.length}</span>
        </header>
        {custom.length === 0 ? (
          <div className="empty-state">{t("presets.empty")}</div>
        ) : (
          <div className="custom-list">
            {custom.map((preset) => {
              const active = view.activePresetId === preset.id;
              return (
                <div key={preset.id} className={`custom-item ${active ? "is-active" : ""}`}>
                  <span className="preset-icon small">
                    <Icon name="sparkle" size={18} />
                  </span>
                  <button
                    type="button"
                    className="custom-main"
                    onClick={() => applyPreset(preset.id)}
                  >
                    <span className="preset-name">{preset.name}</span>
                    <span className="preset-desc">{presetDescription(language, preset)}</span>
                  </button>
                  {active && (
                    <span className="active-tag">
                      <Icon name="check" size={12} />
                      {t("common.active")}
                    </span>
                  )}
                  <div className="custom-actions">
                    <button
                      type="button"
                      className="icon-btn"
                      title={t("common.export")}
                      onClick={() => exportPreset(preset.id)}
                    >
                      <Icon name="download" size={16} />
                    </button>
                    <button
                      type="button"
                      className="icon-btn danger"
                      title={t("common.delete")}
                      onClick={() => setToDelete(preset)}
                    >
                      <Icon name="trash" size={16} />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </section>

      <Modal
        open={saving}
        title={t("presets.newTitle")}
        onClose={() => setSaving(false)}
        footer={
          <>
            <button type="button" className="btn ghost" onClick={() => setSaving(false)}>
              {t("common.cancel")}
            </button>
            <button
              type="button"
              className="btn primary"
              disabled={!name.trim()}
              onClick={submitSave}
            >
              {t("common.save")}
            </button>
          </>
        }
      >
        <label className="field">
          <span>{t("presets.namePlaceholder")}</span>
          <input
            className="text-input"
            value={name}
            autoFocus
            maxLength={60}
            placeholder={t("presets.namePlaceholder")}
            onChange={(event) => setName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") submitSave();
            }}
          />
        </label>
      </Modal>

      <Modal
        open={toDelete !== null}
        title={t("presets.deleteTitle")}
        onClose={() => setToDelete(null)}
        footer={
          <>
            <button type="button" className="btn ghost" onClick={() => setToDelete(null)}>
              {t("common.cancel")}
            </button>
            <button
              type="button"
              className="btn danger"
              onClick={async () => {
                if (toDelete) await deletePreset(toDelete.id);
                setToDelete(null);
              }}
            >
              {t("common.delete")}
            </button>
          </>
        }
      >
        <p className="modal-text">
          {toDelete?.name} — {t("presets.deleteBody")}
        </p>
      </Modal>
    </div>
  );
}

function categoryLabelText(
  t: (key: MessageKey) => string,
  preset: Preset,
): string {
  return t(`category.${preset.category}` as MessageKey);
}