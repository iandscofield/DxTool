import { useMemo, useState } from "react";
import { AlertTriangle, ChevronRight, ShieldAlert } from "lucide-react";
import type { CheckResult, Item, Mode } from "../lib/types";
import { buildTable } from "../lib/table";
import { DataTable } from "./DataTable";
import { DetailPanel } from "./DetailPanel";
import { StatusBadge, StatusDot } from "./Status";

/** One check's outcome: summary line, and when expanded its facts and detail table. */
export function CheckCard({ result, mode, defaultOpen }: { result: CheckResult; mode: Mode; defaultOpen?: boolean }) {
  const [open, setOpen] = useState(!!defaultOpen);
  const [selected, setSelected] = useState<Item | null>(null);
  const advanced = mode === "advanced";
  const table = useMemo(() => (open ? buildTable(result.items, advanced) : null), [open, result, advanced]);
  const facts = result.facts.filter((f) => f.display);
  const noteworthy = result.findings.filter((f) => f.severity !== "ok").length;

  return (
    <div className={`check-card st-${result.status} ${open ? "open" : ""}`}>
      <button className="check-head" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        <StatusDot status={result.status} />
        <div className="check-text">
          <div className="check-title">{result.title}</div>
          <div className="muted small">{result.summary}</div>
        </div>
        {result.limited && (
          <span className="tag limited" title={result.limited}>
            partial
          </span>
        )}
        {noteworthy > 0 && <StatusBadge status={result.status} text={`${noteworthy} finding${noteworthy > 1 ? "s" : ""}`} />}
        {advanced && <span className="muted small">{result.elapsedMs} ms</span>}
        <ChevronRight size={16} className={`chev ${open ? "open" : ""}`} />
      </button>
      {open && (
        <div className="check-body">
          {result.limited && (
            <div className="banner admin">
              <ShieldAlert size={16} />
              <span>{result.limited}</span>
            </div>
          )}
          {result.error && (
            <div className="banner warn">
              <AlertTriangle size={16} />
              <span>{result.error}</span>
            </div>
          )}
          {facts.length > 0 && (
            <dl className="kv facts">
              {facts.map((f) => (
                <div key={f.key} className="kv-row">
                  <dt>{f.label}</dt>
                  <dd>{f.display}</dd>
                </div>
              ))}
            </dl>
          )}
          {table && result.items.length > 0 && (
            <div className="check-table">
              <div className="check-table-label">{result.itemsLabel ?? "Details"}</div>
              <DataTable
                tableKey={`check:${result.id}:${mode}`}
                columns={table.columns}
                rows={table.rows}
                advanced={advanced}
                selectedId={selected?.id ?? null}
                onRowClick={(id) => setSelected((s) => (s?.id === id ? null : result.items.find((i) => i.id === id) ?? null))}
              />
            </div>
          )}
          {result.items.length === 0 && facts.length === 0 && !result.error && !result.limited && <div className="muted small">Nothing more to show.</div>}
        </div>
      )}
      {selected && <DetailPanel item={selected} mode={mode} onClose={() => setSelected(null)} floating />}
    </div>
  );
}
