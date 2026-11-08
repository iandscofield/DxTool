import { useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  ChevronRight,
  Circle,
  CloudDownload,
  History as HistoryIcon,
  Loader2,
  Lock,
  MinusCircle,
  Play,
  Power,
  RotateCcw,
  ShieldAlert,
  ShieldCheck,
  Undo2,
  Wrench,
  XCircle,
} from "lucide-react";
import type { AppInfo, DriverUpdate, FixInfo, FixRequest, FixRun, FixState, FixStepResult, ScanKind, ScanReport, StepStatus } from "../lib/types";
import { api } from "../lib/api";
import { collectFindings } from "../components/FindingList";
import { StatusBadge } from "../components/Status";

const riskText = { low: "Low risk", medium: "Medium risk", high: "High risk" };
const fmtSize = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : b >= 1e6 ? `${(b / 1e6).toFixed(0)} MB` : `${Math.max(1, Math.round(b / 1e3))} KB`);
const fmtWhen = (s: string) => new Date(s).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });

function StepIcon({ status, running }: { status?: StepStatus; running?: boolean }) {
  if (running) return <Loader2 size={17} className="spin step-run" />;
  if (!status) return <Circle size={17} className="muted" />;
  if (status === "ok") return <CheckCircle2 size={17} className="step-ok" />;
  if (status === "warning") return <AlertTriangle size={17} className="step-warn" />;
  if (status === "skipped") return <MinusCircle size={17} className="muted" />;
  return <XCircle size={17} className="step-fail" />;
}

// ───────── Gate: scan first, then acknowledge the risks ─────────

function Gate({ hasScan, onUnlock, onRun }: { hasScan: boolean; onUnlock: () => void; onRun: (k: ScanKind) => void }) {
  const [a, setA] = useState(false);
  const [b, setB] = useState(false);
  return (
    <div className="page">
      <div className="gate card">
        <div className="gate-icon">
          <Lock size={26} />
        </div>
        <h1>Fixes change your PC</h1>
        <p className="muted">Everything else in DxTool only looks. The repairs here change system files and settings. They're the same tools a technician would use, and they're safe for most PCs, but please read this first.</p>
        <ul className="gate-list">
          <li>
            <ShieldCheck size={16} /> <span><b>A restore point is created first.</b> If something goes wrong, Windows System Restore can put system files and settings back. It doesn't affect your documents and photos.</span>
          </li>
          <li>
            <AlertTriangle size={16} /> <span><b>Some repairs take a long time</b> (up to 30 minutes) and some need a restart. Don't turn the PC off while they run.</span>
          </li>
          <li>
            <AlertTriangle size={16} /> <span><b>Save your work and close other programs</b> before you start.</span>
          </li>
          <li>
            <AlertTriangle size={16} /> <span><b>If the drive is encrypted with BitLocker</b>, make sure you know the recovery key before running drive or driver repairs.</span>
          </li>
        </ul>
        {!hasScan ? (
          <div className="gate-scan">
            <div>
              <b>Run a scan first.</b>
              <div className="muted small">Fixes unlock after this PC has been scanned, so you know what actually needs fixing.</div>
            </div>
            <button className="btn" onClick={() => onRun("fast")}>
              <Play size={15} /> Run Fast scan
            </button>
          </div>
        ) : (
          <>
            <label className="ack">
              <input type="checkbox" checked={a} onChange={(e) => setA(e.target.checked)} />
              <span>I understand these repairs change system files and settings, and that a restore point is created first so they can be undone.</span>
            </label>
            <label className="ack">
              <input type="checkbox" checked={b} onChange={(e) => setB(e.target.checked)} />
              <span>I've saved my work and backed up important files, or I accept the risk.</span>
            </label>
            <button className="btn" disabled={!a || !b} onClick={onUnlock}>
              <Wrench size={15} /> Unlock Fixes
            </button>
          </>
        )}
      </div>
    </div>
  );
}

// ───────── Driver updates (online, on click) ─────────

