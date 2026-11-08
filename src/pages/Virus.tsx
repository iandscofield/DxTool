import { useCallback, useEffect, useState } from "react";
import {
  AlertTriangle,
  Bug,
  CheckCircle2,
  ChevronRight,
  Download,
  ExternalLink,
  FolderOpen,
  FolderSearch,
  Loader2,
  RefreshCw,
  ShieldCheck,
  ShieldOff,
  Square,
  Trash2,
  Undo2,
  XCircle,
} from "lucide-react";
import { api } from "../lib/api";
import type { AvJobApi } from "../lib/useAvJob";
import type { AvJob, AvStatus, Mode, QuarantineItem, Threat, VirusScan } from "../lib/types";

const engineName = { defender: "Microsoft Defender", clamav: "ClamAV" } as const;
const kindName = { quick: "Quick", full: "Full", custom: "Custom" } as const;

function mb(bytes: number) {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(2)} GB`;
  return `${Math.max(0, Math.round(bytes / 1e6))} MB`;
}
function ageText(days: number | null | undefined, never = "never") {
  if (days == null) return never;
  if (days === 0) return "today";
  if (days === 1) return "yesterday";
  return `${days} days ago`;
}
function when(iso: string) {
  return new Date(iso).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
}
function duration(ms: number) {
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${s % 60}s`;
  return `${Math.floor(m / 60)}h ${m % 60}m`;
}

