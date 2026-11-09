import {
  Bug,
  CircuitBoard,
  Cpu,
  FileText,
  Gamepad2,
  HeartPulse,
  History,
  LayoutDashboard,
  ScanSearch,
  Rocket,
  Settings,
  ShieldCheck,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import type { Page, ScanReport } from "../lib/types";

interface NavItem {
  id: Page;
  label: string;
  icon: LucideIcon;
  soon?: boolean;
}

const main: NavItem[] = [
  { id: "overview", label: "Overview", icon: LayoutDashboard },
  { id: "hardware", label: "Hardware", icon: Cpu },
  { id: "scans", label: "Scans", icon: ScanSearch },
  { id: "health", label: "Health", icon: HeartPulse },
  { id: "security", label: "Security", icon: ShieldCheck },
  { id: "virus", label: "Virus scan", icon: Bug },
];
const tools: NavItem[] = [
  { id: "upgrades", label: "Upgrades", icon: Rocket },
  { id: "games", label: "Games", icon: Gamepad2 },
  { id: "drivers", label: "Drivers", icon: CircuitBoard },
  { id: "history", label: "History", icon: History },
  { id: "reports", label: "Reports", icon: FileText },
  { id: "fixes", label: "Fixes", icon: Wrench },
];

export function Sidebar({
  page,
  onNavigate,
  version,
  scanning,
  report,
  fixing,
  virusRunning,
}: {
  page: Page;
  onNavigate: (p: Page) => void;
  version: string;
  scanning?: boolean;
  report?: ScanReport | null;
  fixing?: boolean;
  virusRunning?: boolean;
}) {
  const badge = (id: Page) => {
    if (id === "scans" && scanning) return <span className="nav-badge live">running</span>;
    if (id === "virus" && virusRunning) return <span className="nav-badge live">running</span>;
    if (id === "fixes" && fixing) return <span className="nav-badge live">running</span>;
    if (id === "health" && report) {
      const tone = report.score >= 90 ? "excellent" : report.score >= 75 ? "good" : report.score >= 50 ? "fair" : "poor";
      return <span className={`nav-badge tone-${tone}`}>{report.score}</span>;
    }
    return null;
  };
  const render = (n: NavItem) => (
    <button key={n.id} className={`nav-item ${page === n.id ? "active" : ""}`} onClick={() => onNavigate(n.id)}>
      <n.icon size={18} strokeWidth={1.8} />
      <span>{n.label}</span>
      {n.soon && <span className="soon">soon</span>}
      {badge(n.id)}
    </button>
  );
  return (
    <nav className="sidebar">
      <div className="brand">
        <img src="/logo.svg" alt="" />
        <div>
          <div className="brand-name">DxTool</div>
          <div className="brand-sub">PC diagnostics</div>
        </div>
      </div>
      <div className="nav-group">{main.map(render)}</div>
      <div className="nav-label">Tools</div>
      <div className="nav-group">{tools.map(render)}</div>
      <div className="nav-fill" />
      <div className="nav-group">
        {render({ id: "settings", label: "Settings", icon: Settings })}
      </div>
      <div className="version">v{version}</div>
    </nav>
  );
}
