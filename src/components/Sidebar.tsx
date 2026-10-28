import type { CSSProperties } from "react";

import { isTauri } from "../lib/api";
import { formatPercent } from "../lib/format";
import { translate, type MessageKey } from "../lib/i18n";
import { useAppStore, useLanguage, type Page } from "../store/appStore";
import { Icon, type IconName } from "./Icons";

const NAV: { page: Page; icon: IconName }[] = [
  { page: "dashboard", icon: "dashboard" },
  { page: "equalizer", icon: "equalizer" },
  { page: "spatial", icon: "spatial" },
  { page: "dynamics", icon: "dynamics" },
  { page: "presets", icon: "presets" },
  { page: "ear", icon: "ear" },
  { page: "settings", icon: "settings" },
];

export function Sidebar() {
  const page = useAppStore((state) => state.page);
  const setPage = useAppStore((state) => state.setPage);
  const view = useAppStore((state) => state.view);
  const setVolume = useAppStore((state) => state.setVolume);
  const language = useLanguage();
  const t = (key: MessageKey) => translate(language, key);

  const running = view?.engine.running ?? false;
  const bypass = view?.params.bypass ?? false;
  const status = bypass ? t("status.bypassed") : running ? t("status.running") : t("status.stopped");
  const volume = view?.params.volume ?? 0;

  return (
    <aside className="sidebar">
      <div className="brand">
        <span className="brand-mark">
          <Icon name="logo" size={34} />
        </span>
        <span className="brand-text">
          <strong>Audio Pulse</strong>
          <em>{isTauri ? `v${view?.version ?? "1.0.0"}` : t("common.demo")}</em>
        </span>
      </div>

      <nav className="nav" aria-label={t("a11y.primaryNav")}>
        {NAV.map((item) => (
          <button
            key={item.page}
            type="button"
            className={`nav-item ${page === item.page ? "is-active" : ""}`}
            onClick={() => setPage(item.page)}
            aria-current={page === item.page ? "page" : undefined}
            title={t(`nav.${item.page}` as MessageKey)}
          >
            <Icon name={item.icon} size={20} />
            <span>{t(`nav.${item.page}` as MessageKey)}</span>
          </button>
        ))}
      </nav>

      <div className="sidebar-foot">
        <div className="mini-volume">
          <div className="mini-volume-head">
            <Icon name={volume <= 0.001 ? "mute" : "volume"} size={15} />
            <span>{formatPercent(volume * 100)}</span>
          </div>
          <input
            type="range"
            min={0}
            max={1}
            step={0.01}
            value={volume}
            aria-label={t("volume.master")}
            className="range range-cyan range-mini"
            style={{ "--fill": `${volume * 100}%` } as CSSProperties}
            onChange={(event) => setVolume(Number(event.target.value))}
          />
        </div>
        <div className={`engine-status ${running ? (bypass ? "is-bypassed" : "is-running") : ""}`}>
          <span className="status-dot" />
          <span>{status}</span>
        </div>
      </div>
    </aside>
  );
}