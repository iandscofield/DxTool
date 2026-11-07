import { useEffect, useMemo, useState } from "react";
import { ArrowRight, Check, FileText, FolderOpen, GitCompare, History as HistoryIcon, Monitor, Trash2, X } from "lucide-react";
import type { Finding, MachineRecord, ScanReport, ScanSummary, Status } from "../lib/types";
import { api } from "../lib/api";
import { ScoreRing } from "../components/ScoreRing";
import { StatusBadge, StatusDot } from "../components/Status";

const kindName: Record<string, string> = { fast: "Fast", full: "Full", custom: "Custom" };
const fmtDate = (s: string) => new Date(s).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });

/** Small line chart of scores over time, oldest on the left. */
function Trend({ scans }: { scans: ScanSummary[] }) {
  const pts = [...scans].reverse();
  if (pts.length < 2) return <div className="muted small">Run more scans to see a trend.</div>;
  const w = 320;
  const h = 64;
  const x = (i: number) => 6 + (i * (w - 12)) / (pts.length - 1);
  const y = (s: number) => h - 6 - (s / 100) * (h - 12);
  const line = pts.map((p, i) => `${x(i).toFixed(1)},${y(p.score).toFixed(1)}`).join(" ");
  return (
    <svg className="trend" viewBox={`0 0 ${w} ${h}`} width="100%" height={h} preserveAspectRatio="none" role="img" aria-label="Health score over time">
      <line x1="0" x2={w} y1={y(90)} y2={y(90)} className="trend-guide" />
      <line x1="0" x2={w} y1={y(75)} y2={y(75)} className="trend-guide" />
      <polyline points={line} className="trend-line" fill="none" />
      {pts.map((p, i) => (
        <circle key={p.id} cx={x(i)} cy={y(p.score)} r="3.5" className="trend-dot">
          <title>
            {fmtDate(p.startedAt)}: {p.score}
          </title>
        </circle>
      ))}
    </svg>
  );
}

interface Diff {
  a: ScanReport;
  b: ScanReport;
  added: { f: Finding; check: string }[];
  fixed: { f: Finding; check: string }[];
  same: { f: Finding; check: string }[];
  changed: { title: string; from: Status; to: Status; fromText: string; toText: string }[];
}

function diff(a: ScanReport, b: ScanReport): Diff {
  const key = (check: string, f: Finding) => `${check}:${f.id}`;
  const list = (r: ScanReport) => r.results.flatMap((c) => c.findings.filter((f) => f.severity !== "info").map((f) => ({ f, check: c.title, k: key(c.id, f) })));
  const la = list(a);
  const lb = list(b);
  const ka = new Set(la.map((x) => x.k));
  const kb = new Set(lb.map((x) => x.k));
  const changed = b.results
    .map((cb) => {
      const ca = a.results.find((c) => c.id === cb.id);
      return ca && ca.status !== cb.status ? { title: cb.title, from: ca.status, to: cb.status, fromText: ca.summary, toText: cb.summary } : null;
    })
    .filter((x): x is NonNullable<typeof x> => !!x);
  return {
    a,
    b,
    added: lb.filter((x) => !ka.has(x.k)),
    fixed: la.filter((x) => !kb.has(x.k)),
    same: lb.filter((x) => ka.has(x.k)),
    changed,
  };
}

