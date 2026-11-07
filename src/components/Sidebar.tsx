import {
  Cpu,
  FileText,
  HeartPulse,
  History,
  LayoutDashboard,
  ScanSearch,
  Settings,
  ShieldCheck,
  Wrench,
  type LucideIcon,
} from "lucide-react";
import type { Page } from "../lib/types";

interface NavItem {
  id: Page;
  label: string;
  icon: LucideIcon;
  soon?: boolean;
}

const main: NavItem[] = [
  { id: "overview", label: "Overview", icon: LayoutDashboard },
  { id: "hardware", label: "Hardware", icon: Cpu },
  { id: "scans", label: "Scans", icon: ScanSearch, soon: true },
  { id: "health", label: "Health", icon: HeartPulse, soon: true },
  { id: "security", label: "Security", icon: ShieldCheck, soon: true },
];
const tools: NavItem[] = [
  { id: "history", label: "History", icon: History, soon: true },
  { id: "reports", label: "Reports", icon: FileText, soon: true },
  { id: "fixes", label: "Fixes", icon: Wrench, soon: true },
];

export function Sidebar({ page, onNavigate, version }: { page: Page; onNavigate: (p: Page) => void; version: string }) {
  const render = (n: NavItem) => (
    <button key={n.id} className={`nav-item ${page === n.id ? "active" : ""}`} onClick={() => onNavigate(n.id)}>
      <n.icon size={18} strokeWidth={1.8} />
      <span>{n.label}</span>
      {n.soon && <span className="soon">soon</span>}
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
