import { useEffect, useMemo, useState } from "react";
import { FileDown, FileText, FolderOpen, Loader2, Settings as SettingsIcon } from "lucide-react";
import type { AppSettings, FixRun, MachineRecord, ScanKind, ScanReport, VirusScan } from "../lib/types";
import { api, inTauri, loadPref, savePref } from "../lib/api";
import { buildReportHtml, defaultReportOptions, reportFileName, type ReportOptions } from "../lib/report";
import { NoScanYet } from "./Health";

const kindName: Record<string, string> = { fast: "Fast", full: "Full", custom: "Custom" };

export function Reports({
  report,
  machines,
  settings,
  onPickScan,
  onRun,
  onOpenSettings,
}: {
  report: ScanReport | null;
  machines: MachineRecord[];
  settings: AppSettings;
  onPickScan: (r: ScanReport) => void;
  onRun: (kind: ScanKind) => void;
  onOpenSettings: () => void;
}) {
  const b = settings.branding;
  // Branding switches start from Settings; the rest is remembered between reports.
  const [opts, setOpts] = useState<ReportOptions>(() => ({
    ...defaultReportOptions,
    ...loadPref<Partial<ReportOptions>>("report:opts", {}),
    preparedFor: "",
    notes: "",
    showLogo: b.showLogo,
    showCompany: b.showCompany,
    showContact: b.showContact,
    showTech: b.showTech,
  }));
  const [saving, setSaving] = useState<"html" | "pdf" | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loadingScan, setLoadingScan] = useState(false);
  const [repairs, setRepairs] = useState<FixRun[]>([]);
  useEffect(() => {
    if (report?.machine?.id) api.listFixRuns(report.machine.id).then(setRepairs).catch(() => setRepairs([]));
  }, [report?.machine?.id]);
  const laterRepairs = report ? repairs.filter((f) => f.startedAt >= report.startedAt).length : 0;
  const [virus, setVirus] = useState<VirusScan[]>([]);
  useEffect(() => {
    if (report?.machine?.id) api.virusScans(report.machine.id).then(setVirus).catch(() => setVirus([]));
  }, [report?.machine?.id]);
  const laterVirus = report ? virus.filter((v) => v.startedAt >= report.startedAt).length : 0;

  useEffect(() => {
    const { preparedFor: _p, notes: _n, ...keep } = opts;
    savePref("report:opts", keep);
  }, [opts]);
  useEffect(() => {
    setSaved(null);
    setError(null);
  }, [report?.id]);

  const html = useMemo(() => (report ? buildReportHtml(report, opts, b, repairs, virus) : ""), [report, opts, b, repairs, virus]);
  const set = <K extends keyof ReportOptions>(k: K, v: ReportOptions[K]) => setOpts((o) => ({ ...o, [k]: v }));

  if (!report) {
    return (
      <div className="page">
        <NoScanYet onRun={onRun} text="Reports are made from a scan. Run a scan, or open one from History, then come back here." />
      </div>
    );
  }

  const save = async (format: "html" | "pdf") => {
    setSaving(format);
    setError(null);
    setSaved(null);
    try {
      setSaved(await api.exportReport(report.machine.id, reportFileName(report, opts), html, format));
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(null);
    }
  };

  const pick = async (value: string) => {
    const [mid, sid] = value.split("|");
    if (sid === report.id && mid === report.machine.id) return;
    setLoadingScan(true);
    try {
      onPickScan(await api.loadScan(mid, sid));
    } catch (e) {
      setError(String(e));
    } finally {
      setLoadingScan(false);
    }
  };

  const Toggle = ({ k, label, hint, disabled }: { k: keyof ReportOptions; label: string; hint?: string; disabled?: boolean }) => (
    <label className={`opt-toggle ${disabled ? "disabled" : ""}`} title={hint}>
      <input type="checkbox" checked={!!opts[k]} disabled={disabled} onChange={(e) => set(k, e.target.checked as never)} />
      <span>{label}</span>
    </label>
  );
  const noBrand = !b.logo && !b.company.trim() && !b.tech.trim() && !b.contact.trim();
  const inList = machines.some((m) => m.info.id === report.machine.id && m.scans.some((s) => s.id === report.id));

  return (
    <div className="reports">
      <aside className="report-options">
        <h1>
          <FileText size={20} /> Report
        </h1>

        <div className="opt-group">
          <div className="opt-label">Scan</div>
          <select value={`${report.machine.id}|${report.id}`} onChange={(e) => pick(e.target.value)} disabled={loadingScan}>
            {!inList && <option value={`${report.machine.id}|${report.id}`}>{report.machine.computerName} · current scan</option>}
            {machines.map((m) => (
              <optgroup key={m.info.id} label={m.info.computerName}>
                {m.scans.map((s) => (
                  <option key={s.id} value={`${m.info.id}|${s.id}`}>
                    {new Date(s.startedAt).toLocaleString([], { dateStyle: "medium", timeStyle: "short" })} · {kindName[s.kind]} · {s.score}
                  </option>
                ))}
              </optgroup>
            ))}
          </select>
        </div>

        <div className="opt-group">
          <div className="opt-label">Title</div>
          <input value={opts.title} onChange={(e) => set("title", e.target.value)} placeholder="PC Health Report" />
          <div className="opt-label">Prepared for</div>
          <input value={opts.preparedFor} onChange={(e) => set("preparedFor", e.target.value)} placeholder="Customer or owner name (optional)" />
          <div className="opt-label">Technician notes</div>
          <textarea rows={4} value={opts.notes} onChange={(e) => set("notes", e.target.value)} placeholder="Anything you want the owner to know: what you did, what you recommend…" />
        </div>

        <div className="opt-group">
          <div className="opt-label">Include</div>
          {Toggle({ k: "includeComputer", label: "About this computer" })}
          {Toggle({ k: "includeNotes", label: "Notes (for your information)" })}
          {Toggle({ k: "includeChecks", label: "List of everything checked" })}
          {Toggle({ k: "includeVirus", label: `Virus scans run after this scan${laterVirus ? ` (${laterVirus})` : " (none yet)"}`, disabled: laterVirus === 0 })}
          {Toggle({ k: "includeRepairs", label: `Repairs carried out after this scan${laterRepairs ? ` (${laterRepairs})` : " (none yet)"}`, disabled: laterRepairs === 0 })}
          {Toggle({ k: "includeTechnical", label: "Technical details for a technician", hint: "Evidence, measured facts and detail tables for every check" })}
        </div>

        <div className="opt-group">
          <div className="opt-label">
            Branding
            <button className="link-btn" onClick={onOpenSettings}>
              <SettingsIcon size={12} /> Edit
            </button>
          </div>
          {noBrand ? (
            <div className="muted small">
              Add your logo, company name and technician name in{" "}
              <button className="link-btn inline" onClick={onOpenSettings}>
                Settings
              </button>
              .
            </div>
          ) : (
            <>
              {Toggle({ k: "showLogo", label: "Logo", disabled: !b.logo })}
              {Toggle({ k: "showCompany", label: b.company ? `Company: ${b.company}` : "Company", disabled: !b.company.trim() })}
              {Toggle({ k: "showContact", label: b.contact ? `Contact: ${b.contact}` : "Contact", disabled: !b.contact.trim() })}
              {Toggle({ k: "showTech", label: b.tech ? `Technician: ${b.tech}` : "Technician", disabled: !b.tech.trim() })}
            </>
          )}
        </div>

        <div className="opt-actions">
          <button className="btn" disabled={!!saving || !inTauri} onClick={() => save("pdf")}>
            {saving === "pdf" ? <Loader2 size={15} className="spin" /> : <FileDown size={15} />} Save as PDF
          </button>
          <button className="btn ghost" disabled={!!saving || !inTauri} onClick={() => save("html")}>
            {saving === "html" ? <Loader2 size={15} className="spin" /> : <FileDown size={15} />} Save as HTML
          </button>
        </div>
        {!inTauri && <div className="muted small">Saving works inside the DxTool app.</div>}
        {error && <div className="banner error">{error}</div>}
        {saved && (
          <div className="saved-box">
            <div className="small">Saved to</div>
            <div className="saved-path" title={saved}>
              {saved}
            </div>
            <div className="row-gap">
              <button className="btn small" onClick={() => api.openPath(saved)}>
                Open
              </button>
              <button className="btn small ghost" onClick={() => api.showInFolder(saved)}>
                <FolderOpen size={13} /> Show in folder
              </button>
            </div>
          </div>
        )}
      </aside>
      <div className="report-preview">
        <iframe title="Report preview" srcDoc={html} sandbox="allow-same-origin" />
      </div>
    </div>
  );
}
