import { useCallback, useEffect, useState } from "react";
import { RefreshCw, ShieldAlert, Moon, Sun } from "lucide-react";
import { Sidebar } from "./components/Sidebar";
import { Overview } from "./pages/Overview";
import { Hardware } from "./pages/Hardware";
import { Placeholder } from "./pages/Placeholder";
import { Settings } from "./pages/Settings";
import { api, loadPref, savePref } from "./lib/api";
import type { AppInfo, Inventory, Mode, Page, ThemePref } from "./lib/types";

export default function App() {
  const [page, setPage] = useState<Page>("overview");
  const [mode, setMode] = useState<Mode>(loadPref("mode", "simple"));
  const [theme, setTheme] = useState<ThemePref>(loadPref("theme", "system"));
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [inventory, setInventory] = useState<Inventory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [hwCategory, setHwCategory] = useState<string | undefined>();

  useEffect(() => savePref("mode", mode), [mode]);
  useEffect(() => {
    savePref("theme", theme);
    if (theme === "system") document.documentElement.removeAttribute("data-theme");
    else document.documentElement.setAttribute("data-theme", theme);
  }, [theme]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setInventory(await api.inventory());
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    api.appInfo().then(setInfo).catch((e) => setError(String(e)));
    refresh();
  }, [refresh]);

  const elevate = async () => {
    try {
      await api.relaunchAsAdmin();
    } catch (e) {
      setError(String(e));
    }
  };

  const openCategory = (id: string) => {
    setHwCategory(id);
    setPage("hardware");
  };

  const isDark =
    theme === "dark" || (theme === "system" && window.matchMedia?.("(prefers-color-scheme: dark)").matches);

  let content: React.ReactNode;
  if (page === "settings") {
    content = <Settings theme={theme} setTheme={setTheme} mode={mode} setMode={setMode} info={info} onElevate={elevate} />;
  } else if (page !== "overview" && page !== "hardware") {
    content = <Placeholder page={page} />;
  } else if (!inventory) {
    content = error ? (
      <div className="page">
        <div className="banner error">Could not read this PC's hardware: {error}</div>
        <button className="btn" onClick={refresh}>
          Try again
        </button>
      </div>
    ) : (
      <Loading />
    );
  } else if (page === "overview") {
    content = <Overview inventory={inventory} onOpenCategory={openCategory} onRefresh={refresh} refreshing={loading} />;
  } else {
    content = <Hardware inventory={inventory} mode={mode} initialCategory={hwCategory} />;
  }

  return (
    <div className="app">
      <Sidebar page={page} onNavigate={setPage} version={info?.version ?? "0.1.0"} />
      <div className="main">
        <header className="topbar">
          <div className="topbar-title">{info?.computerName ?? "This PC"}</div>
          <div className="topbar-fill" />
          <div className="segmented" title="Simple shows the key facts. Advanced shows everything Windows reports.">
            <button className={mode === "simple" ? "on" : ""} onClick={() => setMode("simple")}>
              Simple
            </button>
            <button className={mode === "advanced" ? "on" : ""} onClick={() => setMode("advanced")}>
              Advanced
            </button>
          </div>
          <button className="icon-btn ghost" onClick={refresh} disabled={loading} title="Re-read hardware">
            <RefreshCw size={16} className={loading ? "spin" : ""} />
          </button>
          <button className="icon-btn ghost" onClick={() => setTheme(isDark ? "light" : "dark")} title="Toggle dark / light">
            {isDark ? <Sun size={16} /> : <Moon size={16} />}
          </button>
        </header>
        {info && !info.isAdmin && (
          <div className="banner admin">
            <ShieldAlert size={16} />
            <span>
              DxTool is running without administrator rights, so some details (TPM, drive health, battery data) may be missing.
            </span>
            <button className="btn small" onClick={elevate}>
              Restart as administrator
            </button>
          </div>
        )}
        <div className="content">{content}</div>
      </div>
    </div>
  );
}

function Loading() {
  return (
    <div className="loading">
      <img src="/logo.svg" alt="" className="pulse" />
      <div className="loading-title">Reading this PC's hardware…</div>
      <div className="muted small">Processor, memory, drives, devices and where everything is connected</div>
      <div className="loading-bar">
        <span />
      </div>
    </div>
  );
}
