import { useEffect, useState } from "react";
import { Battery, CheckCircle2, Cpu, Gauge, HardDrive, Loader2, MemoryStick, Monitor, RefreshCw, Rocket, type LucideIcon } from "lucide-react";
import { api } from "../lib/api";
import type { Upgrade, UpgradeReport } from "../lib/types";

const areaIcon: Record<string, LucideIcon> = {
  Memory: MemoryStick,
  Storage: HardDrive,
  Windows: Monitor,
  Graphics: Gauge,
  Battery: Battery,
  Processor: Cpu,
};

const priorityText: Record<string, string> = { high: "Do this first", medium: "Worth doing", low: "Optional" };

function UpgradeCard({ u }: { u: Upgrade }) {
  const Icon = areaIcon[u.area] ?? Rocket;
  return (
    <article className={`upgrade prio-${u.priority}`}>
      <div className="upgrade-icon">
        <Icon size={20} />
      </div>
      <div className="upgrade-main">
        <div className="upgrade-tags">
          <span className={`badge prio-${u.priority}`}>{priorityText[u.priority] ?? u.priority}</span>
          <span className="badge">{u.area}</span>
          <span className={`badge cost ${u.cost === "Free" ? "free" : ""}`}>{u.cost}</span>
        </div>
        <h3>{u.title}</h3>
        <p className="upgrade-why">{u.why}</p>
        <p className="upgrade-how">
          <strong>What to do: </strong>
          {u.how}
        </p>
        {u.search && <p className="muted small">Part to look for: {u.search}</p>}
      </div>
    </article>
  );
}

/** Upgrade recommendations worked out from this PC's hardware (offline). */
export function Upgrades() {
  const [report, setReport] = useState<UpgradeReport | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const load = async (refresh: boolean) => {
    setLoading(true);
    setError(null);
    try {
      setReport(await api.upgradeAdvice(refresh));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => {
    load(false);
  }, []);

  return (
    <div className="page upgrades">
      <div className="page-head">
        <div>
          <h1>
            <Rocket size={22} /> Upgrades
          </h1>
          <p className="muted">What would make this PC faster, safer or last longer, worked out from its hardware. Free fixes come first where they matter as much.</p>
        </div>
        <button className="btn ghost" onClick={() => load(true)} disabled={loading}>
          {loading ? <Loader2 size={14} className="spin" /> : <RefreshCw size={14} />} Check again
        </button>
      </div>

      {error && <div className="banner error">{error}</div>}

      {loading && !report ? (
        <section className="card empty-state">
          <Loader2 size={28} className="spin" />
          <p>Reading the hardware…</p>
        </section>
      ) : report ? (
        <>
          {report.limited && <div className="banner warn">{report.limited}</div>}
          <section className="card">
            <div className="card-head">
              <h2>Recommendations</h2>
              <span className="muted small">
                {report.pcName} · {report.pcKind}
              </span>
            </div>
            {report.items.length === 0 ? (
              <p className="update-ok">
                <CheckCircle2 size={16} /> Nothing needs upgrading. This PC is well equipped for what it is.
              </p>
            ) : (
              <div className="upgrade-list">
                {report.items.map((u) => (
                  <UpgradeCard key={u.id} u={u} />
                ))}
              </div>
            )}
          </section>
          {report.good.length > 0 && (
            <section className="card">
              <div className="card-head">
                <h2>Already good</h2>
              </div>
              <ul className="good-list">
                {report.good.map((g) => (
                  <li key={g}>
                    <CheckCircle2 size={15} /> {g}
                  </li>
                ))}
              </ul>
            </section>
          )}
          <p className="muted small">Costs are rough: Free, Low cost (a small part), Moderate, Expensive (a major part or a new PC). Prices change, so compare a few shops before buying.</p>
        </>
      ) : null}
    </div>
  );
}
