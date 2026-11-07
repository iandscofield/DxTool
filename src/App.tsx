import { useCallback, useEffect, useState } from "react";
import { RefreshCw, ShieldAlert, Moon, Sun } from "lucide-react";
import { Sidebar } from "./components/Sidebar";
import { Overview } from "./pages/Overview";
import { Hardware } from "./pages/Hardware";
import { Placeholder } from "./pages/Placeholder";
import { Scans } from "./pages/Scans";
import { Health } from "./pages/Health";
import { Security } from "./pages/Security";
import { History } from "./pages/History";
import { Reports } from "./pages/Reports";
import { Settings } from "./pages/Settings";
import { api, loadPref, savePref } from "./lib/api";
import type { AppInfo, AppSettings, CheckInfo, Inventory, MachineRecord, Mode, Page, ScanEvent, ScanKind, ScanReport, ScanState, ThemePref } from "./lib/types";
import { defaultBranding } from "./lib/types";

function applyScanEvent(s: ScanState, e: ScanEvent): ScanState {
  if (e.type === "started") return { ...s, order: e.checks, progress: Object.fromEntries(e.checks.map((id) => [id, { state: "waiting" as const }])) };
  if (e.type === "checkStarted") return { ...s, progress: { ...s.progress, [e.id]: { state: "running" } } };
  return { ...s, progress: { ...s.progress, [e.id]: { state: "done", status: e.status, summary: e.summary, elapsedMs: e.elapsedMs } } };
}

export default function App() {
  const [page, setPage] = useState<Page>("overview");
  const [mode, setMode] = useState<Mode>(loadPref("mode", "simple"));
  const [theme, setTheme] = useState<ThemePref>(loadPref("theme", "system"));
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [inventory, setInventory] = useState<Inventory | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [hwCategory, setHwCategory] = useState<string | undefined>();
  const [catalog, setCatalog] = useState<CheckInfo[]>([]);
  const [scan, setScan] = useState<ScanState | null>(null);
  const [report, setReport] = useState<ScanReport | null>(null);
  const [scanError, setScanError] = useState<string | null>(null);
  const [machines, setMachines] = useState<MachineRecord[]>([]);
  const [machineId, setMachineId] = useState<string | null>(null);
  const [settings, setSettings] = useState<AppSettings>({ branding: defaultBranding });
  const [settingsLoaded, setSettingsLoaded] = useState(false);

  const refreshHistory = useCallback(async () => {
    try {
      const list = await api.listMachines();
      setMachines(list);
      return list;
    } catch {
      return [];
    }
  }, []);

  // On start: load settings, find this PC, and reopen its most recent saved scan.
  useEffect(() => {
    api.loadSettings().then((s) => {
      setSettings(s);
      setSettingsLoaded(true);
    });
    (async () => {
      const [list, me] = await Promise.all([refreshHistory(), api.currentMachine().catch(() => null)]);
      if (!me) return;
      setMachineId(me.id);
      const latest = list.find((m) => m.info.id === me.id)?.scans[0];
      if (latest) {
        const r = await api.loadScan(me.id, latest.id).catch(() => null);
        if (r) setReport((cur) => cur ?? r);
      }
    })();
  }, [refreshHistory]);

  // Save settings shortly after they change.
  useEffect(() => {
    if (!settingsLoaded) return;
    const t = setTimeout(() => api.saveSettings(settings).catch(() => {}), 400);
    return () => clearTimeout(t);
  }, [settings, settingsLoaded]);

  const runScan = useCallback(
    async (kind: ScanKind, checks: string[] = []) => {
      if (scan) return;
      setScanError(null);
      setScan({ kind, order: [], progress: {}, startedAt: Date.now() });
      setPage((p) => (p === "overview" || p === "scans" ? "health" : p));
      try {
        const r = await api.runScan(kind, checks, (e) => setScan((s) => (s ? applyScanEvent(s, e) : s)));
        setReport(r);
        setMachineId(r.machine.id);
        refreshHistory();
      } catch (e) {
        setScanError(String(e));
      } finally {
        setScan(null);
      }
    },
    [scan, refreshHistory],
  );

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
    api.catalog().then(setCatalog).catch(() => setCatalog([]));
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
    content = (
      <Settings
        theme={theme}
        setTheme={setTheme}
        mode={mode}
        setMode={setMode}
        info={info}
        onElevate={elevate}
        branding={settings.branding}
        onBranding={(branding) => setSettings((s) => ({ ...s, branding }))}
      />
    );
  } else if (page === "history") {
    content = (
      <History
        machines={machines}
        currentMachineId={machineId}
        onRefresh={refreshHistory}
        onOpen={(r) => {
          setReport(r);
          setPage("health");
        }}
        onReport={(r) => {
          setReport(r);
          setPage("reports");
        }}
      />
    );
  } else if (page === "reports") {
    content = (
      <Reports report={report} machines={machines} settings={settings} onPickScan={setReport} onRun={runScan} onOpenSettings={() => setPage("settings")} />
    );
  } else if (page === "scans") {
    content = <Scans catalog={catalog} scan={scan} report={report} info={info} onRun={runScan} onViewResults={() => setPage("health")} />;
  } else if (page === "health") {
    content = <Health report={report} scan={scan} catalog={catalog} mode={mode} onRun={runScan} currentMachineId={machineId} onReport={() => setPage("reports")} />;
  } else if (page === "security") {
    content = <Security report={report} scan={scan} catalog={catalog} mode={mode} onRun={runScan} />;
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
    content = (
      <Overview
        inventory={inventory}
        onOpenCategory={openCategory}
        onRefresh={refresh}
        refreshing={loading}
        report={report}
        scan={scan}
        onRunScan={runScan}
        onOpenHealth={() => setPage("health")}
      />
    );
  } else {
    content = <Hardware inventory={inventory} mode={mode} initialCategory={hwCategory} />;
  }

  return (
    <div className="app">
      <Sidebar page={page} onNavigate={setPage} version={info?.version ?? "0.1.0"} scanning={!!scan} report={report} />
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
        {scanError && (
          <div className="banner error">
            The scan could not finish: {scanError}
            <button className="btn small ghost" onClick={() => setScanError(null)}>
              Dismiss
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
