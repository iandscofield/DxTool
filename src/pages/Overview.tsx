import { useMemo } from "react";
import { AlertTriangle, ChevronRight, Clock, RefreshCw, ShieldCheck, Sparkles } from "lucide-react";
import type { Category, Inventory, Item } from "../lib/types";
import { CategoryIcon } from "../lib/icons";
import { StatusBadge } from "../components/Status";

function cat(inv: Inventory, id: string): Category | undefined {
  return inv.categories.find((c) => c.id === id);
}
function item(inv: Inventory, catId: string, itemId: string): Item | undefined {
  return cat(inv, catId)?.items.find((i) => i.id === itemId);
}
function field(it: Item | undefined, label: string): string | undefined {
  return it?.summary.find((f) => f.label === label)?.display;
}
function num(it: Item | undefined, label: string): number {
  const v = it?.summary.find((f) => f.label === label)?.sort;
  return typeof v === "number" ? v : 0;
}
function fmtBytesDec(b: number) {
  if (b >= 1e12) return `${(b / 1e12).toFixed(b >= 1e13 ? 0 : 2)} TB`;
  if (b >= 1e9) return `${(b / 1e9).toFixed(0)} GB`;
  return `${(b / 1e6).toFixed(0)} MB`;
}

interface Tile {
  cat: string;
  icon: string;
  label: string;
  value: string;
  detail: string;
}

