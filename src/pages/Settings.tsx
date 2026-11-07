import { Monitor, Moon, Sun } from "lucide-react";
import type { AppInfo, Branding, Mode, ThemePref } from "../lib/types";
import { BrandingSettings } from "../components/BrandingSettings";

export function Settings({
  theme,
  setTheme,
  mode,
  setMode,
  info,
  onElevate,
  branding,
  onBranding,
}: {
  theme: ThemePref;
  setTheme: (t: ThemePref) => void;
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
