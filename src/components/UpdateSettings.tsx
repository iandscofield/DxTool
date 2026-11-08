import { useEffect, useState } from "react";
import { CheckCircle2, Download, ExternalLink, Loader2, RefreshCw } from "lucide-react";
import { api } from "../lib/api";
import type { UpdateInfo } from "../lib/types";

/** Very small Markdown-to-React for release notes: headings, bullet points and **bold**. */
function Notes({ text }: { text: string }) {
  const bold = (s: string) => s.split(/\*\*(.+?)\*\*/g).map((part, i) => (i % 2 ? <strong key={i}>{part}</strong> : part));
  const out: React.ReactNode[] = [];
  let list: React.ReactNode[] = [];
  const flush = () => {
    if (list.length) out.push(<ul key={`u${out.length}`}>{list}</ul>);
    list = [];
  };
  text.split(/\r?\n/).forEach((raw, i) => {
    const line = raw.trimEnd();
    const item = line.match(/^\s*[-*] (.*)$/);
    if (item) {
      list.push(<li key={i}>{bold(item[1])}</li>);
      return;
    }
    flush();
    const h = line.match(/^#{1,6} (.*)$/);
    if (h) out.push(<h4 key={i}>{bold(h[1])}</h4>);
    else if (line.trim() && line.trim() !== "---") out.push(<p key={i}>{bold(line)}</p>);
  });
  flush();
  return <div className="release-notes">{out}</div>;
}

const mb = (n: number) => `${(n / 1_000_000).toFixed(1)} MB`;

/** Settings card: current version, "Check for updates", and installing the new version. */
export function UpdateSettings({
  version,
  checkOnStart,
  onCheckOnStart,
  found,
}: {
  version: string;
  checkOnStart: boolean;
  onCheckOnStart: (v: boolean) => void;
  /** Result of the start-up check, if one ran. */
  found: UpdateInfo | null;
}) {
  const [kind, setKind] = useState<string>("");
  const [info, setInfo] = useState<UpdateInfo | null>(found);
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [done, setDone] = useState<string | null>(null);

  useEffect(() => {
    api.installKind().then(setKind).catch(() => {});
  }, []);
  useEffect(() => {
    if (found) setInfo(found);
  }, [found]);

  const check = async () => {
    setChecking(true);
    setError(null);
    try {
      setInfo(await api.updateCheck());
    } catch (e) {
      setError(String(e));
    } finally {
      setChecking(false);
    }
  };

  const install = async () => {
    setError(null);
    setProgress({ done: 0, total: info?.downloadSize ?? 0 });
    try {
      setDone(await api.updateInstall((p) => setProgress(p)));
    } catch (e) {
      setError(String(e));
      setProgress(null);
    }
  };

  const installing = !!progress && !done;
  const pct = progress && progress.total ? Math.min(100, Math.round((progress.done / progress.total) * 100)) : null;

  return (
    <section className="card">
      <h2>Updates</h2>
      <div className="setting-row">
        <div>
          <div className="setting-name">DxTool {version}</div>
          <div className="muted small">
            {kind === "installed" ? "Installed copy. Updates run the installer quietly." : kind === "portable" ? "Portable copy. Updates replace DxTool.exe in its folder." : ""} New versions come from DxTool's GitHub
            releases. DxTool only goes online when you check.
          </div>
        </div>
        <button className="btn" onClick={check} disabled={checking || installing}>
          {checking ? <Loader2 size={14} className="spin" /> : <RefreshCw size={14} />} Check for updates
        </button>
      </div>
      <div className="setting-row">
        <div>
          <div className="setting-name">Check when DxTool starts</div>
          <div className="muted small">Looks for a new version each time DxTool opens and shows a note at the top if there is one. Nothing is downloaded without asking.</div>
        </div>
        <label className="opt-toggle">
          <input type="checkbox" checked={checkOnStart} onChange={(e) => onCheckOnStart(e.target.checked)} />
          {checkOnStart ? "On" : "Off"}
        </label>
      </div>

      {error && <div className="banner error">{error}</div>}

      {info && !info.newer && !error && (
        <p className="update-ok">
          <CheckCircle2 size={16} /> You have the newest version{info.latest && info.latest !== info.current ? ` (the latest release is ${info.latest})` : ""}.
        </p>
      )}

      {info?.newer && (
        <div className="update-box">
          <div className="update-head">
            <div>
              <div className="setting-name">DxTool {info.latest} is available</div>
              <div className="muted small">
                {info.published ? `Released ${new Date(info.published).toLocaleDateString()}. ` : ""}
                {info.downloadName ? `${info.downloadName}, ${mb(info.downloadSize)}.` : "This release has no automatic download for this copy; get it from the release page."}
              </div>
            </div>
            <div className="row-gap">
              {info.pageUrl && (
                <button className="btn ghost small" onClick={() => api.openPath(info.pageUrl)}>
                  <ExternalLink size={13} /> Release page
                </button>
              )}
              {info.downloadName && !done && (
                <button className="btn" onClick={install} disabled={installing}>
                  {installing ? <Loader2 size={14} className="spin" /> : <Download size={14} />} {installing ? "Installing…" : "Download and install"}
                </button>
              )}
            </div>
          </div>
          {installing && (
            <div className="progress-wrap">
              <div className="progress-bar">
                <span style={{ width: `${pct ?? 5}%` }} />
              </div>
              <div className="muted small">
                {progress!.done > 0 ? `Downloading… ${mb(progress!.done)}${progress!.total ? ` of ${mb(progress!.total)}` : ""}` : "Starting the download…"}
              </div>
            </div>
          )}
          {done && <p className="update-ok">{done}</p>}
          {!installing && !done && <p className="muted small">DxTool will close and reopen as the new version. Your saved scans and settings are kept. Close any scans first.</p>}
          {info.notes && <Notes text={info.notes} />}
        </div>
      )}
    </section>
  );
}
