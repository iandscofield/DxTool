import { useState } from "react";
import { ChevronRight, Lightbulb, Wrench } from "lucide-react";
import type { CheckResult, Finding, Status } from "../lib/types";
import { StatusBadge } from "./Status";

export interface FindingRef {
  finding: Finding;
  check: CheckResult;
}

const sevLabel: Record<Status, string> = { error: "Problem", warn: "Warning", info: "Note", ok: "OK" };
const order: Record<Status, number> = { error: 0, warn: 1, info: 2, ok: 3 };

export function collectFindings(results: CheckResult[]): FindingRef[] {
  return results
    .flatMap((check) => check.findings.map((finding) => ({ finding, check })))
    .sort((a, b) => order[a.finding.severity] - order[b.finding.severity]);
}

/** Findings ranked by severity, each expandable to show what it means and what to do. */
export function FindingList({ findings, emptyText, onFix }: { findings: FindingRef[]; emptyText?: string; onFix?: (fixId: string) => void }) {
  const [filter, setFilter] = useState<Status | "all">("all");
  const [open, setOpen] = useState<string | null>(null);
  const counts = { error: 0, warn: 0, info: 0, ok: 0 } as Record<Status, number>;
  for (const f of findings) counts[f.finding.severity]++;
  const list = filter === "all" ? findings : findings.filter((f) => f.finding.severity === filter);

  if (findings.length === 0) return <div className="all-good">{emptyText ?? "Nothing to report."}</div>;
  return (
    <div className="findings">
      <div className="chip-filter">
        <button className={filter === "all" ? "on" : ""} onClick={() => setFilter("all")}>
          All <span>{findings.length}</span>
        </button>
        {(["error", "warn", "info"] as Status[]).map(
          (s) =>
            counts[s] > 0 && (
              <button key={s} className={`${filter === s ? "on" : ""} f-${s}`} onClick={() => setFilter(s)}>
                {sevLabel[s]}
                {counts[s] > 1 && s !== "info" ? "s" : s === "info" && counts[s] > 1 ? "s" : ""} <span>{counts[s]}</span>
              </button>
            ),
        )}
      </div>
      <div className="finding-list">
        {list.map(({ finding: f, check }) => {
          const key = `${check.id}:${f.id}`;
          const isOpen = open === key;
          return (
            <div key={key} className={`finding sev-${f.severity} ${isOpen ? "open" : ""}`}>
              <button className="finding-head" onClick={() => setOpen(isOpen ? null : key)} aria-expanded={isOpen}>
                <StatusBadge status={f.severity} text={sevLabel[f.severity]} />
                <span className="finding-title">{f.title}</span>
                <span className="tag">{check.title}</span>
                <ChevronRight size={16} className={`chev ${isOpen ? "open" : ""}`} />
              </button>
              {isOpen && (
                <div className="finding-body">
                  <p>{f.detail}</p>
                  {f.advice && (
                    <div className="advice">
                      <Lightbulb size={15} />
                      <div>
                        <b>What to do:</b> {f.advice}
                      </div>
                    </div>
                  )}
                  {f.fix && onFix && (
                    <button className="btn small ghost fix-link" onClick={() => onFix(f.fix!)}>
                      <Wrench size={13} /> Fix this in the Fixes section
                    </button>
                  )}
                  {f.evidence.filter((e) => e.display).length > 0 && (
                    <dl className="kv compact">
                      {f.evidence
                        .filter((e) => e.display)
                        .map((e) => (
                          <div key={e.key} className="kv-row">
                            <dt>{e.label}</dt>
                            <dd>{e.display}</dd>
                          </div>
                        ))}
                    </dl>
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
