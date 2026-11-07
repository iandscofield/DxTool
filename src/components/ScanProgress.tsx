import { CheckCircle2, Circle, Loader2 } from "lucide-react";
import type { CheckInfo, ScanState } from "../lib/types";
import { StatusBadge } from "./Status";

const kindName = { fast: "Fast scan", full: "Full scan", custom: "Custom scan" };

/** Live list of checks while a scan runs. */
export function ScanProgress({ scan, catalog }: { scan: ScanState; catalog: CheckInfo[] }) {
  const total = scan.order.length;
  const done = scan.order.filter((id) => scan.progress[id]?.state === "done").length;
  const pct = total ? Math.round((done / total) * 100) : 0;
  const title = (id: string) => catalog.find((c) => c.id === id)?.title ?? id;
  return (
    <section className="card scan-progress">
      <div className="card-head">
        <h2>
          <Loader2 size={17} className="spin" /> {kindName[scan.kind]} running…
        </h2>
        <span className="muted small">
          {done} of {total || "…"} checks done
        </span>
      </div>
      <div className="progress-bar" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
        <span style={{ width: `${pct}%` }} />
      </div>
      <ul className="progress-list">
        {scan.order.map((id) => {
          const p = scan.progress[id];
          return (
            <li key={id} className={`progress-row ${p?.state ?? "waiting"}`}>
              {p?.state === "done" ? <CheckCircle2 size={16} className="ok-ic" /> : p?.state === "running" ? <Loader2 size={16} className="spin" /> : <Circle size={16} className="muted" />}
              <span className="progress-name">{title(id)}</span>
              <span className="muted small progress-summary">{p?.summary ?? (p?.state === "running" ? "Checking…" : "Waiting")}</span>
              {p?.state === "done" && p.status && p.status !== "ok" && <StatusBadge status={p.status} />}
            </li>
          );
        })}
      </ul>
    </section>
  );
}
