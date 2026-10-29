import { useEffect, type ComponentType } from "react";

import { Sidebar } from "./components/Sidebar";
import { ToastHost } from "./components/ToastHost";
import { TopBar } from "./components/TopBar";
import { translate, type MessageKey } from "./lib/i18n";
import { Dashboard } from "./pages/Dashboard";
import { Dynamics } from "./pages/Dynamics";
import { EarProfile } from "./pages/EarProfile";
import { Equalizer } from "./pages/Equalizer";
import { Hearing } from "./pages/Hearing";
import { Presets } from "./pages/Presets";
import { Settings } from "./pages/Settings";
import { Spatial } from "./pages/Spatial";
import { useAppStore, useLanguage, type Page } from "./store/appStore";
import "./styles.css";

const PAGES: Record<Page, ComponentType> = {
  dashboard: Dashboard,
  equalizer: Equalizer,
  spatial: Spatial,
  dynamics: Dynamics,
  hearing: Hearing,
  presets: Presets,
  ear: EarProfile,
  settings: Settings,
};

export default function App() {
  const bootstrap = useAppStore((state) => state.bootstrap);
  const view = useAppStore((state) => state.view);
  const page = useAppStore((state) => state.page);
  const language = useLanguage();

  useEffect(() => {
    void bootstrap();
  }, [bootstrap]);

  useEffect(() => {
    const root = document.documentElement;
    const theme = view?.settings.theme ?? "dark";
    root.dataset.theme = theme;
    root.lang = language;
    document.title = `${translate(language, "app.name")} · ${translate(language, `nav.${page}` as MessageKey)}`;
    const meta = document.querySelector('meta[name="theme-color"]');
    if (meta) {
      meta.setAttribute("content", theme === "dark" ? "#05070d" : "#eef2f9");
    }
  }, [view?.settings.theme, language, page]);

  const PageComponent = PAGES[page];

  return (
    <div className="app">
      <Sidebar />
      <div className="app-main">
        <TopBar />
        <main className="page-scroll">
          {view ? (
            <PageComponent key={page} />
          ) : (
            <div className="page-loading">{translate(language, "common.loading")}</div>
          )}
        </main>
      </div>
      <ToastHost />
    </div>
  );
}