export function Overview({
  inventory,
  onOpenCategory,
  onRefresh,
  refreshing,
}: {
  inventory: Inventory;
  onOpenCategory: (id: string) => void;
  onRefresh: () => void;
  refreshing: boolean;
}) {
  const inv = inventory;
  const computer = item(inv, "system", "computer");
  const windows = item(inv, "system", "windows");
  const firmware = item(inv, "system", "firmware");
  const board = item(inv, "system", "motherboard");

  const tiles = useMemo<Tile[]>(() => {
    const t: Tile[] = [];
    const cpu = cat(inv, "cpu")?.items[0];
    if (cpu)
      t.push({
        cat: "cpu",
        icon: "cpu",
        label: "Processor",
        value: cpu.name.replace(/\(R\)|\(TM\)/g, "").replace(/\s+/g, " "),
        detail: [field(cpu, "Cores") && `${field(cpu, "Cores")} cores`, field(cpu, "Threads") && `${field(cpu, "Threads")} threads`, field(cpu, "Base speed")]
          .filter(Boolean)
          .join(" · "),
      });
    const mem = cat(inv, "memory");
    const arr = mem?.items.find((i) => i.id === "memarray");
    const dimm = mem?.items.find((i) => i.id !== "memarray");
    if (mem)
      t.push({
        cat: "memory",
        icon: "memory-stick",
        label: "Memory",
        value: field(arr, "Installed") ?? field(computer, "Installed memory") ?? "—",
        detail: [field(dimm, "Type"), field(dimm, "Running at"), arr && `${field(arr, "Slots used")} of ${field(arr, "Total slots")} slots used`].filter(Boolean).join(" · "),
      });
    const gpus = [...(cat(inv, "graphics")?.items ?? [])].sort((a, b) => num(b, "Video memory") - num(a, "Video memory"));
    if (gpus[0])
      t.push({
        cat: "graphics",
        icon: "gpu",
        label: "Graphics",
        value: gpus[0].name,
        detail: [field(gpus[0], "Video memory"), field(gpus[0], "Driver version") && `driver ${field(gpus[0], "Driver version")}`].filter(Boolean).join(" · "),
      });
    const disks = cat(inv, "storage")?.items ?? [];
    if (disks.length) {
      const total = disks.reduce((s, d) => s + num(d, "Capacity"), 0);
      const bad = disks.filter((d) => d.status === "warn" || d.status === "error").length;
      t.push({
        cat: "storage",
        icon: "hard-drive",
        label: "Storage",
        value: `${fmtBytesDec(total)} total`,
        detail: `${disks.length} drive${disks.length > 1 ? "s" : ""} · ${bad ? `${bad} need attention` : "all report healthy"}`,
      });
    }
    const mons = cat(inv, "displays")?.items ?? [];
    if (mons.length)
      t.push({
        cat: "displays",
        icon: "screen",
        label: "Displays",
        value: `${mons.length} connected`,
        detail: mons.map((m) => `${m.name} (${field(m, "Connection") ?? "?"})`).join(" · "),
      });
    const net = (cat(inv, "network")?.items ?? []).filter((n) => field(n, "Status") === "Connected" && field(n, "Physical adapter") === "Yes");
    t.push({
      cat: "network",
      icon: "network",
      label: "Network",
      value: net[0] ? `${field(net[0], "Type")} · ${field(net[0], "Link speed") ?? ""}` : "Not connected",
      detail: net.map((n) => n.name).join(" · ") || "No connected physical adapter",
    });
    const bat = cat(inv, "battery")?.items[0];
    if (bat)
      t.push({ cat: "battery", icon: "battery", label: "Battery", value: `${field(bat, "Health") ?? "?"} health`, detail: [field(bat, "Charge cycles") && `${field(bat, "Charge cycles")} cycles`, field(bat, "Full charge capacity")].filter(Boolean).join(" · ") });
    if (board)
      t.push({ cat: "system", icon: "slots", label: "Motherboard", value: field(board, "Model") ?? board.name, detail: [field(board, "Manufacturer"), firmware && `BIOS ${field(firmware, "Version")}`].filter(Boolean).join(" · ") });
    return t;
  }, [inv, computer, board, firmware]);

  const attention = useMemo(() => {
    const seen = new Set<string>();
    const out: { item: Item; cat: Category }[] = [];
    for (const c of inv.categories) {
      for (const i of c.items) {
        if ((i.status === "warn" || i.status === "error") && !seen.has(i.id)) {
          seen.add(i.id);
          out.push({ item: i, cat: c });
        }
      }
    }
    return out.sort((a, b) => (a.item.status === "error" ? -1 : 0) - (b.item.status === "error" ? -1 : 0));
  }, [inv]);

  const deviceCount = cat(inv, "devices")?.items.length ?? 0;
  const disabled = cat(inv, "devices")?.items.filter((i) => i.status === "info").length ?? 0;

  return (
    <div className="page overview">
      <section className="hero">
        <div className="hero-main">
          <div className="eyebrow">This PC</div>
          <h1>{inv.computerName}</h1>
          <div className="hero-sub">{computer?.name}</div>
          <div className="hero-chips">
            {windows && (
              <span className="chip">
                {windows.name.replace("Microsoft ", "")} {field(windows, "Version")} · build {field(windows, "Build")}
              </span>
            )}
            {field(windows, "Uptime") && (
              <span className="chip">
                <Clock size={13} /> Up {field(windows, "Uptime")}
              </span>
            )}
            {firmware && (
              <span className="chip">
                <ShieldCheck size={13} /> {field(firmware, "Boot mode")} · Secure Boot {field(firmware, "Secure Boot")}
              </span>
            )}
          </div>
        </div>
        <div className="hero-side">
          <div className="scan-card">
            <Sparkles size={18} />
            <div>
              <div className="scan-title">Health scans are next</div>
              <div className="muted small">Fast, Full and Custom scans arrive in the next build stage.</div>
            </div>
          </div>
          <button className="btn" onClick={onRefresh} disabled={refreshing}>
            <RefreshCw size={15} className={refreshing ? "spin" : ""} /> {refreshing ? "Refreshing…" : "Refresh hardware"}
          </button>
          <div className="muted small">
            Read {deviceCount} devices in {(inv.elapsedMs / 1000).toFixed(1)} s · {new Date(inv.collectedAt).toLocaleTimeString()}
          </div>
        </div>
      </section>

      <section className="tiles">
        {tiles.map((t) => (
          <button key={t.label} className="tile" onClick={() => onOpenCategory(t.cat)}>
            <div className="tile-icon">
              <CategoryIcon name={t.icon} size={20} />
            </div>
            <div className="tile-label">{t.label}</div>
            <div className="tile-value" title={t.value}>
              {t.value}
            </div>
            <div className="tile-detail" title={t.detail}>
              {t.detail}
            </div>
          </button>
        ))}
      </section>

      <section className="card">
        <div className="card-head">
          <h2>
            <AlertTriangle size={17} /> Needs attention
          </h2>
          <span className="muted small">
            {attention.length === 0 ? "Nothing flagged in the hardware inventory" : `${attention.length} item${attention.length > 1 ? "s" : ""}`}
            {disabled > 0 && ` · ${disabled} disabled device${disabled > 1 ? "s" : ""} (not a problem)`}
          </span>
        </div>
        {attention.length === 0 ? (
          <div className="all-good">Everything the inventory can check looks fine. Deeper checks (drive wear, crashes, event logs) come with health scans.</div>
        ) : (
          <div className="attention-list">
            {attention.map(({ item: i, cat: c }) => (
              <button key={`${c.id}-${i.id}`} className="attention-row" onClick={() => onOpenCategory(c.id)}>
                <StatusBadge status={i.status} text={i.status === "error" ? "Problem" : "Warning"} />
                <div className="attention-text">
                  <div className="attention-name">{i.name}</div>
                  <div className="muted small">{i.statusText}</div>
                </div>
                <span className="muted small">{c.title}</span>
                <ChevronRight size={16} className="muted" />
              </button>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