export function Virus({ av, machineId, mode }: { av: AvJobApi; machineId: string | null; mode: Mode }) {
  const [status, setStatus] = useState<AvStatus | null>(null);
  const [scans, setScans] = useState<VirusScan[]>([]);
  const [quarantine, setQuarantine] = useState<QuarantineItem[]>([]);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const finished = av.job?.finished;

  const reload = useCallback(() => {
    api.avStatus().then(setStatus).catch((e) => setError(String(e)));
    if (machineId) api.virusScans(machineId).then(setScans).catch(() => setScans([]));
    api.quarantineList().then(setQuarantine).catch(() => setQuarantine([]));
  }, [machineId]);
  useEffect(() => {
    if (!av.running) reload();
  }, [reload, av.running, finished]);

  const onScanChanged = (s: VirusScan) => {
    setScans((list) => list.map((x) => (x.id === s.id ? s : x)));
    av.updateResult(s);
    api.quarantineList().then(setQuarantine).catch(() => {});
  };
  const reloadScan = async (id: string) => {
    if (!machineId) return;
    const list = await api.virusScans(machineId);
    setScans(list);
    const s = list.find((x) => x.id === id);
    if (s) av.updateResult(s);
    setQuarantine(await api.quarantineList());
  };

  const pick = async (engine: "defender" | "clamav", file = false) => {
    const p = file ? await api.pickFile() : await api.pickFolder();
    if (p) av.scan(engine, "custom", [p]);
  };

  const d = status?.defender;
  const c = status?.clamav;
  const busy = av.running;

  return (
    <div className="page virus">
      <div className="page-head">
        <div>
          <h1>
            <Bug size={22} /> Virus scan
          </h1>
          <p className="muted">Scan with Microsoft Defender, Windows' built-in antivirus, and get a second opinion from ClamAV.</p>
        </div>
      </div>
      {error && <div className="banner error">{error}</div>}

      {av.job && <JobPanel job={av.job} mode={mode} onCancel={av.cancel} onClose={av.clear} machineId={machineId} onScanChanged={onScanChanged} onReloadScan={reloadScan} />}

      <div className="av-engines">
        <section className="card av-engine">
          <div className="av-engine-head">
            <div className={`av-logo ${d?.available ? "ok" : "off"}`}>{d?.available ? <ShieldCheck size={22} /> : <ShieldOff size={22} />}</div>
            <div>
              <h2>Microsoft Defender</h2>
              <div className="muted small">Built into Windows. Uses Microsoft's cloud-backed definitions.</div>
            </div>
          </div>
          {!status ? (
            <div className="muted small av-loading">
              <Loader2 size={14} className="spin" /> Checking…
            </div>
          ) : !d?.available ? (
            <div className="banner warn">
              <AlertTriangle size={16} /> {d?.reason}
            </div>
          ) : (
            <>
              <dl className="kv compact">
                <div className="kv-row">
                  <dt>Real-time protection</dt>
                  <dd className={d.realtime ? "txt-ok" : "txt-error"}>{d.realtime ? "On" : "Off: turn it on in Windows Security"}</dd>
                </div>
                <div className="kv-row">
                  <dt>Virus definitions</dt>
                  <dd className={(d.signatureAgeDays ?? 0) > 7 ? "txt-warn" : ""}>
                    {d.signatureVersion} · updated {ageText(d.signatureAgeDays, "unknown")}
                  </dd>
                </div>
                <div className="kv-row">
                  <dt>Last quick scan</dt>
                  <dd>{ageText(d.quickScanAgeDays)}</dd>
                </div>
                <div className="kv-row">
                  <dt>Last full scan</dt>
                  <dd>{ageText(d.fullScanAgeDays)}</dd>
                </div>
                {mode === "advanced" && (
                  <div className="kv-row">
                    <dt>Versions</dt>
                    <dd>
                      Antimalware {d.productVersion} · engine {d.engineVersion}
                    </dd>
                  </div>
                )}
              </dl>
              <div className="av-actions">
                <button className="btn" disabled={busy} onClick={() => av.scan("defender", "quick")} title="Memory, startup items and the places malware usually hides. A few minutes.">
                  Quick scan
                </button>
                <button className="btn ghost" disabled={busy} onClick={() => av.scan("defender", "full")} title="Every file on every drive. Can take an hour or more.">
                  Full scan
                </button>
                <button className="btn ghost" disabled={busy} onClick={() => pick("defender")}>
                  <FolderSearch size={14} /> Scan a folder…
                </button>
                <button className="btn ghost" disabled={busy} onClick={() => pick("defender", true)}>
                  Scan a file…
                </button>
              </div>
              <div className="av-actions secondary">
                <button className="link-btn" disabled={busy} onClick={() => av.update("defender")}>
                  <RefreshCw size={13} /> Update definitions
                </button>
                <button className="link-btn" onClick={() => api.openWindowsSecurity()}>
                  <ExternalLink size={13} /> Defender's quarantine (Windows Security)
                </button>
              </div>
            </>
          )}
        </section>

        <section className="card av-engine">
          <div className="av-engine-head">
            <div className={`av-logo ${c?.installed && c.databases ? "ok" : "off"}`}>
              <Bug size={22} />
            </div>
            <div>
              <h2>ClamAV second opinion</h2>
              <div className="muted small">Free, open-source scanner from Cisco Talos. Catches things one antivirus can miss.</div>
            </div>
          </div>
          {!status ? (
            <div className="muted small av-loading">
              <Loader2 size={14} className="spin" /> Checking…
            </div>
          ) : !c?.installed || !c.databases ? (
            <div className="av-install">
              <p className="small">
                {c?.installed
                  ? "ClamAV is downloaded, but its virus databases aren't yet."
                  : "ClamAV isn't included with DxTool. Download it from ClamAV's official releases when you want a second opinion. It runs alongside Defender without conflicts."}
              </p>
              <ul className="small muted">
                <li>Downloads about {c?.installed ? "230 MB" : "450 MB"} (the program, then the virus databases)</li>
                <li>Uses about 300 MB of disk, inside DxTool's data folder</li>
                <li>Scans need about 1.5 GB of free memory while they run</li>
              </ul>
              <button className="btn" disabled={busy} onClick={() => (c?.installed ? av.update("clamav") : av.install())}>
                <Download size={14} /> {c?.installed ? "Download virus databases" : "Download ClamAV"}
              </button>
            </div>
          ) : (
            <>
              <dl className="kv compact">
                <div className="kv-row">
                  <dt>Version</dt>
                  <dd>ClamAV {c.version}</dd>
                </div>
                <div className="kv-row">
                  <dt>Virus databases</dt>
                  <dd className={(c.dbAgeDays ?? 0) > 7 ? "txt-warn" : ""}>
                    Version {c.dbVersion} · {ageText(c.dbAgeDays, "unknown")}
                    {(c.dbAgeDays ?? 0) > 7 && " (update them before scanning)"}
                  </dd>
                </div>
                <div className="kv-row">
                  <dt>Signatures</dt>
                  <dd>{c.signatures.toLocaleString()}</dd>
                </div>
                {mode === "advanced" && (
                  <div className="kv-row">
                    <dt>Folder</dt>
                    <dd className="mono-sm">
                      {c.folder} · {mb(c.sizeBytes)}
                    </dd>
                  </div>
                )}
              </dl>
              <div className="av-actions">
                <button className="btn" disabled={busy} onClick={() => av.scan("clamav", "quick")} title="Risky file types in Downloads, Desktop and temp folders, startup items, and programs in AppData and ProgramData. Skips files over 20 MB. Usually 10-30 minutes.">
                  Quick scan
                </button>
                <button className="btn ghost" disabled={busy} onClick={() => av.scan("clamav", "full")} title="Every file on every drive. Can take several hours.">
                  Full scan
                </button>
                <button className="btn ghost" disabled={busy} onClick={() => pick("clamav")}>
                  <FolderSearch size={14} /> Scan a folder…
                </button>
                <button className="btn ghost" disabled={busy} onClick={() => pick("clamav", true)}>
                  Scan a file…
                </button>
              </div>
              <div className="av-actions secondary">
                <button className="link-btn" disabled={busy} onClick={() => av.update("clamav")}>
                  <RefreshCw size={13} /> Update databases
                </button>
                {confirmRemove ? (
                  <span className="skip-confirm small">
                    Remove ClamAV and its databases ({mb(c.sizeBytes)})?
                    <button
                      className="btn small danger"
                      onClick={() => {
                        setConfirmRemove(false);
                        api.clamavRemove().then(reload).catch((e) => setError(String(e)));
                      }}
                    >
                      Remove
                    </button>
                    <button className="btn small ghost" onClick={() => setConfirmRemove(false)}>
                      Keep
                    </button>
                  </span>
                ) : (
                  <button className="link-btn" disabled={busy} onClick={() => setConfirmRemove(true)}>
                    <Trash2 size={13} /> Remove ClamAV
                  </button>
                )}
              </div>
            </>
          )}
        </section>
      </div>

      {quarantine.length > 0 && (
        <section className="card">
          <div className="card-head">
            <h2>DxTool quarantine</h2>
            <span className="muted small">Files moved here can't run. Restore them only if you're sure they're safe.</span>
          </div>
          <table className="simple-table">
            <thead>
              <tr>
                <th>Threat</th>
                <th>Original location</th>
                <th>Quarantined</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {quarantine.map((q) => (
                <QuarantineRow key={q.id} q={q} onChanged={() => api.quarantineList().then(setQuarantine)} />
              ))}
            </tbody>
          </table>
        </section>
      )}

      <section className="card">
        <div className="card-head">
          <h2>Scan history</h2>
          <span className="muted small">Virus scans on this PC, newest first</span>
        </div>
        {scans.length === 0 ? (
          <div className="muted small">No virus scans yet.</div>
        ) : (
          <div className="fix-log">
            {scans.map((s) => (
              <HistoryRow key={s.id} s={s} machineId={machineId} onScanChanged={onScanChanged} onReloadScan={reloadScan} onDelete={() => machineId && api.deleteVirusScan(machineId, s.id).then(reload)} />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

function JobPanel({
  job,
  mode,
  onCancel,
  onClose,
  machineId,
  onScanChanged,
  onReloadScan,
}: {
  job: AvJob;
  mode: Mode;
  onCancel: () => void;
  onClose: () => void;
  machineId: string | null;
  onScanChanged: (s: VirusScan) => void;
  onReloadScan: (id: string) => void;
}) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (job.finished) return;
    const t = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(t);
  }, [job.finished]);
  const [showLog, setShowLog] = useState(false);
  const isBytes = job.op !== "scan";
  const pct = job.total > 0 ? Math.min(100, (job.done / job.total) * 100) : null;
  const r = job.result;

  if (job.finished) {
    const active = r ? r.threats.filter((t) => t.status === "active").length : 0;
    const tone = job.error || r?.outcome === "failed" ? "error" : r?.outcome === "threats" && active > 0 ? "warn" : r?.outcome === "cancelled" ? "info" : "ok";
    const Icon = tone === "error" ? XCircle : tone === "warn" ? AlertTriangle : CheckCircle2;
    return (
      <section className={`card av-job done tone-${tone}`}>
        <div className="av-result-head">
          <Icon size={26} />
          <div>
            <h2>{job.title}</h2>
            <div>{job.error ?? (r?.outcome === "threats" && active === 0 ? "All threats found have been dealt with." : `${r?.message ?? job.message ?? ""}${active > 0 ? " Choose what to do with each one below." : ""}`)}</div>
            {r && (
              <div className="muted small">
                {when(r.startedAt)} · took {duration(r.elapsedMs)}
                {r.filesScanned > 0 && ` · ${r.filesScanned.toLocaleString()} files`}
                {r.targets.length > 0 && ` · ${r.targets.join(", ")}`}
              </div>
            )}
          </div>
          <button className="btn small ghost" onClick={onClose}>
            Close
          </button>
        </div>
        {r && r.threats.length > 0 && <ThreatTable scan={r} machineId={machineId} onScanChanged={onScanChanged} onReloadScan={onReloadScan} />}
        {mode === "advanced" && job.lines.length > 0 && <pre className="console">{job.lines.join("\n")}</pre>}
      </section>
    );
  }

  return (
    <section className="card av-job">
      <div className="card-head">
        <h2>
          <Loader2 size={18} className="spin" /> {job.title}
        </h2>
        <div className="row-gap">
          <span className="muted small">{duration(now - job.startedAt)}</span>
          <button className="btn small ghost" onClick={onCancel} disabled={job.cancelling}>
            <Square size={12} /> {job.cancelling ? "Stopping…" : "Stop"}
          </button>
        </div>
      </div>
      <div className="av-phase">{job.phase}</div>
      <div className={`progress-bar ${pct == null ? "indeterminate" : ""}`}>
        <span style={pct == null ? undefined : { width: `${pct}%` }} />
      </div>
      <div className="av-progress-text muted small">
        {job.total > 0
          ? isBytes
            ? `${mb(job.done)} of ${mb(job.total)}`
            : `${job.done.toLocaleString()} of ${job.total.toLocaleString()} files (${Math.floor(pct!)}%)`
          : job.done > 0
            ? isBytes
              ? `${mb(job.done)} downloaded`
              : `${job.done.toLocaleString()} files found so far`
            : job.engine === "defender" && job.op === "scan"
              ? "Defender doesn't report progress while it scans. It will finish on its own; you can keep using DxTool."
              : ""}
      </div>
      {job.current && <div className="av-current mono-sm">{job.current}</div>}
      {job.threats.length > 0 && (
        <div className="av-live-threats">
          <AlertTriangle size={15} /> {job.threats.length} threat{job.threats.length === 1 ? "" : "s"} found so far:{" "}
          {job.threats
            .slice(-3)
            .map((t) => t.name)
            .join(", ")}
        </div>
      )}
      {job.lines.length > 0 && (
        <>
          <button className="link-btn" onClick={() => setShowLog(!showLog)}>
            <ChevronRight size={13} className={`chev ${showLog ? "open" : ""}`} /> Details
          </button>
          {showLog && <pre className="console">{job.lines.join("\n")}</pre>}
        </>
      )}
    </section>
  );
}

function ThreatTable({ scan, machineId, onScanChanged, onReloadScan }: { scan: VirusScan; machineId: string | null; onScanChanged: (s: VirusScan) => void; onReloadScan: (id: string) => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<Threat | null>(null);
  const activeDefender = scan.threats.filter((t) => t.engine === "defender" && t.status === "active");

  const act = async (key: string, f: () => Promise<unknown>) => {
    setBusy(key);
    setError(null);
    try {
      await f();
      onReloadScan(scan.id);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="threats">
      {activeDefender.length > 0 && (
        <div className="threat-bar">
          <span>
            Defender found {activeDefender.length === 1 ? "this threat" : `${activeDefender.length} threats`} but hasn't removed {activeDefender.length === 1 ? "it" : "them"} yet.
          </span>
          <button
            className="btn small"
            disabled={!!busy}
            onClick={() =>
              machineId &&
              act("defender", async () => {
                const s = await api.defenderRemove(machineId, scan.id);
                onScanChanged(s);
              })
            }
          >
            {busy === "defender" ? <Loader2 size={13} className="spin" /> : <ShieldCheck size={13} />} Remove with Defender (recommended)
          </button>
        </div>
      )}
      {error && <div className="banner error">{error}</div>}
      <table className="simple-table threat-table">
        <thead>
          <tr>
            <th>Threat</th>
            <th>File</th>
            <th>What happened</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {scan.threats.map((t, i) => (
            <tr key={`${t.path}-${i}`}>
              <td>
                <div className="threat-name">{t.name}</div>
                <div className="muted small">
                  {engineName[t.engine]}
                  {t.severity && ` · ${t.severity} severity`}
                </div>
              </td>
              <td className="wrap mono-sm">{t.path}</td>
              <td className={`nw threat-status st-${t.status}`}>{t.action}</td>
              <td className="nw">
                {t.status === "active" && t.engine === "clamav" && machineId && (
                  <div className="row-gap">
                    <button className="btn small ghost" disabled={!!busy} onClick={() => act(t.path, () => api.threatQuarantine(machineId, scan.id, t.path))} title="Move it into DxTool's quarantine, where it can't run. You can restore it later.">
                      {busy === t.path ? <Loader2 size={12} className="spin" /> : null} Quarantine
                    </button>
                    <button className="btn small ghost danger-text" disabled={!!busy} onClick={() => setConfirmDelete(t)}>
                      <Trash2 size={12} /> Delete
                    </button>
                  </div>
                )}
                {t.status === "active" && (
                  <button className="link-btn small" onClick={() => api.showInFolder(t.path)}>
                    <FolderOpen size={12} /> Show
                  </button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {confirmDelete && machineId && (
        <div className="modal-backdrop" onClick={() => setConfirmDelete(null)}>
          <div className="card modal" onClick={(e) => e.stopPropagation()}>
            <h2>Delete this file permanently?</h2>
            <p className="muted">
              <b>{confirmDelete.name}</b> in
              <br />
              <span className="mono-sm">{confirmDelete.path}</span>
            </p>
            <p>It won't go to the Recycle Bin and can't be undone. If you're not sure, quarantine it instead: that also stops it running, and you can restore it later.</p>
            <div className="row-gap end">
              <button className="btn ghost" onClick={() => setConfirmDelete(null)}>
                Cancel
              </button>
              <button
                className="btn ghost"
                onClick={() => {
                  const t = confirmDelete;
                  setConfirmDelete(null);
                  act(t.path, () => api.threatQuarantine(machineId, scan.id, t.path));
                }}
              >
                Quarantine instead
              </button>
              <button
                className="btn danger"
                onClick={() => {
                  const t = confirmDelete;
                  setConfirmDelete(null);
                  act(t.path, () => api.threatDelete(machineId, scan.id, t.path));
                }}
              >
                <Trash2 size={14} /> Delete permanently
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function HistoryRow({
  s,
  machineId,
  onScanChanged,
  onReloadScan,
  onDelete,
}: {
  s: VirusScan;
  machineId: string | null;
  onScanChanged: (s: VirusScan) => void;
  onReloadScan: (id: string) => void;
  onDelete: () => void;
}) {
  const [open, setOpen] = useState(false);
  const active = s.threats.filter((t) => t.status === "active").length;
  const Icon = s.outcome === "failed" ? XCircle : s.outcome === "threats" ? AlertTriangle : s.outcome === "cancelled" ? Square : CheckCircle2;
  const tone = s.outcome === "failed" ? "step-fail" : s.outcome === "threats" ? (active ? "step-fail" : "step-ok") : s.outcome === "cancelled" ? "muted" : "step-ok";
  return (
    <div className="fix-log-run">
      <button className="fix-log-head av-hist-head" onClick={() => setOpen(!open)}>
        <span>{when(s.startedAt)}</span>
        <span className="muted">
          {engineName[s.engine]} · {kindName[s.kind]} scan
        </span>
        <span className={`small ${tone}`}>
          <Icon size={13} />{" "}
          {s.outcome === "threats" ? `${s.threats.length} threat${s.threats.length === 1 ? "" : "s"}${active ? `, ${active} still active` : ", dealt with"}` : s.outcome === "clean" ? "No threats" : s.outcome === "cancelled" ? "Stopped" : "Failed"}
        </span>
        <ChevronRight size={14} className={`chev ${open ? "open" : ""}`} />
      </button>
      {open && (
        <div className="fix-log-body">
          <div className="small">{s.message}</div>
          <div className="muted small">
            Took {duration(s.elapsedMs)}
            {s.filesScanned > 0 && ` · ${s.filesScanned.toLocaleString()} files`} · {s.targets.join(", ")}
            {s.signatureVersion && ` · definitions ${s.signatureVersion}`}
          </div>
          {s.threats.length > 0 && <ThreatTable scan={s} machineId={machineId} onScanChanged={onScanChanged} onReloadScan={onReloadScan} />}
          <div>
            <button className="link-btn small" onClick={onDelete}>
              <Trash2 size={12} /> Remove from history
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function QuarantineRow({ q, onChanged }: { q: QuarantineItem; onChanged: () => void }) {
  const [confirm, setConfirm] = useState<"restore" | "delete" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const run = (f: () => Promise<unknown>) => {
    setConfirm(null);
    setError(null);
    f()
      .then(onChanged)
      .catch((e) => setError(String(e)));
  };
  return (
    <tr>
      <td>{q.threat}</td>
      <td className="wrap mono-sm">
        {q.originalPath}
        {error && <div className="txt-error small">{error}</div>}
      </td>
      <td className="nw">{when(q.quarantinedAt)}</td>
      <td className="nw">
        {confirm ? (
          <span className="skip-confirm small">
            {confirm === "restore" ? "Put it back? It could harm the PC." : "Delete for good?"}
            <button className={`btn small ${confirm === "delete" ? "danger" : ""}`} onClick={() => run(() => (confirm === "restore" ? api.quarantineRestore(q.id) : api.quarantineDelete(q.id)))}>
              Yes
            </button>
            <button className="btn small ghost" onClick={() => setConfirm(null)}>
              No
            </button>
          </span>
        ) : (
          <div className="row-gap">
            <button className="btn small ghost" onClick={() => setConfirm("restore")}>
              <Undo2 size={12} /> Restore
            </button>
            <button className="btn small ghost danger-text" onClick={() => setConfirm("delete")}>
              <Trash2 size={12} /> Delete
            </button>
          </div>
        )}
      </td>
    </tr>
  );
}
