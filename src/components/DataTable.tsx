import { useEffect, useMemo, useRef, useState } from "react";
import {
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  getSortedRowModel,
  useReactTable,
  type ColumnDef,
  type ColumnFiltersState,
  type ColumnOrderState,
  type ColumnSizingState,
  type Row as TRow,
  type SortingState,
  type VisibilityState,
} from "@tanstack/react-table";
import {
  ArrowDown,
  ArrowUp,
  ArrowUpDown,
  Check,
  ClipboardCopy,
  Columns3,
  Filter,
  GripVertical,
  Rows3,
  RotateCcw,
  Search,
  X,
} from "lucide-react";
import { copyText, loadPref, savePref } from "../lib/api";
import type { Status } from "../lib/types";
import { StatusDot } from "./Status";

export interface TableColumn {
  id: string;
  header: string;
  /** Group shown in the column picker, e.g. "Summary" or a WMI class name. */
  group: string;
  defaultVisible: boolean;
  /** Advanced-only columns are hidden from the picker in Simple mode. */
  advanced?: boolean;
}

export interface Cell {
  d: string;
  v: string | number | boolean | null;
}

export interface TableRow {
  id: string;
  status?: Status;
  statusText?: string | null;
  cells: Record<string, Cell>;
}

interface Props {
  tableKey: string;
  columns: TableColumn[];
  rows: TableRow[];
  advanced: boolean;
  selectedId?: string | null;
  onRowClick?: (id: string) => void;
  emptyText?: string;
  toolbarLeft?: React.ReactNode;
}

interface Saved {
  visibility?: VisibilityState;
  order?: ColumnOrderState;
  sizing?: ColumnSizingState;
  sorting?: SortingState;
  dense?: boolean;
}

function compareValues(a: Cell["v"] | undefined, b: Cell["v"] | undefined) {
  const an = a == null || a === "";
  const bn = b == null || b === "";
  if (an && bn) return 0;
  if (an) return 1;
  if (bn) return -1;
  if (typeof a === "number" && typeof b === "number") return a - b;
  return String(a).localeCompare(String(b), undefined, { numeric: true, sensitivity: "base" });
}

