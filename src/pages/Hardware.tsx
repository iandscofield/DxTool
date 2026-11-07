import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, LayoutGrid, Table2 } from "lucide-react";
import { CardGrid } from "../components/CardGrid";
import type { Inventory, Mode } from "../lib/types";
import { buildTable } from "../lib/table";
import { CategoryIcon } from "../lib/icons";
import { DataTable } from "../components/DataTable";
import { DetailPanel } from "../components/DetailPanel";
import { loadPref, savePref } from "../lib/api";

/** Categories that read better as cards by default. Everything else starts as a table. */
const CARD_DEFAULT = new Set(["system", "cpu", "graphics", "displays", "battery"]);

export function Hardware({ inventory, mode, initialCategory }: { inventory: Inventory; mode: Mode; initialCategory?: string }) {
  const cats = inventory.categories;
  const [catId, setCatId] = useState<string>(initialCategory ?? loadPref("hw:cat", cats[0]?.id ?? "system"));
  const [selected, setSelected] = useState<string | null>(null);
  const advanced = mode === "advanced";

  useEffect(() => {
    if (initialCategory) setCatId(initialCategory);
  }, [initialCategory]);
  useEffect(() => savePref("hw:cat", catId), [catId]);
  useEffect(() => setSelected(null), [catId]);

  const cat = cats.find((c) => c.id === catId) ?? cats[0];
  const { columns, rows } = useMemo(() => buildTable(cat.items, advanced), [cat, advanced]);
  const item = cat.items.find((i) => i.id === selected) ?? null;

  const defaultView = CARD_DEFAULT.has(cat.id) ? "cards" : "table";
  const [views, setViews] = useState<Record<string, "cards" | "table">>(loadPref("hw:views", {}));
  const view = views[cat.id] ?? defaultView;
  const setView = (v: "cards" | "table") => {
    const next = { ...views, [cat.id]: v };
    setViews(next);
    savePref("hw:views", next);
  };
  const viewToggle = (
    <div className="segmented" title="Switch between table and card layouts">
      <button className={view === "table" ? "on" : ""} onClick={() => setView("table")}>
        <Table2 size={14} /> Table
      </button>
      <button className={view === "cards" ? "on" : ""} onClick={() => setView("cards")}>
        <LayoutGrid size={14} /> Cards
      </button>
    </div>
  );
  const toggleSelect = (id: string) => setSelected((s) => (s === id ? null : id));

  return (
    <div className="hw">
      <div className="hw-rail">
        {cats.map((c) => {
          const problems = c.items.filter((i) => i.status === "warn" || i.status === "error").length;
          return (
            <button key={c.id} className={`rail-item ${c.id === cat.id ? "active" : ""}`} onClick={() => setCatId(c.id)} title={c.description}>
              <CategoryIcon name={c.icon} />
              <span className="rail-label">{c.title}</span>
              {problems > 0 ? <span className="rail-count warn">{problems}</span> : <span className="rail-count">{c.items.length}</span>}
            </button>
          );
        })}
      </div>

      <div className="hw-main">
        <div className="hw-head">
          <div>
            <h1>
              <CategoryIcon name={cat.icon} size={20} /> {cat.title}
            </h1>
            <p className="muted">{cat.description}</p>
          </div>
        </div>
        {cat.error && (
          <div className="banner warn">
            <AlertTriangle size={16} />
            <span>Some information could not be read: {cat.error}</span>
          </div>
        )}
        {view === "cards" && cat.items.length > 0 ? (
          <CardGrid items={cat.items} mode={mode} selectedId={selected} onSelect={toggleSelect} toolbarLeft={viewToggle} />
        ) : (
          <DataTable
            tableKey={`hw:${cat.id}:${mode}`}
            columns={columns}
            rows={rows}
            advanced={advanced}
            selectedId={selected}
            onRowClick={toggleSelect}
            toolbarLeft={viewToggle}
            emptyText={cat.id === "battery" ? "No battery found. This looks like a desktop PC." : "Nothing found in this category."}
          />
        )}
      </div>

      {item && <DetailPanel item={item} mode={mode} onClose={() => setSelected(null)} />}
    </div>
  );
}
