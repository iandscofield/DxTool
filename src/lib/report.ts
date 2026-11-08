// Builds a self-contained HTML report from a scan. The same HTML is shown in the
// preview, saved as an .html file, and printed to PDF by Edge.

import type { Branding, CheckResult, Finding, FixRun, ScanReport, Status } from "./types";

export interface ReportOptions {
  title: string;
  preparedFor: string;
  notes: string;
  includeComputer: boolean;
  includeNotes: boolean;
  includeChecks: boolean;
  includeTechnical: boolean;
  includeRepairs: boolean;
  showLogo: boolean;
  showCompany: boolean;
  showContact: boolean;
  showTech: boolean;
}

export const defaultReportOptions: ReportOptions = {
  title: "PC Health Report",
  preparedFor: "",
  notes: "",
  includeComputer: true,
  includeNotes: true,
  includeChecks: true,
  includeTechnical: false,
  includeRepairs: true,
  showLogo: true,
  showCompany: true,
  showContact: true,
  showTech: true,
};

const esc = (s: unknown) =>
  String(s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");

const sevLabel: Record<Status, string> = { error: "Problem", warn: "Warning", info: "Note", ok: "OK" };
const kindName: Record<string, string> = { fast: "Fast scan", full: "Full scan", custom: "Custom scan" };
const tone = (score: number) => (score >= 90 ? "#16a34a" : score >= 75 ? "#65a30d" : score >= 50 ? "#d97706" : "#dc2626");
const order: Record<Status, number> = { error: 0, warn: 1, info: 2, ok: 3 };

export function verdict(score: number) {
  if (score >= 90) return "This PC is in great shape.";
  if (score >= 75) return "This PC is in good shape, with a few things worth a look.";
  if (score >= 50) return "This PC has some problems that should be looked at.";
  return "This PC has serious problems that need attention.";
}

function ring(score: number) {
  const r = 52;
  const c = 2 * Math.PI * r;
  const off = c * (1 - Math.max(0, Math.min(100, score)) / 100);
  return `<svg width="128" height="128" viewBox="0 0 128 128" aria-hidden="true">
    <circle cx="64" cy="64" r="${r}" fill="none" stroke="#e5e7eb" stroke-width="11"/>
    <circle cx="64" cy="64" r="${r}" fill="none" stroke="${tone(score)}" stroke-width="11" stroke-linecap="round"
      stroke-dasharray="${c.toFixed(2)}" stroke-dashoffset="${off.toFixed(2)}" transform="rotate(-90 64 64)"/>
    <text x="64" y="72" text-anchor="middle" font-size="34" font-weight="700" fill="#111827">${score}</text>
  </svg>`;
}

function findingBlock(f: Finding, check: CheckResult, withEvidence: boolean) {
  const ev = withEvidence ? f.evidence.filter((e) => e.display) : [];
  return `<div class="finding ${f.severity}">
    <div class="f-head"><span class="badge ${f.severity}">${sevLabel[f.severity]}</span><span class="f-title">${esc(f.title)}</span><span class="f-area">${esc(check.title)}</span></div>
    <p>${esc(f.detail)}</p>
    ${f.advice ? `<p class="advice"><b>What to do:</b> ${esc(f.advice)}</p>` : ""}
    ${ev.length ? `<table class="kv">${ev.map((e) => `<tr><th>${esc(e.label)}</th><td>${esc(e.display)}</td></tr>`).join("")}</table>` : ""}
  </div>`;
}

function itemTable(check: CheckResult) {
  if (!check.items.length) return "";
  const cols: string[] = [];
  for (const it of check.items) for (const f of it.summary) if (!cols.includes(f.label) && cols.length < 6) cols.push(f.label);
  const rows = check.items.slice(0, 20);
  const more = check.items.length - rows.length;
  return `<div class="tbl-label">${esc(check.itemsLabel ?? "Details")} (${check.items.length})</div>
  <table class="grid"><thead><tr><th>Name</th>${cols.map((c) => `<th>${esc(c)}</th>`).join("")}</tr></thead><tbody>
  ${rows
    .map(
      (it) =>
        `<tr class="${it.status}"><td>${esc(it.name)}</td>${cols
          .map((c) => {
            const full = it.summary.find((f) => f.label === c)?.display ?? "";
            const v = full.length > 140 ? `${full.slice(0, 137)}…` : full;
            return `<td${v.length <= 20 ? ' class="nw"' : ""}>${esc(v)}</td>`;
          })
          .join("")}</tr>`,
    )
    .join("")}
  </tbody></table>${more > 0 ? `<div class="more">…and ${more} more, available in DxTool.</div>` : ""}`;
}

export function buildReportHtml(r: ScanReport, o: ReportOptions, b: Branding, repairs: FixRun[] = []): string {
  const m = r.machine;
  const when = new Date(r.startedAt);
  const dateText = when.toLocaleString([], { dateStyle: "long", timeStyle: "short" });
  const all = r.results
    .flatMap((check) => check.findings.map((finding) => ({ finding, check })))
    .sort((a, b2) => order[a.finding.severity] - order[b2.finding.severity]);
  const important = all.filter((x) => x.finding.severity === "error" || x.finding.severity === "warn");
  const notes = all.filter((x) => x.finding.severity === "info");
  const limited = r.results.filter((x) => x.limited);
  const stepLabel: Record<string, string> = { ok: "Done", warning: "Done with warnings", failed: "Failed", skipped: "Skipped" };
  const stepTone: Record<string, Status> = { ok: "ok", warning: "warn", failed: "error", skipped: "info" };
  const repairRuns = o.includeRepairs ? repairs.filter((f) => f.startedAt >= r.startedAt).sort((a, b2) => a.startedAt.localeCompare(b2.startedAt)) : [];
  const repairsHtml = repairRuns.length
    ? `<h2>Repairs carried out</h2>${repairRuns
        .map(
          (f) => `<h3>${esc(new Date(f.startedAt).toLocaleString([], { dateStyle: "medium", timeStyle: "short" }))}</h3>
      <table class="grid"><thead><tr><th>Repair</th><th>Result</th><th>Details</th></tr></thead><tbody>
      ${[...(f.restorePoint ? [f.restorePoint] : []), ...f.steps]
        .map((s) => `<tr><td><span class="dot ${stepTone[s.status] ?? "info"}"></span>${esc(s.title)}</td><td class="nw">${esc(stepLabel[s.status] ?? s.status)}</td><td>${esc(s.summary)}</td></tr>`)
        .join("")}
      </tbody></table>${f.restorePointSkipped ? `<p class="partial">Run without a restore point, at the user's request.</p>` : ""}`,
        )
        .join("")}`
    : "";

  const logo = o.showLogo && b.logo ? `<img class="logo" src="${b.logo}" alt="">` : "";
  const company = o.showCompany && b.company.trim() ? `<div class="company">${esc(b.company)}</div>` : "";
  const contact = o.showContact && b.contact.trim() ? `<div class="contact">${esc(b.contact)}</div>` : "";
  const tech = o.showTech && b.tech.trim() ? `<div><span>Technician</span> ${esc(b.tech)}</div>` : "";
  const hasBrand = logo || company || contact;

  const chips = [
    r.counts.error && `<span class="chip error">${r.counts.error} problem${r.counts.error > 1 ? "s" : ""}</span>`,
    r.counts.warn && `<span class="chip warn">${r.counts.warn} warning${r.counts.warn > 1 ? "s" : ""}</span>`,
    r.counts.info && `<span class="chip info">${r.counts.info} note${r.counts.info > 1 ? "s" : ""}</span>`,
    r.counts.error + r.counts.warn === 0 && `<span class="chip ok">No problems found</span>`,
  ]
    .filter(Boolean)
    .join("");

  const machineRows: [string, string][] = [
    ["Computer name", m.computerName],
    ["Make and model", [m.manufacturer, m.model].filter(Boolean).join(" ")],
    ["Serial number", m.serial],
    ["Windows", m.windows],
    ["Processor", m.cpu],
    ["Memory", m.memory],
    ["Graphics", m.graphics.join(", ")],
    ["Drives", m.drives.join("; ")],
    ["Motherboard", m.motherboard],
    ["BIOS", m.bios],
  ];

  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(o.title)} · ${esc(m.computerName)}</title>
<style>
  @page { margin: 14mm 12mm; }
  * { box-sizing: border-box; }
  html { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
  body { margin: 0; background: #f3f4f6; color: #111827; font: 13.5px/1.5 "Segoe UI", system-ui, -apple-system, sans-serif; }
  .page { max-width: 880px; margin: 0 auto; background: #fff; padding: 36px 44px 28px; }
  @media print { body { background: #fff; } .page { padding: 0; max-width: none; } }
  @media (max-width: 640px) { .page { padding: 20px 16px; } .summary { flex-direction: column; text-align: center; } }
  header.brand { display: flex; align-items: center; gap: 16px; padding-bottom: 16px; border-bottom: 2px solid #e5e7eb; margin-bottom: 22px; }
  .logo { max-height: 56px; max-width: 200px; object-fit: contain; }
  .company { font-size: 18px; font-weight: 600; }
  .contact { color: #4b5563; font-size: 12.5px; }
  .meta { margin-left: auto; text-align: right; font-size: 12.5px; color: #374151; }
  .meta span { color: #6b7280; }
  h1 { font-size: 26px; margin: 0 0 2px; letter-spacing: -0.01em; }
  h2 { font-size: 16px; margin: 28px 0 10px; padding-bottom: 6px; border-bottom: 1px solid #e5e7eb; break-after: avoid; }
  h3 { font-size: 14px; margin: 18px 0 6px; break-after: avoid; }
  .sub { color: #4b5563; margin-bottom: 18px; }
  .summary { display: flex; align-items: center; gap: 22px; padding: 18px 20px; border: 1px solid #e5e7eb; border-radius: 12px; background: #f9fafb; break-inside: avoid; }
  .grade { font-size: 13px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.08em; }
  .verdict { font-size: 19px; font-weight: 600; margin: 2px 0 10px; }
  .chip { display: inline-block; padding: 2px 10px; border-radius: 999px; font-size: 12px; font-weight: 600; margin-right: 6px; border: 1px solid; }
  .chip.error { color: #b91c1c; background: #fef2f2; border-color: #fecaca; }
  .chip.warn { color: #b45309; background: #fffbeb; border-color: #fde68a; }
  .chip.info { color: #1d4ed8; background: #eff6ff; border-color: #bfdbfe; }
  .chip.ok { color: #15803d; background: #f0fdf4; border-color: #bbf7d0; }
  .finding { border: 1px solid #e5e7eb; border-left: 4px solid #60a5fa; border-radius: 8px; padding: 10px 14px; margin-bottom: 8px; break-inside: avoid; }
  .finding.warn { border-left-color: #f59e0b; }
  .finding.error { border-left-color: #ef4444; }
  .finding p { margin: 6px 0 0; color: #374151; }
  .f-head { display: flex; align-items: center; gap: 10px; }
  .f-title { font-weight: 600; flex: 1; }
  .f-area { font-size: 11px; color: #6b7280; text-transform: uppercase; letter-spacing: 0.05em; }
  .badge { font-size: 11px; font-weight: 700; padding: 1px 8px; border-radius: 999px; text-transform: uppercase; letter-spacing: 0.04em; }
  .badge.error { background: #fee2e2; color: #b91c1c; }
  .badge.warn { background: #fef3c7; color: #b45309; }
  .badge.info { background: #dbeafe; color: #1d4ed8; }
  .badge.ok { background: #dcfce7; color: #15803d; }
  .advice { background: #eff6ff; border-radius: 6px; padding: 7px 10px; }
  .good { padding: 12px 14px; border-radius: 8px; background: #f0fdf4; border: 1px solid #bbf7d0; color: #166534; }
  table { border-collapse: collapse; width: 100%; }
  table.kv th { text-align: left; font-weight: 500; color: #6b7280; width: 32%; padding: 5px 10px 5px 0; vertical-align: top; }
  table.kv td { padding: 5px 0; }
  table.kv tr + tr th, table.kv tr + tr td { border-top: 1px dashed #e5e7eb; }
  .finding table.kv { margin-top: 8px; font-size: 12px; }
  table.grid { font-size: 11.5px; margin-bottom: 4px; }
  table.grid th { text-align: left; background: #f3f4f6; padding: 5px 7px; font-weight: 600; border-bottom: 1px solid #d1d5db; }
  table.grid td { padding: 4px 7px; border-bottom: 1px solid #f0f0f0; vertical-align: top; overflow-wrap: anywhere; }
  table.grid td.nw { white-space: nowrap; overflow-wrap: normal; }
  table.grid th { white-space: nowrap; }
  table.grid tr { break-inside: avoid; }
  table.grid tr.error td:first-child { color: #b91c1c; font-weight: 600; }
  table.grid tr.warn td:first-child { color: #b45309; font-weight: 600; }
  .dot { display: inline-block; width: 9px; height: 9px; border-radius: 50%; margin-right: 7px; }
  .dot.ok { background: #22c55e; } .dot.info { background: #60a5fa; } .dot.warn { background: #f59e0b; } .dot.error { background: #ef4444; }
  .notes { white-space: pre-wrap; padding: 12px 14px; border-radius: 8px; background: #fafaf9; border: 1px solid #e7e5e4; }
  .tbl-label { font-size: 12px; font-weight: 600; color: #374151; margin: 10px 0 4px; }
  .more, .partial { font-size: 12px; color: #6b7280; }
  footer { margin-top: 30px; padding-top: 12px; border-top: 1px solid #e5e7eb; font-size: 11.5px; color: #6b7280; }
</style></head>
<body><div class="page">
${
  hasBrand || tech
    ? `<header class="brand">${logo}<div>${company}${contact}</div><div class="meta"><div><span>Date</span> ${esc(dateText)}</div>${tech}</div></header>`
    : ""
}
<h1>${esc(o.title)}</h1>
<div class="sub">${esc(m.computerName)}${m.model ? ` · ${esc([m.manufacturer, m.model].filter(Boolean).join(" "))}` : ""}${
    o.preparedFor.trim() ? ` · Prepared for <b>${esc(o.preparedFor)}</b>` : ""
  }${hasBrand || tech ? "" : ` · ${esc(dateText)}`}</div>

<section class="summary">
  ${ring(r.score)}
  <div>
    <div class="grade" style="color:${tone(r.score)}">Health score: ${esc(r.grade)}</div>
    <div class="verdict">${esc(verdict(r.score))}</div>
    <div>${chips}</div>
  </div>
</section>

<h2>What needs attention</h2>
${
  important.length
    ? important.map((x) => findingBlock(x.finding, x.check, false)).join("")
    : `<div class="good">No problems were found. Everything that was checked looks healthy.</div>`
}

${repairsHtml}

${o.notes.trim() ? `<h2>Technician notes</h2><div class="notes">${esc(o.notes)}</div>` : ""}

${
  o.includeComputer
    ? `<h2>About this computer</h2><table class="kv">${machineRows
        .filter(([, v]) => v)
        .map(([k, v]) => `<tr><th>${esc(k)}</th><td>${esc(v)}</td></tr>`)
        .join("")}</table>`
    : ""
}

${
  o.includeNotes && notes.length
    ? `<h2>For your information</h2><p class="more">These are not problems and don't lower the score.</p>${notes.map((x) => findingBlock(x.finding, x.check, false)).join("")}`
    : ""
}

${
  o.includeChecks
    ? `<h2>What was checked</h2><table class="grid"><thead><tr><th>Area</th><th>Check</th><th>Result</th></tr></thead><tbody>${r.results
        .map(
          (c) =>
            `<tr><td>${esc(c.group)}</td><td><span class="dot ${c.status}"></span>${esc(c.title)}</td><td>${esc(c.summary)}${
              c.limited ? ' <span class="partial">(partial: needs administrator rights)</span>' : ""
            }</td></tr>`,
        )
        .join("")}</tbody></table>`
    : ""
}

${
  o.includeTechnical
    ? `<h2>Technical details</h2>${important.length ? `<h3>Findings with evidence</h3>${important.map((x) => findingBlock(x.finding, x.check, true)).join("")}` : ""}${r.results
        .map((c) => {
          const facts = c.facts.filter((f) => f.display);
          if (!facts.length && !c.items.length && !c.error) return "";
          return `<div class="check"><h3><span class="dot ${c.status}"></span>${esc(c.group)} · ${esc(c.title)}</h3>
          ${c.error ? `<p class="partial">Could not complete: ${esc(c.error)}</p>` : ""}
          ${c.limited ? `<p class="partial">${esc(c.limited)}</p>` : ""}
          ${facts.length ? `<table class="kv">${facts.map((f) => `<tr><th>${esc(f.label)}</th><td>${esc(f.display)}</td></tr>`).join("")}</table>` : ""}
          ${itemTable(c)}</div>`;
        })
        .join("")}`
    : ""
}

<footer>
  Generated by DxTool ${esc(r.appVersion)} · ${esc(kindName[r.kind] ?? r.kind)} of ${r.results.length} checks on ${esc(dateText)} · took ${(r.elapsedMs / 1000).toFixed(1)} s ·
  ${r.isAdmin ? "run with administrator rights" : "run without administrator rights"}${
    limited.length ? ` (partial results for ${limited.map((l) => esc(l.title)).join(", ")})` : ""
  } · scan ${esc(r.id)} · machine ${esc(m.id)}
</footer>
</div></body></html>`;
}

/** A sensible file name, e.g. "PC Health Report - SPARTAN - 2026-11-07". */
export function reportFileName(r: ScanReport, o: ReportOptions) {
  const d = new Date(r.startedAt);
  const ymd = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const hm = `${String(d.getHours()).padStart(2, "0")}${String(d.getMinutes()).padStart(2, "0")}`;
  return `${o.title || "PC Health Report"} - ${r.machine.computerName} - ${ymd} ${hm}`;
}
