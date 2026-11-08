import { useMemo } from "react";
import { Bug, ChevronRight, ShieldCheck } from "lucide-react";
import type { CheckInfo, Mode, ScanKind, ScanReport, ScanState } from "../lib/types";
import { ScanProgress } from "../components/ScanProgress";
import { FindingList, collectFindings } from "../components/FindingList";
import { CheckCard } from "../components/CheckCard";
import { NoScanYet } from "./Health";

export function Security({
  report,
  scan,
  catalog,
  mode,
  onRun,
  onFix,
  onOpenVirus,
}: {
  report: ScanReport | null;
  scan: ScanState | null;
  catalog: CheckInfo[];
  mode: Mode;
  onRun: (kind: ScanKind, checks?: string[]) => void;
  onFix: (fixId: string) => void;
  onOpenVirus: () => void;
}) {
  const results = useMemo(() => (report?.results ?? []).filter((r) => r.group === "Security"), [report]);
  const findings = useMemo(() => collectFindings(results), [results]);

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>
            <ShieldCheck size={22} /> Security
          </h1>
          <p className="muted">Antivirus, firewall and the security features built into the PC and Windows.</p>
        </div>
      </div>
      {scan ? (
        <ScanProgress scan={scan} catalog={catalog} />
      ) : results.length === 0 ? (
        <NoScanYet onRun={onRun} text="Security status comes from the health scan. A Fast scan includes antivirus, firewall and device security checks." />
      ) : (
        <>
          <section className="card">
            <div className="card-head">
              <h2>Findings</h2>
              <span className="muted small">From the {report?.kind} scan at {report && new Date(report.startedAt).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
            </div>
            <FindingList onFix={onFix} findings={findings} emptyText="No security problems found." />
          </section>
          <section className="card">
            <div className="card-head">
              <h2>Security checks</h2>
            </div>
            {results.map((r) => (
              <CheckCard key={r.id} result={r} mode={mode} defaultOpen />
            ))}
          </section>
        </>
      )}
      <button className="card coming link-card" onClick={onOpenVirus}>
        <Bug size={18} />
        <div>
          <b>Scan for viruses</b>
          <div className="muted small">Run a Microsoft Defender scan, or get a second opinion from ClamAV, on the Virus scan page.</div>
        </div>
        <ChevronRight size={16} className="chev" />
      </button>
    </div>
  );
}
