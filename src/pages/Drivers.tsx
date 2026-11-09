import { useEffect, useState } from "react";
import { Bluetooth, CheckCircle2, CircleAlert, CircleHelp, CircuitBoard, CloudDownload, ExternalLink, Loader2, Monitor, Network, RefreshCw, Wifi, Wrench, type LucideIcon } from "lucide-react";
import { api } from "../lib/api";
import type { DriverCheck, DriverState, DriverStatus } from "../lib/types";

const kindIcon: Record<string, LucideIcon> = { Graphics: Monitor, "Wi-Fi": Wifi, Bluetooth: Bluetooth, Ethernet: Network };

const stateText: Record<DriverState, string> = {
  update: "Update available",
  current: "Up to date",
  newer: "Newer than the maker's",
  unknown: "Check yourself",
};

function StateBadge({ s }: { s: DriverState }) {
  const Icon = s === "update" ? CircleAlert : s === "unknown" ? CircleHelp : CheckCircle2;
  const cls = s === "update" ? "v-minimum" : s === "unknown" ? "" : "v-recommended";
  return (
    <span className={`badge verdict ${cls}`}>
      <Icon size={13} /> {stateText[s]}
    </span>
  );
}

const size = (b: number) => (b >= 1e9 ? `${(b / 1e9).toFixed(1)} GB` : `${Math.round(b / 1e6)} MB`);

function DriverCard({ d }: { d: DriverStatus }) {
  const Icon = kindIcon[d.kind] ?? CircuitBoard;
  const prio = d.status === "update" ? "high" : d.status === "unknown" ? "medium" : "low";
  return (
    <article className={`upgrade prio-${prio}`}>
      <div className="upgrade-icon">
        <Icon size={20} />
      </div>
      <div className="upgrade-main">
        <div className="upgrade-tags">
          <StateBadge s={d.status} />
          <span className="badge">{d.vendor}</span>
          <span className="badge">{d.kind}</span>
        </div>
        <h3>{d.device}</h3>
        <p className="upgrade-why">{d.summary}</p>
        <table className="simple-table driver-versions">
          <tbody>
            <tr>
              <th>This PC</th>
              <td>
                {d.installedVersion || "—"} {d.installedDate && <span className="muted small">({d.installedDate})</span>}
              </td>
            </tr>
            {d.latestVersion && (
              <tr>
                <th>Newest</th>
                <td>
                  {d.latestVersion} {d.latestDate && <span className="muted small">({d.latestDate})</span>}
                  {d.latestName && <div className="muted small">{d.latestName}</div>}
                </td>
              </tr>
            )}
          </tbody>
        </table>
        <div className="row-gap">
          {d.status === "update" && d.downloadUrl && (
            <button className="btn small" onClick={() => api.openPath(d.downloadUrl)}>
              <CloudDownload size={13} /> Download {d.downloadSize ? `(${size(d.downloadSize)})` : ""}
            </button>
          )}
          {d.detailsUrl && (
            <button className="btn ghost small" onClick={() => api.openPath(d.detailsUrl)}>
              <ExternalLink size={13} /> {d.vendor === "AMD" ? "AMD driver page" : "Driver page"}
            </button>
          )}
        </div>
        {d.note && <p className="muted small">{d.note}</p>}
      </div>
    </article>
  );
}

/** Newest drivers: graphics, Wi-Fi, Bluetooth and Ethernet compared with the makers' newest (online, on click). */
export function Drivers({ onWindowsUpdate }: { onWindowsUpdate: () => void }) {
  const [check, setCheck] = useState<DriverCheck | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.driversLast().then((c) => c && setCheck(c)).catch(() => {});
  }, []);

  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      setCheck(await api.driversCheck());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const updates = check?.items.filter((i) => i.status === "update").length ?? 0;

  return (
    <div className="page drivers">
      <div className="page-head">
        <div>
          <h1>
            <CircuitBoard size={22} /> Drivers
          </h1>
          <p className="muted">Are the graphics, Wi-Fi, Bluetooth and network drivers the newest? DxTool asks NVIDIA and Intel directly and points to AMD's page. Everything else comes from Windows Update.</p>
        </div>
        <button className="btn" onClick={run} disabled={busy}>
          {busy ? <Loader2 size={14} className="spin" /> : <RefreshCw size={14} />} {check ? "Check again" : "Check for newer drivers"}
        </button>
      </div>

      {error && <div className="banner error">{error}</div>}
      {check?.problems.map((p) => (
        <div key={p} className="banner warn">
          {p}
        </div>
      ))}

      {!check ? (
        <section className="card empty-state">
          {busy ? (
            <p className="muted">
              <Loader2 size={14} className="spin" /> Asking NVIDIA and Intel for their newest drivers…
            </p>
          ) : (
            <>
              <p>Click <b>Check for newer drivers</b> to compare this PC's drivers with the newest versions from the makers.</p>
              <p className="muted small">This uses the internet: DxTool sends the device IDs of the graphics card (to NVIDIA) and downloads Intel's public driver list. Nothing is installed without you.</p>
            </>
          )}
        </section>
      ) : (
        <section className="card">
          <div className="card-head">
            <h2>
              {check.nothingToCheck ? "Nothing to look up" : updates > 0 ? `${updates} driver update${updates > 1 ? "s" : ""} available` : "Drivers are up to date"}
            </h2>
            <span className="muted small">Checked {new Date(check.checkedAt).toLocaleString()}</span>
          </div>
          {check.nothingToCheck ? (
            <p className="muted">This PC has no NVIDIA, Intel or AMD graphics, Wi-Fi, Bluetooth or Ethernet devices that DxTool can look up. Windows Update covers its drivers (below).</p>
          ) : (
            <div className="upgrade-list">
              {check.items.map((d) => (
                <DriverCard key={`${d.vendor}-${d.device}`} d={d} />
              ))}
            </div>
          )}
          {updates > 0 && (
            <p className="muted small">
              To update: download the file, close games and other programs, run it and follow its steps. The screen may go black or flicker for a moment while a graphics driver installs. Restart if it asks. If something goes wrong, the old driver can be rolled back in Device Manager.
            </p>
          )}
        </section>
      )}

      <section className="card">
        <div className="card-head">
          <h2>Other drivers</h2>
        </div>
        <p className="muted">Chipset, audio, card readers, printers and most other devices get their drivers through Windows Update. DxTool can check Windows Update and install the ones you pick.</p>
        <button className="btn ghost" onClick={onWindowsUpdate}>
          <Wrench size={14} /> Check Windows Update for drivers
        </button>
      </section>
    </div>
  );
}
