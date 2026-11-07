import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, LayoutGrid, Table2 } from "lucide-react";
import { CardGrid } from "../components/CardGrid";
import type { Category, Inventory, Mode } from "../lib/types";
import { CategoryIcon } from "../lib/icons";
import { DataTable, type TableColumn, type TableRow } from "../components/DataTable";
import { DetailPanel } from "../components/DetailPanel";
import { loadPref, savePref } from "../lib/api";

/** Categories that read better as cards by default. Everything else starts as a table. */
const CARD_DEFAULT = new Set(["system", "cpu", "graphics", "displays", "battery"]);

/** Turns a category's items into table columns and rows. */
function buildTable(cat: Category, advanced: boolean): { columns: TableColumn[]; rows: TableRow[] } {
  const summaryCols = new Map<string, string>();
  const rawCols = new Map<string, { header: string; group: string }>();
  for (const it of cat.items) {
    for (const f of it.summary) if (!summaryCols.has(f.key)) summaryCols.set(f.key, f.label);
    for (const s of it.sections) {
      for (const f of s.fields) {
        const key = s.raw ? f.key : `${s.title}.${f.key}`;
        if (!rawCols.has(key) && !summaryCols.has(f.key)) rawCols.set(key, { header: f.label, group: s.title });
      }
    }
  }
  const simpleLimit = 6;
  const columns: TableColumn[] = [
    { id: "name", header: "Name", group: "Summary", defaultVisible: true },
    ...[...summaryCols.entries()]
      .filter(([k]) => k !== "Name" && k !== "Model")
      .map(([key, label], i) => ({
        id: `s:${key}`,
        header: label,
        group: "Summary",
        defaultVisible: i < (advanced ? 10 : simpleLimit),
      })),
    ...[...rawCols.entries()].map(([key, v]) => ({
      id: `r:${key}`,
      header: v.header,
      group: v.group,
      defaultVisible: false,
      advanced: true,
    })),
  ];
  const rows: TableRow[] = cat.items.map((it) => {
    const cells: TableRow["cells"] = { name: { d: it.name, v: it.name.toLowerCase() } };
    for (const f of it.summary) cells[`s:${f.key}`] = { d: f.display, v: f.sort };
    for (const s of it.sections) {
      for (const f of s.fields) {
        const key = s.raw ? f.key : `${s.title}.${f.key}`;
        cells[`r:${key}`] = { d: f.display, v: f.sort };
      }
    }
    return { id: it.id, status: it.status, statusText: it.statusText, cells };
  });
  return { columns, rows };
}

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
  const { columns, rows } = useMemo(() => buildTable(cat, advanced), [cat, advanced]);
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
