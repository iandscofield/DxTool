import { useMemo, useState } from "react";
import { Check, ChevronRight, Copy, Search, X } from "lucide-react";
import type { Item, Mode, Section } from "../lib/types";
import { copyText } from "../lib/api";
import { StatusBadge } from "./Status";

export function DetailPanel({ item, mode, onClose, floating }: { item: Item; mode: Mode; onClose: () => void; floating?: boolean }) {
  const [q, setQ] = useState("");
  const [copied, setCopied] = useState(false);
  const advanced = mode === "advanced";
  const sections = useMemo(() => item.sections.filter((s) => advanced || !s.raw), [item, advanced]);

  const copyAll = async () => {
    const lines = [item.name, item.subtitle, ""];
    for (const f of item.summary) lines.push(`${f.label}: ${f.display}`);
    for (const s of sections) {
      lines.push("", `[${s.title}]`);
      for (const f of s.fields) lines.push(`${f.label}: ${f.display}`);
    }
    if (await copyText(lines.join("\n"))) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    }
  };

  return (
    <aside className={`detail ${floating ? "floating" : ""}`}>
      <header className="detail-head">
        <div className="detail-title">
          <h2 title={item.name}>{item.name}</h2>
          {item.subtitle && <div className="muted">{item.subtitle}</div>}
          <div className="detail-badges">
            <StatusBadge status={item.status} text={item.statusText} />
          </div>
        </div>
        <div className="detail-actions">
          <button className="icon-btn ghost" onClick={copyAll} title="Copy all details">
            {copied ? <Check size={16} /> : <Copy size={16} />}
          </button>
          <button className="icon-btn ghost" onClick={onClose} title="Close">
            <X size={16} />
          </button>
        </div>
      </header>

      {advanced && (
        <div className="dt-search detail-search">
          <Search size={14} />
          <input placeholder="Search properties…" value={q} onChange={(e) => setQ(e.target.value)} />
        </div>
      )}

      <div className="detail-body">
        <KV fields={item.summary} q={q} />
        {sections.map((s, i) => (
          <SectionBlock key={`${s.title}-${i}`} section={s} q={q} defaultOpen={!s.raw || !!q} />
        ))}
        {!advanced && item.sections.some((s) => s.raw) && (
          <div className="hint-card">Switch to <b>Advanced</b> mode to see every raw property Windows reports for this item.</div>
        )}
      </div>
    </aside>
  );
}

function SectionBlock({ section, q, defaultOpen }: { section: Section; q: string; defaultOpen: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  const isOpen = open || !!q;
  const matches = filterFields(section.fields, q);
  if (q && matches.length === 0) return null;
  return (
    <div className={`section ${section.raw ? "raw" : ""}`}>
      <button className="section-head" onClick={() => setOpen((o) => !o)}>
        <ChevronRight size={14} className={`chev ${isOpen ? "open" : ""}`} />
        <span>{section.title}</span>
        {section.raw && <span className="tag">raw</span>}
        <span className="muted small">{section.fields.length}</span>
      </button>
      {isOpen && <KV fields={section.fields} q={q} mono={section.raw} />}
    </div>
  );
}

function filterFields(fields: Item["summary"], q: string) {
  if (!q) return fields;
  const l = q.toLowerCase();
  return fields.filter((f) => f.label.toLowerCase().includes(l) || f.display.toLowerCase().includes(l));
}

function KV({ fields, q, mono }: { fields: Item["summary"]; q: string; mono?: boolean }) {
  const list = filterFields(fields, q);
  if (list.length === 0) return null;
  return (
    <dl className={`kv ${mono ? "mono" : ""}`}>
      {list.map((f) => (
        <div key={f.key} className="kv-row" onDoubleClick={() => copyText(f.display)} title="Double-click to copy">
          <dt>{f.label}</dt>
          <dd>{f.display}</dd>
        </div>
      ))}
    </dl>
  );
}