function CompareView({ d, onClose }: { d: Diff; onClose: () => void }) {
  const delta = d.b.score - d.a.score;
  const Section = ({ title, items, empty, cls }: { title: string; items: Diff["added"]; empty: string; cls: string }) => (
    <div className={`cmp-col ${cls}`}>
      <div className="cmp-title">
        {title} <span>{items.length}</span>
      </div>
      {items.length === 0 ? (
        <div className="muted small">{empty}</div>
      ) : (
        items.map((x) => (
          <div key={`${x.check}-${x.f.id}`} className="cmp-item">
            <StatusDot status={x.f.severity} />
            <div>
              <div>{x.f.title}</div>
              <div className="muted small">{x.check}</div>
            </div>
          </div>
        ))
      )}
    </div>
  );
  return (
    <section className="card compare">
      <div className="card-head">
        <h2>
          <GitCompare size={17} /> Comparing two scans
        </h2>
        <button className="icon-btn ghost" onClick={onClose} title="Close comparison">
          <X size={16} />
        </button>
      </div>
      <div className="cmp-head">
        <div className="cmp-scan">
          <ScoreRing score={d.a.score} size={56} stroke={6} />
          <div>
            <div className="muted small">Before</div>
            <div>{fmtDate(d.a.startedAt)}</div>
            <div className="muted small">{kindName[d.a.kind]} scan</div>
          </div>
        </div>
        <div className={`cmp-delta ${delta > 0 ? "up" : delta < 0 ? "down" : ""}`}>
          <ArrowRight size={18} />
          {delta === 0 ? "No change" : `${delta > 0 ? "+" : ""}${delta} points`}
        </div>
        <div className="cmp-scan">
          <ScoreRing score={d.b.score} size={56} stroke={6} />
          <div>
            <div className="muted small">After</div>
            <div>{fmtDate(d.b.startedAt)}</div>
            <div className="muted small">{kindName[d.b.kind]} scan</div>
          </div>
        </div>
      </div>
      {d.a.kind !== d.b.kind && <div className="muted small cmp-note">These scans are different types, so some differences may just be checks that only one of them ran.</div>}
      <div className="cmp-cols">
        <Section title="New since before" items={d.added} empty="Nothing new." cls="new" />
        <Section title="Fixed" items={d.fixed} empty="Nothing fixed." cls="fixed" />
        <Section title="Still there" items={d.same} empty="Nothing carried over." cls="same" />
      </div>
      {d.changed.length > 0 && (
        <>
          <div className="cmp-title">Checks that changed</div>
          <div className="cmp-changes">
            {d.changed.map((c) => (
              <div key={c.title} className="cmp-change">
                <span className="cmp-check">{c.title}</span>
                <StatusBadge status={c.from} />
                <ArrowRight size={14} className="muted" />
                <StatusBadge status={c.to} />
                <span className="muted small">{c.toText}</span>
              </div>
            ))}
          </div>
        </>
      )}
      <div className="muted small cmp-note">Notes (blue) are left out of the comparison.</div>
    </section>
  );
}

