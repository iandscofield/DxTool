import { Monitor, Moon, Sun } from "lucide-react";
import type { Accent, AppInfo, Branding, Mode, ThemePref } from "../lib/types";
import { BrandingSettings } from "../components/BrandingSettings";
import { ClaudeSettings } from "../components/ClaudeSettings";

export function Settings({
  theme,
  setTheme,
  accent,
  setAccent,
  mode,
  setMode,
  info,
  onElevate,
  branding,
  onBranding,
}: {
  theme: ThemePref;
  setTheme: (t: ThemePref) => void;
  accent: Accent;
  setAccent: (a: Accent) => void;
  mode: Mode;
  setMode: (m: Mode) => void;
  info: AppInfo | null;
  onElevate: () => void;
  branding: Branding;
  onBranding: (b: Branding) => void;
}) {
  return (
    <div className="page settings">
      <h1>Settings</h1>

      <section className="card">
        <h2>Appearance</h2>
        <div className="setting-row">
          <div>
            <div className="setting-name">Theme</div>
            <div className="muted small">Follow Windows, or always use dark or light.</div>
          </div>
          <div className="segmented">
            {(
              [
                ["system", "System", Monitor],
                ["dark", "Dark", Moon],
                ["light", "Light", Sun],
              ] as const
            ).map(([v, l, I]) => (
              <button key={v} className={theme === v ? "on" : ""} onClick={() => setTheme(v)}>
                <I size={14} /> {l}
              </button>
            ))}
          </div>
        </div>
        <div className="setting-row">
          <div>
            <div className="setting-name">Color theme</div>
            <div className="muted small">The highlight color used for buttons, links and charts. Orange &amp; black also makes dark mode pure black.</div>
          </div>
          <div className="swatches" role="radiogroup" aria-label="Color theme">
            {(
              [
                ["blue", "Blue", "linear-gradient(135deg, #3b82f6, #8b5cf6)"],
                ["purple", "Purple", "linear-gradient(135deg, #7c3aed, #d946ef)"],
                ["red", "Red", "linear-gradient(135deg, #dc2626, #f97316)"],
                ["green", "Green", "linear-gradient(135deg, #16a34a, #0d9488)"],
                ["orange", "Orange & black", "linear-gradient(135deg, #ea580c 0%, #f59e0b 50%, #000 50%)"],
              ] as const
            ).map(([v, label, bg]) => (
              <button key={v} className={`swatch ${accent === v ? "on" : ""}`} onClick={() => setAccent(v)} role="radio" aria-checked={accent === v} title={label}>
                <span className="swatch-dot" style={{ background: bg }} />
                {label}
              </button>
            ))}
          </div>
        </div>
        <div className="setting-row">
          <div>
            <div className="setting-name">Detail level</div>
            <div className="muted small">Simple shows the important facts. Advanced shows every property Windows reports.</div>
          </div>
          <div className="segmented">
            <button className={mode === "simple" ? "on" : ""} onClick={() => setMode("simple")}>
              Simple
            </button>
            <button className={mode === "advanced" ? "on" : ""} onClick={() => setMode("advanced")}>
              Advanced
            </button>
          </div>
        </div>
      </section>

      <BrandingSettings branding={branding} onChange={onBranding} />

      <ClaudeSettings />

      <section className="card">
        <h2>About</h2>
        <dl className="kv">
          <div className="kv-row">
            <dt>Version</dt>
            <dd>
              {info?.version ?? "—"}
              {info?.debugBuild ? " (development build)" : ""}
            </dd>
          </div>
          <div className="kv-row">
            <dt>Computer</dt>
            <dd>{info?.computerName}</dd>
          </div>
          <div className="kv-row">
            <dt>Firmware</dt>
            <dd>{info?.firmware}</dd>
          </div>
          <div className="kv-row">
            <dt>Administrator rights</dt>
            <dd>
              {info?.isAdmin ? (
                "Yes"
              ) : (
                <>
                  No{" "}
                  <button className="btn small" onClick={onElevate}>
                    Restart as administrator
                  </button>
                </>
              )}
            </dd>
          </div>
          <div className="kv-row">
            <dt>Licence</dt>
            <dd>Free and open source (MIT)</dd>
          </div>
        </dl>
      </section>
    </div>
  );
}
