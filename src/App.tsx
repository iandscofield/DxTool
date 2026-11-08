import { useCallback, useEffect, useRef, useState } from "react";
import { Download, RefreshCw, ShieldAlert, Moon, Sun } from "lucide-react";
import { Sidebar } from "./components/Sidebar";
import { Overview } from "./pages/Overview";
import { Hardware } from "./pages/Hardware";
import { Placeholder } from "./pages/Placeholder";
import { Scans } from "./pages/Scans";
import { Health } from "./pages/Health";
import { Security } from "./pages/Security";
import { History } from "./pages/History";
import { Reports } from "./pages/Reports";
import { Fixes } from "./pages/Fixes";
import { Settings } from "./pages/Settings";
import { Virus } from "./pages/Virus";
import { useAvJob } from "./lib/useAvJob";
import { api, loadPref, savePref } from "./lib/api";
import type { UpdateInfo, Accent, AppInfo, AppSettings, CheckInfo, FixEvent, FixInfo, FixRequest, FixRun, FixState, Inventory, MachineRecord, Mode, Page, ScanEvent, ScanKind, ScanReport, ScanState, ThemePref } from "./lib/types";
import { defaultBranding } from "./lib/types";

function applyFixEvent(s: FixState, e: FixEvent): FixState {
  const steps = [...s.steps];
  const id = e.type === "stepFinished" ? e.result.id : e.id;
  let i = steps.findIndex((x) => x.id === id);
  if (i < 0) {
    steps.push({ id, title: e.type === "stepFinished" ? e.result.title : e.type === "stepStarted" ? e.title : id, state: "waiting", lines: [] });
    i = steps.length - 1;
  }
  const st = { ...steps[i] };
  if (e.type === "stepStarted") st.state = "running";
  else if (e.type === "output") st.lines = [...st.lines, e.line].slice(-500);
  else if (e.type === "progress") st.percent = e.percent;
  else {
    st.state = "done";
    st.result = e.result;
  }
  steps[i] = st;
  return { ...s, steps };
}

function applyScanEvent(s: ScanState, e: ScanEvent): ScanState {
  if (e.type === "started") return { ...s, order: e.checks, progress: Object.fromEntries(e.checks.map((id) => [id, { state: "waiting" as const }])) };
  if (e.type === "checkStarted") return { ...s, progress: { ...s.progress, [e.id]: { state: "running" } } };
  return { ...s, progress: { ...s.progress, [e.id]: { state: "done", status: e.status, summary: e.summary, elapsedMs: e.elapsedMs } } };
}

export default function App() {
  const [page, setPage] = useState<Page>("overview");
  const [mode, setMode] = useState<Mode>(loadPref("mode", "simple"));
  const [theme, setTheme] = useState<ThemePref>(loadPref("theme", "system"));
  const [accent, setAccent] = useState<Accent>(loadPref("accent", "blue"));
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
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [updateDismissed, setUpdateDismissed] = useState(false);
  const startupCheckDone = useRef(false);
  const [fixCatalog, setFixCatalog] = useState<FixInfo[]>([]);
  const [fixesUnlocked, setFixesUnlocked] = useState(false);
  const [fixState, setFixState] = useState<FixState | null>(null);
  const [fixRun, setFixRun] = useState<FixRun | null>(null);
  const [fixPreselect, setFixPreselect] = useState<string | null>(null);
  const av = useAvJob(useCallback(() => {}, []));

  const runFixes = useCallback(
    async (requests: FixRequest[], skipRestorePoint: boolean) => {
      if (fixState && !fixRun) return;
      const ordered = fixCatalog.filter((f) => requests.some((r) => r.id === f.id));
      setFixRun(null);
      setFixState({
        requests,
        startedAt: Date.now(),
        steps: [
          ...(skipRestorePoint ? [] : [{ id: "restore_point", title: "Create a restore point", state: "waiting" as const, lines: [] }]),
          ...ordered.map((f) => ({ id: f.id, title: f.title, state: "waiting" as const, lines: [] })),
        ],
      });
      try {
        const run = await api.runFixes(requests, skipRestorePoint, (e) => setFixState((s) => (s ? applyFixEvent(s, e) : s)));
        setFixRun(run);
      } catch (e) {
        setFixState(null);
        setScanError(`Fixes could not run: ${e}`);
      }
    },
    [fixState, fixRun, fixCatalog],
  );
  const openFix = (fixId: string) => {
    setFixPreselect(fixId);
    setPage("fixes");
  };

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

  // Optional start-up check for a new version (Settings → Updates). Silent if offline.
  useEffect(() => {
    if (!settingsLoaded || !settings.checkUpdates || startupCheckDone.current) return;
    startupCheckDone.current = true;
    api
      .updateCheck()
      .then((u) => u.newer && setUpdate(u))
      .catch(() => {});
  }, [settingsLoaded, settings.checkUpdates]);

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
    savePref("accent", accent);
    if (accent === "blue") document.documentElement.removeAttribute("data-accent");
    else document.documentElement.setAttribute("data-accent", accent);
  }, [accent]);
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
    api.fixCatalog().then(setFixCatalog).catch(() => setFixCatalog([]));
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
        accent={accent}
        setAccent={setAccent}
        mode={mode}
        setMode={setMode}
        info={info}
        onElevate={elevate}
        branding={settings.branding}
        onBranding={(branding) => setSettings((s) => ({ ...s, branding }))}
        checkUpdates={!!settings.checkUpdates}
        onCheckUpdates={(checkUpdates) => setSettings((s) => ({ ...s, checkUpdates }))}
        update={update}
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
  } else if (page === "fixes") {
    content = (
      <Fixes
        info={info}
        report={report}
        machineId={machineId}
        unlocked={fixesUnlocked}
        onUnlock={() => setFixesUnlocked(true)}
        onLock={() => setFixesUnlocked(false)}
        fixState={fixState}
        fixRun={fixRun}
        onRunFixes={runFixes}
        onClearRun={() => {
          setFixState(null);
          setFixRun(null);
        }}
        preselect={fixPreselect}
        onRunScan={runScan}
      />
    );
  } else if (page === "reports") {
    content = (
      <Reports report={report} machines={machines} settings={settings} onPickScan={setReport} onRun={runScan} onOpenSettings={() => setPage("settings")} />
    );
  } else if (page === "scans") {
    content = <Scans catalog={catalog} scan={scan} report={report} info={info} onRun={runScan} onViewResults={() => setPage("health")} />;
  } else if (page === "health") {
    content = <Health report={report} scan={scan} catalog={catalog} mode={mode} onRun={runScan} currentMachineId={machineId} onReport={() => setPage("reports")} onFix={openFix} />;
  } else if (page === "security") {
    content = <Security report={report} scan={scan} catalog={catalog} mode={mode} onRun={runScan} onFix={openFix} onOpenVirus={() => setPage("virus")} />;
  } else if (page === "virus") {
    content = <Virus av={av} machineId={machineId} mode={mode} />;
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
      <Sidebar page={page} onNavigate={setPage} version={info?.version ?? "0.1.0"} scanning={!!scan} report={report} fixing={!!fixState && !fixRun} virusRunning={av.running} />
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
        {update?.newer && !updateDismissed && page !== "settings" && (
          <div className="banner update">
            <Download size={16} />
            <span>DxTool {update.latest} is available.</span>
            <button className="btn small" onClick={() => setPage("settings")}>
              See what's new
            </button>
            <button className="btn small ghost" onClick={() => setUpdateDismissed(true)}>
              Later
            </button>
          </div>
        )}
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
