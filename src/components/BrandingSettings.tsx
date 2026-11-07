import { useEffect, useRef, useState } from "react";
import { FolderOpen, ImagePlus, Trash2 } from "lucide-react";
import type { Branding } from "../lib/types";
import { api } from "../lib/api";

/** Logo, company, contact and technician name for reports. Saved in the DxTool data folder. */
export function BrandingSettings({ branding, onChange }: { branding: Branding; onChange: (b: Branding) => void }) {
  const file = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [dataDir, setDataDir] = useState("");
  useEffect(() => {
    api.dataDir().then(setDataDir).catch(() => {});
  }, []);
  const set = <K extends keyof Branding>(k: K, v: Branding[K]) => onChange({ ...branding, [k]: v });

  const pickLogo = (f: File | undefined) => {
    setError(null);
    if (!f) return;
    if (!f.type.startsWith("image/")) return setError("Please choose an image file (PNG, JPG, SVG…).");
    if (f.size > 1_500_000) return setError("That image is over 1.5 MB. Please use a smaller logo.");
    const reader = new FileReader();
    reader.onload = () => set("logo", String(reader.result));
    reader.onerror = () => setError("Could not read that image.");
    reader.readAsDataURL(f);
  };

  const Row = ({ label, hint, k, show, placeholder }: { label: string; hint: string; k: "company" | "contact" | "tech"; show: "showCompany" | "showContact" | "showTech"; placeholder: string }) => (
    <div className="setting-row">
      <div>
        <div className="setting-name">{label}</div>
        <div className="muted small">{hint}</div>
      </div>
      <div className="brand-input">
        <input value={branding[k]} placeholder={placeholder} onChange={(e) => set(k, e.target.value)} />
        <label className="opt-toggle">
          <input type="checkbox" checked={branding[show]} onChange={(e) => set(show, e.target.checked)} />
          <span>Show on reports</span>
        </label>
      </div>
    </div>
  );

  return (
    <section className="card">
      <h2>Report branding</h2>
      <p className="muted small">Shown at the top of every report. Each item can be hidden here, or per report on the Reports page.</p>
      <div className="setting-row">
        <div>
          <div className="setting-name">Logo</div>
          <div className="muted small">PNG, JPG or SVG, up to 1.5 MB. A wide logo works best.</div>
        </div>
        <div className="brand-input">
          <div className="logo-row">
            {branding.logo ? <img className="logo-preview" src={branding.logo} alt="Logo preview" /> : <div className="logo-empty">No logo</div>}
            <button className="btn small ghost" onClick={() => file.current?.click()}>
              <ImagePlus size={13} /> {branding.logo ? "Change" : "Choose image"}
            </button>
            {branding.logo && (
              <button className="icon-btn ghost" onClick={() => set("logo", null)} title="Remove logo">
                <Trash2 size={14} />
              </button>
            )}
            <input ref={file} type="file" accept="image/*" hidden onChange={(e) => pickLogo(e.target.files?.[0])} />
          </div>
          <label className="opt-toggle">
            <input type="checkbox" checked={branding.showLogo} onChange={(e) => set("showLogo", e.target.checked)} />
            <span>Show on reports</span>
          </label>
        </div>
      </div>
      {error && <div className="banner error">{error}</div>}
      {Row({ label: "Company name", hint: "Your business or your name", k: "company", show: "showCompany", placeholder: "e.g. Scofield PC Repair" })}
      {Row({ label: "Contact", hint: "Phone, email or website under the company name", k: "contact", show: "showContact", placeholder: "e.g. 555-0123 · help@example.com" })}
      {Row({ label: "Technician", hint: "Who ran the scan", k: "tech", show: "showTech", placeholder: "e.g. Alex" })}
      {dataDir && (
        <div className="setting-row">
          <div>
            <div className="setting-name">Data folder</div>
            <div className="muted small">Settings, scan history and saved reports live here. Copy it to move everything to another PC.</div>
            <div className="muted small mono-path">{dataDir}</div>
          </div>
          <button className="btn small ghost" onClick={() => api.openPath(dataDir)}>
            <FolderOpen size={13} /> Open
          </button>
        </div>
      )}
    </section>
  );
}