export function History({
  machines,
  currentMachineId,
  onRefresh,
  onOpen,
  onReport,
}: {
  machines: MachineRecord[];
  currentMachineId: string | null;
  onRefresh: () => void;
  onOpen: (r: ScanReport) => void;
  onReport: (r: ScanReport) => void;
}) {
  const [selId, setSelId] = useState<string | null>(null);
  const [picked, setPicked] = useState<string[]>([]);
  const [compare, setCompare] = useState<Diff | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [dataDir, setDataDir] = useState("");

  useEffect(() => {
    api.dataDir().then(setDataDir).catch(() => {});
  }, []);
  const machine = useMemo(
    () => machines.find((m) => m.info.id === selId) ?? machines.find((m) => m.info.id === currentMachineId) ?? machines[0],
    [machines, selId, currentMachineId],
  );
  useEffect(() => {
    setPicked([]);
    setCompare(null);
  }, [machine?.info.id]);

  const load = async (scanId: string) => api.loadScan(machine!.info.id, scanId);
  const run = async (fn: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const togglePick = (id: string) =>
    setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id].slice(-2)));
  const doCompare = () =>
    run(async () => {
      const ids = [...picked].sort((x, y) => {
        const sx = machine!.scans.find((s) => s.id === x)!.startedAt;
        const sy = machine!.scans.find((s) => s.id === y)!.startedAt;
        return sx.localeCompare(sy);
      });
      const [a, b] = await Promise.all(ids.map(load));
      setCompare(diff(a, b));
    });

  if (machines.length === 0) {
    return (
      <div className="page">
        <div className="empty-state card">
          <div className="placeholder-icon">
            <HistoryIcon size={28} />
          </div>
          <h1>No saved scans yet</h1>
          <p className="muted">Every scan is saved here automatically, grouped by PC. Run a scan from the Scans page to start the history.</p>
          {dataDir && <p className="muted small">Saved in {dataDir}</p>}
        </div>
      </div>
    );
  }

  const m = machine!;
  return (
    <div className="page history">
      <div className="page-head">
        <div>
          <h1>
            <HistoryIcon size={22} /> History
          </h1>
          <p className="muted">Every scan is saved automatically, per PC. Run DxTool from a USB stick to keep the history of all your PCs in one place.</p>
        </div>
        {dataDir && (
          <button className="btn ghost small" onClick={() => api.openPath(dataDir)} title={dataDir}>
            <FolderOpen size={14} /> Open data folder
          </button>
        )}
      </div>
      {error && <div className="banner error">{error}</div>}

      <div className="history-layout">
        <div className="machine-list">
          {machines.map((x) => {
            const last = x.scans[0];
            return (
              <button key={x.info.id} className={`machine-card ${x.info.id === m.info.id ? "active" : ""}`} onClick={() => setSelId(x.info.id)}>
                <Monitor size={18} />
                <div className="machine-text">
                  <div className="machine-name">
                    {x.info.computerName}
                    {x.info.id === currentMachineId && <span className="tag">this PC</span>}
                  </div>
                  <div className="muted small">{[x.info.manufacturer, x.info.model].filter(Boolean).join(" ")}</div>
                  <div className="muted small">
                    {x.scans.length} scan{x.scans.length === 1 ? "" : "s"} · last {last ? new Date(last.startedAt).toLocaleDateString() : "never"}
                  </div>
                </div>
                {last && <ScoreRing score={last.score} size={38} stroke={4} />}
              </button>
            );
          })}
        </div>

        <div className="machine-detail">
          <section className="card">
            <div className="machine-head">
              <div>
                <h2>{m.info.computerName}</h2>
                <div className="muted small">
                  {[m.info.manufacturer, m.info.model].filter(Boolean).join(" ")}
                  {m.info.serial && ` · serial ${m.info.serial}`}
                </div>
                <div className="muted small">{[m.info.windows, m.info.cpu, m.info.memory].filter(Boolean).join(" · ")}</div>
                <div className="muted small">First scanned {fmtDate(m.firstSeen)}</div>
              </div>
              <div className="trend-box">
                <div className="muted small">Health score over time</div>
                <Trend scans={m.scans} />
              </div>
            </div>
          </section>

          {compare && <CompareView d={compare} onClose={() => setCompare(null)} />}

          <section className="card">
            <div className="card-head">
              <h2>Scans</h2>
              <div className="row-gap">
                <span className="muted small">{picked.length === 0 ? "Tick two scans to compare them" : picked.length === 1 ? "Tick one more to compare" : "Ready to compare"}</span>
                <button className="btn small" disabled={picked.length !== 2 || busy} onClick={doCompare}>
                  <GitCompare size={13} /> Compare
                </button>
              </div>
            </div>
            <div className="table-scroll">
            <table className="simple-table">
              <thead>
                <tr>
                  <th />
                  <th>When</th>
                  <th>Type</th>
                  <th>Score</th>
                  <th>Problems</th>
                  <th>Warnings</th>
                  <th>Notes</th>
                  <th>Admin</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {m.scans.map((s) => (
                  <tr key={s.id} className={picked.includes(s.id) ? "picked" : ""}>
                    <td>
                      <input type="checkbox" checked={picked.includes(s.id)} onChange={() => togglePick(s.id)} aria-label="Pick for comparison" />
                    </td>
                    <td>{fmtDate(s.startedAt)}</td>
                    <td>{kindName[s.kind] ?? s.kind}</td>
                    <td>
                      <span className={`score-pill tone-${s.score >= 90 ? "excellent" : s.score >= 75 ? "good" : s.score >= 50 ? "fair" : "poor"}`}>{s.score}</span> {s.grade}
                    </td>
                    <td className={s.counts.error ? "c-err" : "muted"}>{s.counts.error}</td>
                    <td className={s.counts.warn ? "c-warn" : "muted"}>{s.counts.warn}</td>
                    <td className="muted">{s.counts.info}</td>
                    <td>{s.isAdmin ? <Check size={14} /> : <span className="muted">—</span>}</td>
                    <td className="row-actions">
                      {confirmDelete === s.id ? (
                        <>
                          <span className="small">Delete this scan for good?</span>
                          <button
                            className="btn small danger"
                            onClick={() =>
                              run(async () => {
                                await api.deleteScan(m.info.id, s.id);
                                setConfirmDelete(null);
                                setPicked((p) => p.filter((x) => x !== s.id));
                                onRefresh();
                              })
                            }
                          >
                            Delete
                          </button>
                          <button className="btn small ghost" onClick={() => setConfirmDelete(null)}>
                            Keep
                          </button>
                        </>
                      ) : (
                        <>
                          <button className="btn small ghost" disabled={busy} onClick={() => run(async () => onOpen(await load(s.id)))}>
                            Open
                          </button>
                          <button className="btn small ghost" disabled={busy} onClick={() => run(async () => onReport(await load(s.id)))} title="Make a report from this scan">
                            <FileText size={13} /> Report
                          </button>
                          <button className="icon-btn ghost" onClick={() => setConfirmDelete(s.id)} title="Delete this scan">
                            <Trash2 size={14} />
                          </button>
                        </>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}