function DriverPanel({ picked, setPicked }: { picked: string[]; setPicked: (ids: string[]) => void }) {
  const [list, setList] = useState<DriverUpdate[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [line, setLine] = useState("");
  const [error, setError] = useState<string | null>(null);
  const check = async () => {
    setBusy(true);
    setError(null);
    try {
      const l = await api.checkDriverUpdates(setLine);
      setList(l);
      setPicked(l.filter((d) => !d.warning).map((d) => d.updateId));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const older = (d: DriverUpdate) => d.warning.includes("older than");
  const toggle = (id: string) => setPicked(picked.includes(id) ? picked.filter((x) => x !== id) : [...picked, id]);
  return (
    <div className="driver-panel">
      <div className="row-gap">
        <button className="btn small" onClick={check} disabled={busy}>
          {busy ? <Loader2 size={13} className="spin" /> : <CloudDownload size={13} />} {list ? "Check again" : "Check for driver updates"}
        </button>
        <span className="muted small">{busy ? line || "Asking Windows Update…" : "Asks Windows Update over the internet. Only happens when you click."}</span>
      </div>
      {error && <div className="banner error">{error}</div>}
      {list && list.length === 0 && <div className="all-good">Windows Update has no driver updates for this PC.</div>}
      {list && list.length > 0 && (
        <div className="table-scroll">
          <table className="simple-table drivers">
            <thead>
              <tr>
                <th />
                <th>Device</th>
                <th>Installed now</th>
                <th>Available</th>
                <th>Size</th>
              </tr>
            </thead>
            <tbody>
              {list.map((d) => (
                <tr key={d.updateId} className={picked.includes(d.updateId) ? "picked" : ""}>
                  <td>
                    <input type="checkbox" checked={picked.includes(d.updateId)} disabled={older(d)} onChange={() => toggle(d.updateId)} aria-label={`Install ${d.title}`} />
                  </td>
                  <td className="wrap">
                    <div>{d.deviceName}</div>
                    <div className="muted small">{d.title}</div>
                    {d.warning && (
                      <div className={`driver-warn ${older(d) ? "bad" : ""}`}>
                        <AlertTriangle size={12} /> {d.warning}
                      </div>
                    )}
                  </td>
                  <td>
                    {d.installedVersion || <span className="muted">—</span>}
                    <div className="muted small">{d.installedDate}</div>
                  </td>
                  <td>
                    {d.newVersion || <span className="muted">—</span>}
                    <div className="muted small">{d.newDate}</div>
                  </td>
                  <td className="muted">{fmtSize(d.sizeBytes)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

// ───────── Running and results ─────────

function RunView({ state, run, onDone, onRetry }: { state: FixState | null; run: FixRun | null; onDone: () => void; onRetry: (skip: boolean, enableProtection: boolean) => void }) {
  const [open, setOpen] = useState<string | null>(null);
  const [restart, setRestart] = useState<"idle" | "confirm" | "scheduled">("idle");
  const [skipConfirm, setSkipConfirm] = useState(false);
  const running = !!state && !run;
  const steps = state?.steps ?? [];
  const active = steps.find((s) => s.state === "running");
  const rp = run?.restorePoint;
  const rpFailed = rp?.status === "failed";
  const protectionOff = rpFailed && rp!.summary.includes("System Protection");

  return (
    <section className="card fix-run">
      <div className="card-head">
        <h2>
          {running ? <Loader2 size={17} className="spin" /> : <Wrench size={17} />} {running ? "Running fixes…" : "Fixes finished"}
        </h2>
        {running && <span className="muted small">Keep the PC on. You can use other parts of DxTool meanwhile.</span>}
      </div>
      <div className="fix-steps">
        {steps.map((s) => {
          const isOpen = open === s.id || (running && active?.id === s.id);
          return (
            <div key={s.id} className={`fix-step ${s.state}`}>
              <button className="fix-step-head" onClick={() => setOpen(open === s.id ? null : s.id)}>
                <StepIcon status={s.result?.status} running={s.state === "running"} />
                <span className="fix-step-title">{s.title}</span>
                <span className="muted small fix-step-sum">{s.result?.summary ?? (s.state === "running" ? (s.percent != null ? `${s.percent.toFixed(0)}%` : "Working…") : "Waiting")}</span>
                <ChevronRight size={15} className={`chev ${isOpen ? "open" : ""}`} />
              </button>
              {s.state === "running" && s.percent != null && (
                <div className="progress-bar slim">
                  <span style={{ width: `${s.percent}%` }} />
                </div>
              )}
              {isOpen && (
                <div className="fix-step-body">
                  {s.result?.details.map((d) => (
                    <div key={d} className="small">
                      • {d}
                    </div>
                  ))}
                  {(s.lines.length > 0 || (s.result?.outputTail.length ?? 0) > 0) && <pre className="console">{(s.lines.length ? s.lines : s.result!.outputTail).slice(-200).join("\n")}</pre>}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {run && rpFailed && (
        <div className="rp-failed">
          <ShieldAlert size={18} />
          <div>
            <b>No fixes were run because the restore point couldn't be created.</b>
            <div className="small">{rp!.summary}</div>
            <div className="row-gap">
              {protectionOff && (
                <button className="btn small" onClick={() => onRetry(false, true)}>
                  <ShieldCheck size={13} /> Turn on System Protection and try again
                </button>
              )}
              {!skipConfirm ? (
                <button className="btn small ghost" onClick={() => setSkipConfirm(true)}>
                  Run without a restore point…
                </button>
              ) : (
                <span className="skip-confirm">
                  <span className="small">Without a restore point these changes can't be undone with System Restore. Continue?</span>
                  <button className="btn small danger" onClick={() => onRetry(true, false)}>
                    Yes, run without one
                  </button>
                  <button className="btn small ghost" onClick={() => setSkipConfirm(false)}>
                    Cancel
                  </button>
                </span>
              )}
            </div>
          </div>
        </div>
      )}

      {run && !rpFailed && (
        <div className="fix-done">
          {run.needsRestart && (
            <div className="restart-box">
              <Power size={18} />
              <div>
                <b>Restart the PC to finish.</b>
                <div className="small muted">Some repairs only take effect after a restart.</div>
              </div>
              {restart === "idle" && (
                <button className="btn small" onClick={() => setRestart("confirm")}>
                  Restart now
                </button>
              )}
              {restart === "confirm" && (
                <span className="row-gap">
                  <span className="small">Save your work first. Restart in 30 seconds?</span>
                  <button
                    className="btn small"
                    onClick={async () => {
                      await api.restartPc();
                      setRestart("scheduled");
                    }}
                  >
                    Restart
                  </button>
                  <button className="btn small ghost" onClick={() => setRestart("idle")}>
                    Not now
                  </button>
                </span>
              )}
              {restart === "scheduled" && (
                <span className="row-gap">
                  <span className="small">Restarting in 30 seconds…</span>
                  <button
                    className="btn small ghost"
                    onClick={async () => {
                      await api.cancelRestart();
                      setRestart("idle");
                    }}
                  >
                    Cancel restart
                  </button>
                </span>
              )}
            </div>
          )}
          <div className="row-gap">
            <button className="btn ghost" onClick={onDone}>
              Back to fixes
            </button>
            <span className="muted small">Tip: run a scan again afterwards to check the problems are gone.</span>
          </div>
        </div>
      )}
    </section>
  );
}

function FixLog({ runs }: { runs: FixRun[] }) {
  const [open, setOpen] = useState<string | null>(null);
  if (runs.length === 0) return null;
  const all = (r: FixRun): FixStepResult[] => [...(r.restorePoint ? [r.restorePoint] : []), ...r.steps];
  return (
    <section className="card">
      <div className="card-head">
        <h2>
          <HistoryIcon size={17} /> Fix log for this PC
        </h2>
        <span className="muted small">Every fix DxTool has run here, newest first</span>
      </div>
      <div className="fix-log">
        {runs.map((r) => (
          <div key={r.id} className="fix-log-run">
            <button className="fix-log-head" onClick={() => setOpen(open === r.id ? null : r.id)}>
              <span>{fmtWhen(r.startedAt)}</span>
              <span className="muted small">{r.steps.map((s) => s.title).join(", ") || "Restore point only"}</span>
              <span className="fix-log-dots">
                {all(r).map((s) => (
                  <StepIcon key={s.id} status={s.status} />
                ))}
              </span>
              <ChevronRight size={15} className={`chev ${open === r.id ? "open" : ""}`} />
            </button>
            {open === r.id && (
              <div className="fix-log-body">
                {r.restorePointSkipped && <div className="small step-warn">Run without a restore point (chosen by the user).</div>}
                {all(r).map((s) => (
                  <div key={s.id} className="fix-log-step">
                    <StepIcon status={s.status} />
                    <div>
                      <div>{s.title}</div>
                      <div className="muted small">{s.summary}</div>
                      {s.details.map((d) => (
                        <div key={d} className="muted small">
                          • {d}
                        </div>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </section>
  );
}

// ───────── Page ─────────

export function Fixes({
  info,
  report,
  machineId,
  unlocked,
  onUnlock,
  onLock,
  fixState,
  fixRun,
  onRunFixes,
  onClearRun,
  preselect,
  onRunScan,
}: {
  info: AppInfo | null;
  report: ScanReport | null;
  machineId: string | null;
  unlocked: boolean;
  onUnlock: () => void;
  onLock: () => void;
  fixState: FixState | null;
  fixRun: FixRun | null;
  onRunFixes: (requests: FixRequest[], skipRestorePoint: boolean) => void;
  onClearRun: () => void;
  preselect: string | null;
  onRunScan: (k: ScanKind) => void;
}) {
  const [catalog, setCatalog] = useState<FixInfo[]>([]);
  const [picked, setPicked] = useState<string[]>([]);
  const [tempOpts, setTempOpts] = useState({ userTemp: true, windowsTemp: true, errorReports: true, recycleBin: false });
  const [drives, setDrives] = useState<[string, string, string][]>([]);
  const [drive, setDrive] = useState("C:");
  const [driverIds, setDriverIds] = useState<string[]>([]);
  const [confirming, setConfirming] = useState(false);
  const [runs, setRuns] = useState<FixRun[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const hasScan = !!report && (!machineId || report.machine?.id === machineId);
  useEffect(() => {
    api.fixCatalog().then(setCatalog).catch((e) => setError(String(e)));
    api.ntfsDrives().then((d) => {
      setDrives(d);
      if (d[0]) setDrive(d[0][0]);
    });
  }, []);
  useEffect(() => {
    if (machineId) api.listFixRuns(machineId).then(setRuns).catch(() => {});
  }, [machineId, fixRun]);
  useEffect(() => {
    if (preselect) setPicked((p) => (p.includes(preselect) ? p : [...p, preselect]));
  }, [preselect]);

  // Fixes suggested by the latest scan's findings.
  const suggested = useMemo(() => {
    const m = new Map<string, string[]>();
    if (report) for (const { finding } of collectFindings(report.results)) if (finding.fix && finding.severity !== "ok") m.set(finding.fix, [...(m.get(finding.fix) ?? []), finding.title]);
    return m;
  }, [report]);
  const groups = useMemo(() => {
    const g = new Map<string, FixInfo[]>();
    for (const f of catalog) g.set(f.group, [...(g.get(f.group) ?? []), f]);
    return [...g.entries()];
  }, [catalog]);

  if (!unlocked) return <Gate hasScan={hasScan} onUnlock={onUnlock} onRun={onRunScan} />;

  const isAdmin = !!info?.isAdmin;
  const toggle = (id: string) => setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));
  const requests = (): FixRequest[] =>
    catalog
      .filter((f) => picked.includes(f.id))
      .map((f) => ({
        id: f.id,
        options: f.id === "clear_temp" ? tempOpts : f.id === "chkdsk" ? { drive } : f.id === "update_drivers" ? { updateIds: driverIds } : {},
      }));
  const blockers: string[] = [];
  if (picked.includes("update_drivers") && driverIds.length === 0) blockers.push("Check for driver updates and tick at least one, or untick Install driver updates.");
  if (picked.includes("clear_temp") && !Object.values(tempOpts).some(Boolean)) blockers.push("Choose at least one thing to clear.");
  const pickedInfo = catalog.filter((f) => picked.includes(f.id));

  const start = (skip: boolean) => {
    setConfirming(false);
    onRunFixes(requests(), skip);
  };
  const retry = async (skip: boolean, enableProtection: boolean) => {
    if (enableProtection) {
      try {
        await api.enableSystemProtection();
      } catch (e) {
        setError(String(e));
        return;
      }
    }
    onRunFixes(fixState?.requests ?? requests(), skip);
  };

  if (fixState || fixRun) {
    return (
      <div className="page fixes">
        <RunView state={fixState} run={fixRun} onDone={onClearRun} onRetry={retry} />
        {error && <div className="banner error">{error}</div>}
        <FixLog runs={runs} />
      </div>
    );
  }

  return (
    <div className="page fixes">
      <div className="page-head">
        <div>
          <h1>
            <Wrench size={22} /> Fixes
          </h1>
          <p className="muted">Tick the repairs you want, then run them. A restore point is created first.</p>
        </div>
        <div className="row-gap">
          <button className="btn ghost small" onClick={() => api.openSystemRestore().catch((e) => setError(String(e)))} title="Open Windows System Restore to undo changes">
            <Undo2 size={14} /> Undo with System Restore
          </button>
          <button className="btn ghost small" onClick={onLock}>
            <Lock size={14} /> Lock
          </button>
        </div>
      </div>

      {!isAdmin && (
        <div className="banner admin">
          <ShieldAlert size={16} />
          <span>Most fixes, and creating the restore point, need administrator rights. Use "Restart as administrator" at the top first.</span>
        </div>
      )}
      {error && <div className="banner error">{error}</div>}
      {report && report.machine?.id === machineId && (Date.now() - new Date(report.startedAt).getTime()) / 3_600_000 > 24 && (
        <div className="banner info-banner">
          <HistoryIcon size={16} />
          <span>Your latest scan is from {fmtWhen(report.startedAt)}. Consider scanning again so suggestions are up to date.</span>
        </div>
      )}

      {suggested.size > 0 && (
        <section className="card suggested">
          <div className="card-head">
            <h2>Suggested by your scan</h2>
          </div>
          {[...suggested.entries()].map(([fixId, titles]) => {
            const f = catalog.find((c) => c.id === fixId);
            if (!f) return null;
            return (
              <div key={fixId} className="suggest-row">
                <div>
                  <div className="suggest-title">{f.title}</div>
                  <div className="muted small">For: {titles.join("; ")}</div>
                </div>
                <button className={`btn small ${picked.includes(fixId) ? "ghost" : ""}`} onClick={() => toggle(fixId)}>
                  {picked.includes(fixId) ? "Selected ✓" : "Select"}
                </button>
              </div>
            );
          })}
        </section>
      )}

      {groups.map(([g, list]) => (
        <section key={g} className="card">
          <div className="check-group-title">{g}</div>
          {list.map((f) => {
            const on = picked.includes(f.id);
            const disabled = f.needsAdmin && !isAdmin;
            return (
              <div key={f.id} className={`fix-card ${on ? "on" : ""} ${disabled ? "disabled" : ""}`}>
                <div className="fix-card-head">
                  <input type="checkbox" checked={on} disabled={disabled} onChange={() => toggle(f.id)} aria-label={`Select ${f.title}`} />
                  <div className="fix-card-text" onClick={() => !disabled && toggle(f.id)}>
                    <div className="fix-title">
                      {f.title}
                      {suggested.has(f.id) && <span className="tag suggested-tag">suggested</span>}
                    </div>
                    <div className="muted small">{f.description}</div>
                    <div className="fix-meta">
                      <span className={`risk risk-${f.risk}`}>{riskText[f.risk]}</span>
                      <span className="muted small">Takes {f.duration}</span>
                      {f.needsAdmin && <span className="tag">admin</span>}
                      {f.mayNeedRestart && <span className="tag">may need restart</span>}
                    </div>
                  </div>
                  <button className="link-btn" onClick={() => setExpanded(expanded === f.id ? null : f.id)}>
                    What it changes <ChevronRight size={12} className={`chev ${expanded === f.id ? "open" : ""}`} />
                  </button>
                </div>
                {expanded === f.id && <div className="fix-changes">{f.changes}</div>}
                {on && f.id === "clear_temp" && (
                  <div className="fix-options">
                    {(
                      [
                        ["userTemp", "Your temporary files"],
                        ["windowsTemp", "Windows temporary files (admin)"],
                        ["errorReports", "Old error reports (admin)"],
                        ["recycleBin", "Empty the Recycle Bin (permanently deletes what's in it)"],
                      ] as const
                    ).map(([k, label]) => (
                      <label key={k} className="opt-toggle">
                        <input type="checkbox" checked={tempOpts[k]} onChange={(e) => setTempOpts({ ...tempOpts, [k]: e.target.checked })} />
                        <span>{label}</span>
                      </label>
                    ))}
                  </div>
                )}
                {on && f.id === "chkdsk" && (
                  <div className="fix-options">
                    <label className="opt-inline">
                      Drive to check
                      <select value={drive} onChange={(e) => setDrive(e.target.value)}>
                        {drives.map(([l, label, size]) => (
                          <option key={l} value={l}>
                            {l} {label && `(${label})`} · {size}
                          </option>
                        ))}
                      </select>
                    </label>
                  </div>
                )}
                {on && f.id === "update_drivers" && (
                  <div className="fix-options">
                    <DriverPanel picked={driverIds} setPicked={setDriverIds} />
                  </div>
                )}
              </div>
            );
          })}
        </section>
      ))}

      <FixLog runs={runs} />

      {picked.length > 0 && (
        <div className="fix-bar">
          <div>
            <b>
              {picked.length} fix{picked.length > 1 ? "es" : ""} selected
            </b>
            <div className="muted small">{blockers[0] ?? "A restore point will be created first."}</div>
          </div>
          <button className="btn ghost small" onClick={() => setPicked([])}>
            Clear
          </button>
          <button className="btn" disabled={blockers.length > 0} onClick={() => setConfirming(true)}>
            <Play size={15} /> Review and run
          </button>
        </div>
      )}

      {confirming && (
        <div className="modal-backdrop" onClick={() => setConfirming(false)}>
          <div className="modal card" onClick={(e) => e.stopPropagation()} role="dialog" aria-modal="true" aria-label="Confirm fixes">
            <h2>Run these fixes?</h2>
            <p className="muted small">Here's exactly what will change. A restore point is created first so system changes can be undone.</p>
            <div className="confirm-list">
              <div className="confirm-item">
                <RotateCcw size={16} />
                <div>
                  <b>Create a restore point</b>
                  <div className="muted small">Saves the current state of Windows system files and settings.</div>
                </div>
              </div>
              {pickedInfo.map((f) => (
                <div key={f.id} className="confirm-item">
                  <Wrench size={16} />
                  <div>
                    <b>{f.title}</b> <StatusBadge status={f.risk === "low" ? "info" : "warn"} text={riskText[f.risk]} />
                    <div className="muted small">{f.changes}</div>
                    {f.id === "clear_temp" && tempOpts.recycleBin && <div className="driver-warn bad">The Recycle Bin will be emptied. Those files can't be recovered.</div>}
                    {f.id === "chkdsk" && <div className="muted small">Drive: {drive}</div>}
                    {f.id === "update_drivers" && <div className="muted small">{driverIds.length} driver update(s) selected.</div>}
                  </div>
                </div>
              ))}
            </div>
            <div className="row-gap end">
              <button className="btn ghost" onClick={() => setConfirming(false)}>
                Cancel
              </button>
              <button className="btn" onClick={() => start(false)}>
                <ShieldCheck size={15} /> Create restore point and run
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