export function DataTable({ tableKey, columns, rows, advanced, selectedId, onRowClick, emptyText, toolbarLeft }: Props) {
  const saved = useMemo(() => loadPref<Saved>(`table:${tableKey}`, {}), [tableKey]);
  const defaultVisibility = useMemo(() => {
    const v: VisibilityState = {};
    for (const c of columns) v[c.id] = c.defaultVisible;
    return v;
  }, [columns]);

  const [sorting, setSorting] = useState<SortingState>(saved.sorting ?? []);
  const [visibility, setVisibility] = useState<VisibilityState>({ ...defaultVisibility, ...saved.visibility });
  const [order, setOrder] = useState<ColumnOrderState>(saved.order ?? []);
  const [sizing, setSizing] = useState<ColumnSizingState>(saved.sizing ?? {});
  const [dense, setDense] = useState<boolean>(saved.dense ?? false);
  const [globalFilter, setGlobalFilter] = useState("");
  const [columnFilters, setColumnFilters] = useState<ColumnFiltersState>([]);
  const [showFilters, setShowFilters] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const dragCol = useRef<string | null>(null);

  // Reset state when switching to a different table (e.g. another hardware category).
  useEffect(() => {
    const s = loadPref<Saved>(`table:${tableKey}`, {});
    setSorting(s.sorting ?? []);
    setVisibility({ ...defaultVisibility, ...s.visibility });
    setOrder(s.order ?? []);
    setSizing(s.sizing ?? {});
    setDense(s.dense ?? false);
    setGlobalFilter("");
    setColumnFilters([]);
  }, [tableKey, defaultVisibility]);

  useEffect(() => {
    savePref<Saved>(`table:${tableKey}`, { visibility, order, sizing, sorting, dense });
  }, [tableKey, visibility, order, sizing, sorting, dense]);

  const defs = useMemo<ColumnDef<TableRow>[]>(() => {
    const status: ColumnDef<TableRow> = {
      id: "__status",
      header: "",
      size: 34,
      enableResizing: false,
      enableHiding: false,
      accessorFn: (r) => ({ ok: 0, info: 1, warn: 2, error: 3 })[r.status ?? "ok"],
      cell: (c) => <StatusDot status={c.row.original.status ?? "ok"} title={c.row.original.statusText ?? undefined} />,
    };
    return [
      status,
      ...columns.map<ColumnDef<TableRow>>((c) => ({
        id: c.id,
        header: c.header,
        accessorFn: (r) => r.cells[c.id]?.v ?? null,
        cell: (info) => info.row.original.cells[c.id]?.d ?? "",
        sortingFn: (a: TRow<TableRow>, b: TRow<TableRow>, id: string) =>
          compareValues(a.original.cells[id]?.v, b.original.cells[id]?.v),
        filterFn: (row: TRow<TableRow>, id: string, val: string) =>
          (row.original.cells[id]?.d ?? "").toLowerCase().includes(String(val).toLowerCase()),
        size: c.id === "name" ? 280 : 170,
        minSize: 60,
      })),
    ];
  }, [columns]);

  const table = useReactTable({
    data: rows,
    columns: defs,
    getRowId: (r) => r.id,
    state: { sorting, columnVisibility: visibility, columnOrder: order, columnSizing: sizing, globalFilter, columnFilters },
    onSortingChange: setSorting,
    onColumnVisibilityChange: setVisibility,
    onColumnOrderChange: setOrder,
    onColumnSizingChange: setSizing,
    onGlobalFilterChange: setGlobalFilter,
    onColumnFiltersChange: setColumnFilters,
    globalFilterFn: (row, _id, val: string) => {
      const q = String(val).toLowerCase();
      return Object.values(row.original.cells).some((c) => c.d.toLowerCase().includes(q));
    },
    columnResizeMode: "onChange",
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
  });

  const visibleRows = table.getRowModel().rows;
  const leafs = table.getVisibleLeafColumns();

  const onDrop = (target: string) => {
    const src = dragCol.current;
    dragCol.current = null;
    if (!src || src === target) return;
    const current = table.getAllLeafColumns().map((c) => c.id);
    const base = order.length ? [...order, ...current.filter((id) => !order.includes(id))] : current;
    const from = base.indexOf(src);
    const to = base.indexOf(target);
    if (from < 0 || to < 0) return;
    base.splice(to, 0, base.splice(from, 1)[0]);
    setOrder(base);
  };

  const copyTable = async () => {
    const cols = leafs.filter((c) => c.id !== "__status");
    const lines = [
      cols.map((c) => String(c.columnDef.header ?? c.id)).join("\t"),
      ...visibleRows.map((r) => cols.map((c) => (r.original.cells[c.id]?.d ?? "").replace(/\s+/g, " ")).join("\t")),
    ];
    if (await copyText(lines.join("\n"))) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    }
  };

  const resetLayout = () => {
    setVisibility(defaultVisibility);
    setOrder([]);
    setSizing({});
    setSorting([]);
  };

  return (
    <div className={`dt ${dense ? "dt-dense" : ""}`}>
      <div className="dt-toolbar">
        {toolbarLeft}
        <div className="dt-search">
          <Search size={15} />
          <input
            placeholder={`Search ${rows.length} rows…`}
            value={globalFilter}
            onChange={(e) => setGlobalFilter(e.target.value)}
          />
          {globalFilter && (
            <button className="icon-btn ghost" onClick={() => setGlobalFilter("")} aria-label="Clear search">
              <X size={14} />
            </button>
          )}
        </div>
        <div className="dt-spacer" />
        <span className="dt-count">
          {visibleRows.length === rows.length ? `${rows.length} rows` : `${visibleRows.length} of ${rows.length}`}
        </span>
        <button className={`btn ghost ${showFilters ? "active" : ""}`} onClick={() => setShowFilters((s) => !s)} title="Filter by column">
          <Filter size={15} /> Filters
          {columnFilters.length > 0 && <span className="pill-count">{columnFilters.length}</span>}
        </button>
        <div className="popover-anchor">
          <button className={`btn ghost ${pickerOpen ? "active" : ""}`} onClick={() => setPickerOpen((o) => !o)}>
            <Columns3 size={15} /> Columns
          </button>
          {pickerOpen && (
            <ColumnPicker
              columns={columns.filter((c) => advanced || !c.advanced)}
              visibility={visibility}
              onToggle={(id, v) => setVisibility((s) => ({ ...s, [id]: v }))}
              onReset={resetLayout}
              onClose={() => setPickerOpen(false)}
              advanced={advanced}
            />
          )}
        </div>
        <button className="icon-btn ghost" onClick={() => setDense((d) => !d)} title={dense ? "Comfortable rows" : "Compact rows"}>
          <Rows3 size={16} />
        </button>
        <button className="icon-btn ghost" onClick={copyTable} title="Copy table (paste into Excel or a document)">
          {copied ? <Check size={16} /> : <ClipboardCopy size={16} />}
        </button>
      </div>

      <div className="dt-scroll">
        <table style={{ width: table.getTotalSize() }}>
          <thead>
            {table.getHeaderGroups().map((hg) => (
              <tr key={hg.id}>
                {hg.headers.map((h) => {
                  const sorted = h.column.getIsSorted();
                  const isStatus = h.column.id === "__status";
                  return (
                    <th
                      key={h.id}
                      style={{ width: h.getSize() }}
                      draggable={!isStatus}
                      onDragStart={() => (dragCol.current = h.column.id)}
                      onDragOver={(e) => e.preventDefault()}
                      onDrop={() => onDrop(h.column.id)}
                    >
                      {!isStatus && (
                        <div className="th-inner" onClick={h.column.getToggleSortingHandler()} title="Click to sort, Shift+click to sort by several columns, drag to move">
                          <GripVertical size={12} className="th-grip" />
                          <span className="th-label">{flexRender(h.column.columnDef.header, h.getContext())}</span>
                          {sorted === "asc" ? (
                            <ArrowUp size={13} className="th-sort on" />
                          ) : sorted === "desc" ? (
                            <ArrowDown size={13} className="th-sort on" />
                          ) : (
                            <ArrowUpDown size={13} className="th-sort" />
                          )}
                        </div>
                      )}
                      {h.column.getCanResize() && (
                        <div
                          className={`th-resize ${h.column.getIsResizing() ? "on" : ""}`}
                          onMouseDown={h.getResizeHandler()}
                          onDoubleClick={() => h.column.resetSize()}
                          onClick={(e) => e.stopPropagation()}
                        />
                      )}
                    </th>
                  );
                })}
              </tr>
            ))}
            {showFilters && (
              <tr className="filter-row">
                {leafs.map((c) => (
                  <th key={c.id}>
                    {c.id !== "__status" && (
                      <input
                        className="col-filter"
                        placeholder="Filter…"
                        value={(c.getFilterValue() as string) ?? ""}
                        onChange={(e) => c.setFilterValue(e.target.value || undefined)}
                      />
                    )}
                  </th>
                ))}
              </tr>
            )}
          </thead>
          <tbody>
            {visibleRows.map((r) => (
              <tr
                key={r.id}
                className={`${selectedId === r.id ? "selected" : ""} ${onRowClick ? "clickable" : ""} st-${r.original.status ?? "ok"}`}
                onClick={() => onRowClick?.(r.id)}
              >
                {r.getVisibleCells().map((c) => (
                  <td key={c.id} style={{ width: c.column.getSize() }} title={c.column.id === "__status" ? undefined : r.original.cells[c.column.id]?.d}>
                    {flexRender(c.column.columnDef.cell, c.getContext())}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
        {visibleRows.length === 0 && <div className="dt-empty">{rows.length === 0 ? emptyText ?? "Nothing to show." : "No rows match your search or filters."}</div>}
      </div>
    </div>
  );
}

function ColumnPicker({
  columns,
  visibility,
  onToggle,
  onReset,
  onClose,
  advanced,
}: {
  columns: TableColumn[];
  visibility: VisibilityState;
  onToggle: (id: string, v: boolean) => void;
  onReset: () => void;
  onClose: () => void;
  advanced: boolean;
}) {
  const [q, setQ] = useState("");
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const h = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    setTimeout(() => document.addEventListener("mousedown", h), 0);
    return () => document.removeEventListener("mousedown", h);
  }, [onClose]);

  const groups = useMemo(() => {
    const m = new Map<string, TableColumn[]>();
    const ql = q.toLowerCase();
    for (const c of columns) {
      if (ql && !c.header.toLowerCase().includes(ql) && !c.group.toLowerCase().includes(ql)) continue;
      if (!m.has(c.group)) m.set(c.group, []);
      m.get(c.group)!.push(c);
    }
    return [...m.entries()];
  }, [columns, q]);

  return (
    <div className="popover column-picker" ref={ref}>
      <div className="popover-head">
        <div className="dt-search small">
          <Search size={14} />
          <input autoFocus placeholder="Find a column…" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
        <button className="btn ghost small" onClick={onReset} title="Restore default columns, order and widths">
          <RotateCcw size={13} /> Reset
        </button>
      </div>
      {!advanced && <div className="picker-hint">Switch to Advanced mode to add every raw property as a column.</div>}
      <div className="picker-list">
        {groups.map(([g, cols]) => (
          <div key={g} className="picker-group">
            <div className="picker-group-title">
              <span>{g}</span>
              <span className="picker-group-actions">
                <button onClick={() => cols.forEach((c) => onToggle(c.id, true))}>all</button>
                <button onClick={() => cols.forEach((c) => onToggle(c.id, false))}>none</button>
              </span>
            </div>
            {cols.map((c) => (
              <label key={c.id} className="picker-item">
                <input type="checkbox" checked={visibility[c.id] ?? c.defaultVisible} onChange={(e) => onToggle(c.id, e.target.checked)} />
                <span>{c.header}</span>
              </label>
            ))}
          </div>
        ))}
        {groups.length === 0 && <div className="dt-empty small">No matching columns.</div>}
      </div>
    </div>
  );
}
