import { useMemo, useState } from "react";
import { Gauge, ListChecks, Play, ShieldAlert, Zap } from "lucide-react";
import type { AppInfo, CheckInfo, ScanKind, ScanReport, ScanState } from "../lib/types";
import { loadPref, savePref } from "../lib/api";
import { ScanProgress } from "../components/ScanProgress";
import { ScoreRing } from "../components/ScoreRing";

export function Scans({
  catalog,
  scan,
  report,
  info,
  onRun,
  onViewResults,
}: {
  catalog: CheckInfo[];
  scan: ScanState | null;
  report: ScanReport | null;
  info: AppInfo | null;
  onRun: (kind: ScanKind, checks?: string[]) => void;
  onViewResults: () => void;
}) {
  const fast = catalog.filter((c) => c.fast);
  const full = catalog.filter((c) => !c.customOnly);
  const [picked, setPicked] = useState<string[]>(loadPref("scan:custom", full.map((c) => c.id)));
  const [showCustom, setShowCustom] = useState(false);
  const groups = useMemo(() => {
    const m = new Map<string, CheckInfo[]>();
    for (const c of catalog) m.set(c.group, [...(m.get(c.group) ?? []), c]);
    return [...m.entries()];
  }, [catalog]);
  const toggle = (id: string) => {
    const next = picked.includes(id) ? picked.filter((p) => p !== id) : [...picked, id];
    setPicked(next);
    savePref("scan:custom", next);
  };
  const setAll = (ids: string[]) => {
    setPicked(ids);
    savePref("scan:custom", ids);
  };
  const busy = !!scan;

  return (
    <div className="page scans">
      <div className="page-head">
        <div>
          <h1>Health scans</h1>
          <p className="muted">Scans only look. Nothing on the PC is changed. Repairs live in the Fixes section.</p>
        </div>
        {report && !scan && (
          <button className="last-scan" onClick={onViewResults}>
            <ScoreRing score={report.score} size={44} stroke={5} />
            <div>
              <div className="small muted">Last scan</div>
              <div>
                {report.grade} · {new Date(report.startedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
              </div>
            </div>
          </button>
        )}
      </div>

      {info && !info.isAdmin && (
        <div className="banner admin">
          <ShieldAlert size={16} />
          <span>For the deepest results (drive wear and temperature, crash dump files, system file check, TPM and BitLocker) run DxTool as administrator.</span>
        </div>
      )}

      {scan ? (
        <ScanProgress scan={scan} catalog={catalog} />
      ) : (
        <div className="scan-modes">
          <div className="scan-mode">
            <div className="scan-mode-icon">
              <Zap size={22} />
            </div>
            <h2>Fast scan</h2>
            <p className="muted">The most important checks: drives, crashes, hardware errors, updates, security and network. Looks back 30 days.</p>
            <div className="scan-mode-meta">{fast.length} checks · a few seconds</div>
            <button className="btn" disabled={busy} onClick={() => onRun("fast")}>
              <Play size={15} /> Run Fast scan
            </button>
          </div>
          <div className="scan-mode featured">
            <div className="scan-mode-icon">
              <Gauge size={22} />
            </div>
            <h2>Full scan</h2>
            <p className="muted">Everything in the Fast scan plus drivers, services, app crashes, Windows activation and the system image. Looks back 90 days.</p>
            <div className="scan-mode-meta">{full.length} checks · usually under a minute</div>
            <button className="btn" disabled={busy} onClick={() => onRun("full")}>
              <Play size={15} /> Run Full scan
            </button>
          </div>
          <div className="scan-mode">
            <div className="scan-mode-icon">
              <ListChecks size={22} />
            </div>
            <h2>Custom scan</h2>
            <p className="muted">Pick exactly which checks to run, including the slow deep system file check.</p>
            <div className="scan-mode-meta">{picked.length} selected</div>
            <button className="btn ghost" onClick={() => setShowCustom((s) => !s)}>
              {showCustom ? "Hide checks" : "Choose checks"}
            </button>
          </div>
        </div>
      )}

      {showCustom && !scan && (
        <section className="card custom-picker">
          <div className="card-head">
            <h2>Choose checks</h2>
            <div className="row-gap">
              <button className="btn ghost small" onClick={() => setAll(fast.map((c) => c.id))}>
                Fast set
              </button>
              <button className="btn ghost small" onClick={() => setAll(full.map((c) => c.id))}>
                Full set
              </button>
              <button className="btn ghost small" onClick={() => setAll(catalog.map((c) => c.id))}>
                Everything
              </button>
              <button className="btn ghost small" onClick={() => setAll([])}>
                None
              </button>
              <button className="btn small" disabled={picked.length === 0} onClick={() => onRun("custom", picked)}>
                <Play size={13} /> Run {picked.length} checks
              </button>
            </div>
          </div>
          <div className="check-groups">
            {groups.map(([g, list]) => (
              <div key={g} className="check-group">
                <div className="check-group-title">{g}</div>
                {list.map((c) => (
                  <label key={c.id} className={`check-option ${picked.includes(c.id) ? "on" : ""}`}>
                    <input type="checkbox" checked={picked.includes(c.id)} onChange={() => toggle(c.id)} />
                    <div>
                      <div className="check-option-title">
                        {c.title}
                        {c.fast && <span className="tag">fast</span>}
                        {c.needsAdmin && <span className="tag">admin</span>}
                        {c.customOnly && <span className="tag slow">slow</span>}
                      </div>
                      <div className="muted small">{c.description}</div>
                      <div className="muted small">Takes {c.duration}</div>
                    </div>
                  </label>
                ))}
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
