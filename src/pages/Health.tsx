import { useMemo } from "react";
import { FileText, HeartPulse, History as HistoryIcon, Play, RefreshCw, ShieldAlert } from "lucide-react";
import type { CheckInfo, Mode, ScanKind, ScanReport, ScanState } from "../lib/types";
import { ScoreRing } from "../components/ScoreRing";
import { ScanProgress } from "../components/ScanProgress";
import { FindingList, collectFindings } from "../components/FindingList";
import { CheckCard } from "../components/CheckCard";

const kindName: Record<ScanKind, string> = { fast: "Fast scan", full: "Full scan", custom: "Custom scan" };

export function gradeText(score: number) {
  if (score >= 90) return "This PC is in great shape.";
  if (score >= 75) return "This PC is in good shape, with a few things worth a look.";
  if (score >= 50) return "This PC has some problems that should be looked at.";
  return "This PC has serious problems that need attention.";
}

export function NoScanYet({ onRun, text }: { onRun: (kind: ScanKind) => void; text: string }) {
  return (
    <div className="empty-state card">
      <div className="placeholder-icon">
        <HeartPulse size={28} />
      </div>
      <h1>No scan yet</h1>
      <p className="muted">{text}</p>
      <div className="row-gap center">
        <button className="btn" onClick={() => onRun("fast")}>
          <Play size={15} /> Run Fast scan
        </button>
        <button className="btn ghost" onClick={() => onRun("full")}>
          Run Full scan
        </button>
      </div>
    </div>
  );
}

export function Health({
  report,
  scan,
  catalog,
  mode,
  onRun,
  currentMachineId,
  onReport,
}: {
  report: ScanReport | null;
  scan: ScanState | null;
  catalog: CheckInfo[];
  mode: Mode;
  onRun: (kind: ScanKind, checks?: string[]) => void;
  currentMachineId: string | null;
  onReport: () => void;
}) {
  const findings = useMemo(() => (report ? collectFindings(report.results) : []), [report]);
  const groups = useMemo(() => {
    const m = new Map<string, ScanReport["results"]>();
    for (const r of report?.results ?? []) m.set(r.group, [...(m.get(r.group) ?? []), r]);
    return [...m.entries()];
  }, [report]);

  if (scan) {
    return (
      <div className="page">
        <ScanProgress scan={scan} catalog={catalog} />
      </div>
    );
  }
  if (!report) {
    return (
      <div className="page">
        <NoScanYet onRun={onRun} text="Run a scan to check drive health, crashes, hardware errors, Windows Update, security and more. Scans only look; nothing is changed." />
      </div>
    );
  }
  const limited = report.results.filter((r) => r.limited);
  const otherPc = !!currentMachineId && !!report.machine?.id && report.machine.id !== currentMachineId;
  const ageHours = (Date.now() - new Date(report.startedAt).getTime()) / 3_600_000;
  const when = new Date(report.startedAt);

  return (
    <div className="page health">
      {(otherPc || ageHours > 1) && (
        <div className="banner info-banner">
          <HistoryIcon size={16} />
          <span>
            {otherPc
              ? `You're viewing a saved scan of another PC: ${report.machine.computerName}. "Scan again" scans this PC instead.`
              : `This is a saved scan from ${when.toLocaleString([], { dateStyle: "medium", timeStyle: "short" })}. Scan again for up-to-date results.`}
          </span>
        </div>
      )}
      <section className="health-hero card">
        <ScoreRing score={report.score} grade={report.grade} />
        <div className="health-hero-text">
          <div className="eyebrow">Health score</div>
          <h1>{gradeText(report.score)}</h1>
          <div className="hero-chips">
            {report.counts.error > 0 && <span className="chip c-error">{report.counts.error} problem{report.counts.error > 1 ? "s" : ""}</span>}
            {report.counts.warn > 0 && <span className="chip c-warn">{report.counts.warn} warning{report.counts.warn > 1 ? "s" : ""}</span>}
            {report.counts.info > 0 && <span className="chip c-info">{report.counts.info} note{report.counts.info > 1 ? "s" : ""}</span>}
            {report.counts.error + report.counts.warn === 0 && <span className="chip c-ok">No problems found</span>}
          </div>
          <div className="muted small">
            {report.machine?.computerName && `${report.machine.computerName} · `}
            {kindName[report.kind]} · {report.results.length} checks · {(report.elapsedMs / 1000).toFixed(1)} s · {when.toLocaleDateString()} {when.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
          </div>
        </div>
        <div className="health-hero-actions">
          <button className="btn" onClick={() => onRun(report.kind === "custom" ? "full" : report.kind)}>
            <RefreshCw size={15} /> Scan again
          </button>
          <button className="btn ghost" onClick={onReport}>
            <FileText size={15} /> Make a report
          </button>
          {report.kind === "fast" && (
            <button className="btn ghost" onClick={() => onRun("full")}>
              Run Full scan
            </button>
          )}
        </div>
      </section>

      {limited.length > 0 && !report.isAdmin && (
        <div className="banner admin">
          <ShieldAlert size={16} />
          <span>
            {limited.length} check{limited.length > 1 ? "s" : ""} gave partial results because DxTool is not running as administrator. Use the button at the top to restart it with full access . Affected: {limited.map((l) => l.title).join(", ")}.
          </span>
        </div>
      )}

      <section className="card">
        <div className="card-head">
          <h2>Findings</h2>
          <span className="muted small">Most serious first. Click one to see what it means and what to do.</span>
        </div>
        <FindingList findings={findings} emptyText="No problems found. Everything checked looks healthy." />
      </section>

      <section className="card">
        <div className="card-head">
          <h2>All checks</h2>
          <span className="muted small">Click a check to see the details it collected</span>
        </div>
        {groups.map(([g, results]) => (
          <div key={g} className="check-section">
            <div className="check-group-title">{g}</div>
            {results.map((r) => (
              <CheckCard key={r.id} result={r} mode={mode} />
            ))}
          </div>
        ))}
      </section>
    </div>
  );
}
