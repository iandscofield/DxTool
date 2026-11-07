import { useMemo, useState } from "react";
import { Search, X } from "lucide-react";
import type { Item, Mode } from "../lib/types";
import { StatusBadge } from "./Status";

/** Card layout for categories whose items differ a lot from each other (e.g. System). */
export function CardGrid({
  items,
  mode,
  selectedId,
  onSelect,
  toolbarLeft,
}: {
  items: Item[];
  mode: Mode;
  selectedId: string | null;
  onSelect: (id: string) => void;
  toolbarLeft?: React.ReactNode;
}) {
  const [q, setQ] = useState("");
  const limit = mode === "advanced" ? 99 : 8;
  const list = useMemo(() => {
    if (!q) return items;
    const l = q.toLowerCase();
    return items.filter(
      (i) => i.name.toLowerCase().includes(l) || i.summary.some((f) => f.display.toLowerCase().includes(l) || f.label.toLowerCase().includes(l)),
    );
  }, [items, q]);

  return (
    <div className="cards-wrap">
      <div className="dt-toolbar plain">
        {toolbarLeft}
        <div className="dt-search">
          <Search size={15} />
          <input placeholder={`Search ${items.length} items…`} value={q} onChange={(e) => setQ(e.target.value)} />
          {q && (
            <button className="icon-btn ghost" onClick={() => setQ("")} aria-label="Clear search">
              <X size={14} />
            </button>
          )}
        </div>
      </div>
      <div className="card-grid">
        {list.map((it) => (
          <button key={it.id} className={`item-card ${selectedId === it.id ? "selected" : ""} st-${it.status}`} onClick={() => onSelect(it.id)}>
            <div className="item-card-head">
              <div className="item-card-title">
                <div className="item-card-sub">{it.subtitle}</div>
                <div className="item-card-name">{it.name}</div>
              </div>
              {it.status !== "ok" && <StatusBadge status={it.status} text={it.status === "info" ? "Note" : undefined} />}
            </div>
            {it.statusText && <div className={`item-card-note s-${it.status}`}>{it.statusText}</div>}
            <dl className="kv compact">
              {it.summary.slice(0, limit).map((f) => (
                <div className="kv-row" key={f.key}>
                  <dt>{f.label}</dt>
                  <dd>{f.display}</dd>
                </div>
              ))}
            </dl>
            {it.summary.length > limit && <div className="muted small more">+{it.summary.length - limit} more · click for everything</div>}
          </button>
        ))}
        {list.length === 0 && <div className="dt-empty">Nothing matches your search.</div>}
      </div>
    </div>
  );
}
