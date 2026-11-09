import { useEffect, useState } from "react";
import { Battery, CheckCircle2, ExternalLink, Tag, Cpu, Gauge, HardDrive, Loader2, MemoryStick, Monitor, RefreshCw, Rocket, type LucideIcon } from "lucide-react";
import { api } from "../lib/api";
import { shopLinks } from "../lib/shops";
import type { PartPrice, Upgrade, UpgradeReport } from "../lib/types";

/** A row of shop search buttons for one search phrase. */
function ShopButtons({ search }: { search: string }) {
  return (
    <div className="shop-buttons">
      {shopLinks(search).map((l) => (
        <button key={l.shop} className="btn ghost small" onClick={() => api.openPath(l.url)} title={`Search ${l.shop} for "${search}"`}>
          {l.shop} <ExternalLink size={11} />
        </button>
      ))}
    </div>
  );
}

function PartsCard({ parts }: { parts: PartPrice[] }) {
  return (
    <section className="card">
      <div className="card-head">
        <h2>
          <Tag size={16} /> Prices for this PC's parts
        </h2>
      </div>
      <p className="muted">What would each part cost today, new or used? Each button opens that shop's search in the browser.</p>
      <div className="game-table-wrap">
        <table className="simple-table parts-table">
          <tbody>
            {parts.map((p) => (
              <tr key={`${p.part}-${p.search}`}>
                <th>{p.part}</th>
                <td>
                  {p.name}
                  {p.note && <div className="muted small">{p.note}</div>}
                </td>
                <td className="right">
                  <ShopButtons search={p.search} />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="muted small">DxTool doesn't read or store prices: it only opens the searches. PCPartPicker compares many shops at once; eBay shows used prices, handy for older parts. Prices are for the US stores.</p>
    </section>
  );
}

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
        {u.search && (
          <div className="upgrade-shop">
            <span className="muted small">Find prices for “{u.search}”:</span>
            <ShopButtons search={u.search} />
          </div>
        )}
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
          {report.parts && report.parts.length > 0 && <PartsCard parts={report.parts} />}
          <p className="muted small">Costs are rough: Free, Low cost (a small part), Moderate, Expensive (a major part or a new PC). Prices change, so compare a few shops before buying.</p>
        </>
      ) : null}
    </div>
  );
}
