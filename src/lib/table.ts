import type { TableColumn, TableRow } from "../components/DataTable";
import type { Item } from "./types";

/** Turns a list of items into table columns and rows. */
export function buildTable(items: Item[], advanced: boolean): { columns: TableColumn[]; rows: TableRow[] } {
  const summaryCols = new Map<string, string>();
  const rawCols = new Map<string, { header: string; group: string }>();
  for (const it of items) {
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
  const rows: TableRow[] = items.map((it) => {
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
