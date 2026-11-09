import { useCallback, useEffect, useRef, useState } from "react";
import { CheckCircle2, CircleAlert, CircleX, CloudDownload, ExternalLink, FolderOpen, Images, Loader2, MessageSquareText, RefreshCw, Smartphone, Square } from "lucide-react";
import { api } from "../lib/api";
import type { BackupEvent, BackupResult, PhoneFinding, PhoneInfo, PhoneStatus } from "../lib/types";

const TERMS_URL = "https://developer.android.com/studio/terms";
const gb = (b: number) => (b >= 10e9 ? `${(b / 1e9).toFixed(0)} GB` : `${(b / 1e9).toFixed(1)} GB`);

function FindingRow({ f }: { f: PhoneFinding }) {
  const Icon = f.level === "ok" ? CheckCircle2 : f.level === "warning" ? CircleAlert : CircleX;
  return (
    <div className={`phone-finding lvl-${f.level}`}>
      <Icon size={16} />
      <div>
        <div className="phone-finding-title">{f.title}</div>
        {f.detail && <div className="muted small">{f.detail}</div>}
      </div>
    </div>
  );
}

function SetupSteps() {
  return (
    <ol className="phone-steps">
      <li>
        On the phone, open <b>Settings → About phone</b> and tap <b>Build number</b> seven times (on Samsung it's under <b>Software information</b>). Enter the PIN if asked. This turns on <b>Developer options</b>.
      </li>
      <li>
        Open <b>Settings → System → Developer options</b> (Samsung: <b>Settings → Developer options</b>) and turn on <b>USB debugging</b>.
      </li>
      <li>Connect the phone to this PC with a USB cable. Some cables only charge: if nothing happens, try another one.</li>
      <li>
        Unlock the phone. When it asks <b>"Allow USB debugging?"</b>, tick <b>Always allow from this computer</b> and tap <b>Allow</b>.
      </li>
    </ol>
  );
}

function Facts({ p }: { p: PhoneInfo }) {
  const b = p.battery;
  const rows: [string, string][] = [
    ["Android", `${p.android}${p.securityPatch ? ` · security patch ${p.securityPatch}` : ""}`],
    ["Battery", [b.level != null ? `${b.level}%` : "", b.status, b.plugged && `on ${b.plugged}`].filter(Boolean).join(" · ")],
    ["Battery health", [b.health && `Android says "${b.health}"`, b.healthPercent != null ? `about ${b.healthPercent.toFixed(0)}% of new capacity` : "", b.capacityMah && b.designMah ? `(${Math.round(b.capacityMah)} of ${Math.round(b.designMah)} mAh)` : ""].filter(Boolean).join(" · ") || "Not reported"],
    ["Charge cycles", b.cycleCount != null ? String(b.cycleCount) : "Not reported by this phone"],
    ["Battery temperature", b.temperatureC != null ? `${b.temperatureC.toFixed(1)} °C` : "—"],
    ["Storage", p.storageTotal ? `${gb(p.storageFree)} free of ${gb(p.storageTotal)}` : "—"],
    ["Memory", p.memoryTotal ? `${gb(p.memoryAvailable)} available of ${gb(p.memoryTotal)}` : "—"],
    ["Since last restart", p.uptimeHours >= 48 ? `${Math.round(p.uptimeHours / 24)} days` : `${Math.round(p.uptimeHours)} hours`],
    ["Apps you installed", String(p.userApps)],
  ];
  return (
    <table className="simple-table phone-facts">
      <tbody>
        {rows.map(([k, v]) => (
          <tr key={k}>
            <th>{k}</th>
            <td>{v}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function BackupCard({ serial }: { serial: string }) {
  const [dest, setDest] = useState("");
  const [running, setRunning] = useState<"photos" | "messages" | null>(null);
  const [ev, setEv] = useState<BackupEvent | null>(null);
  const [result, setResult] = useState<BackupResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let saved: string | null = null;
    try {
      saved = localStorage.getItem("phoneBackupFolder");
    } catch {
      /* ignore */
    }
    if (saved) setDest(saved);
    else api.phoneDefaultFolder().then(setDest);
  }, []);

  const pick = async () => {
    const f = await api.pickFolder("Choose where to save phone backups");
    if (f) {
      setDest(f);
      try {
        localStorage.setItem("phoneBackupFolder", f);
      } catch {
        /* ignore */
      }
    }
  };

  const run = async (kind: "photos" | "messages") => {
    setRunning(kind);
    setResult(null);
    setError(null);
    setEv(null);
    try {
      setResult(await api.phoneBackup(serial, kind, dest, setEv));
    } catch (e) {
      setError(String(e));
    } finally {
      setRunning(null);
    }
  };

  const pct = ev && ev.bytesTotal ? Math.min(100, Math.round((ev.bytesDone / ev.bytesTotal) * 100)) : null;

  return (
    <section className="card">
      <div className="card-head">
        <h2>Back up to this PC</h2>
      </div>
      <div className="phone-dest">
        <span className="muted small">Save to</span>
        <code>{dest || "…"}</code>
        <button className="btn ghost small" onClick={pick} disabled={!!running}>
          Change
        </button>
      </div>
      <div className="row-gap">
        <button className="btn" onClick={() => run("photos")} disabled={!!running || !dest}>
          {running === "photos" ? <Loader2 size={14} className="spin" /> : <Images size={14} />} Back up photos and videos
        </button>
        <button className="btn" onClick={() => run("messages")} disabled={!!running || !dest}>
          {running === "messages" ? <Loader2 size={14} className="spin" /> : <MessageSquareText size={14} />} Back up text messages
        </button>
      </div>
      {running && (
        <div className="phone-progress">
          {pct != null && (
            <div className="progress-bar">
              <span style={{ width: `${pct}%` }} />
            </div>
          )}
          <div className="row-gap">
            <span className="muted small">
              {ev?.phase === "copying" ? `File ${Math.min(ev.done + 1, ev.total)} of ${ev.total} · ${gb(ev.bytesDone)} of ${gb(ev.bytesTotal)} · ${ev.current}` : ev?.current || "Starting…"}
            </span>
            <button className="btn ghost small" onClick={() => api.phoneBackupCancel()}>
              <Square size={12} /> Stop
            </button>
          </div>
          <p className="muted small">Keep the phone connected and unlocked until it finishes.</p>
        </div>
      )}
      {error && <div className="banner error">{error}</div>}
      {result && (
        <div className={`banner ${result.failed || result.cancelled ? "warn" : "ok"}`}>
          <div>
            <div>{result.summary}</div>
            {result.errors.length > 0 && (
              <ul className="small">
                {result.errors.slice(0, 5).map((e) => (
                  <li key={e}>{e}</li>
                ))}
              </ul>
            )}
          </div>
          <button className="btn ghost small" onClick={() => api.openPath(result.folder)}>
            <FolderOpen size={13} /> Open folder
          </button>
        </div>
      )}
      <p className="muted small">
        <b>Photos and videos</b>: everything in the phone's camera (DCIM), Pictures and Movies folders, in the same folders on the PC. Running it again copies only new files. Nothing is deleted from the phone.
        <br />
        <b>Text messages</b>: saved as a readable page, a spreadsheet and a data file. Picture messages (MMS) aren't included. Some phones don't let a PC read messages; DxTool then explains how to use the free "SMS Backup & Restore" app instead.
      </p>
    </section>
  );
}

/** Android phone diagnostics and backups over USB (adb). */
export function Phone() {
  const [status, setStatus] = useState<PhoneStatus | null>(null);
  const [serial, setSerial] = useState<string | null>(null);
  const [info, setInfo] = useState<PhoneInfo | null>(null);
  const [loadingInfo, setLoadingInfo] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [agreed, setAgreed] = useState(false);
  const [dl, setDl] = useState<{ done: number; total: number } | null>(null);
  const timer = useRef<number | null>(null);

  const refresh = useCallback(async () => {
    try {
      const s = await api.phoneStatus();
      setStatus(s);
      const ready = s.devices.filter((d) => d.state === "device");
      setSerial((cur) => (cur && ready.some((d) => d.serial === cur) ? cur : (ready[0]?.serial ?? null)));
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    timer.current = window.setInterval(refresh, 4000);
    return () => {
      if (timer.current) window.clearInterval(timer.current);
    };
  }, [refresh]);

  const loadInfo = useCallback(async (s: string) => {
    setLoadingInfo(true);
    setError(null);
    try {
      setInfo(await api.phoneInfo(s));
    } catch (e) {
      setInfo(null);
      setError(String(e));
    } finally {
      setLoadingInfo(false);
    }
  }, []);

  useEffect(() => {
    if (serial) loadInfo(serial);
    else setInfo(null);
  }, [serial, loadInfo]);

  const install = async () => {
    setError(null);
    setDl({ done: 0, total: 0 });
    try {
      await api.phoneToolsInstall((done, total) => setDl({ done, total }));
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setDl(null);
    }
  };

  const waiting = status?.devices.filter((d) => d.state === "unauthorized") ?? [];
  const other = status?.devices.filter((d) => d.state !== "device" && d.state !== "unauthorized") ?? [];
  const ready = status?.devices.filter((d) => d.state === "device") ?? [];

  return (
    <div className="page phone">
      <div className="page-head">
        <div>
          <h1>
            <Smartphone size={22} /> Phone
          </h1>
          <p className="muted">Check an Android phone's battery, storage and updates, and back up its photos and text messages to this PC, over a USB cable. (iPhone support is planned.)</p>
        </div>
        {status?.toolsInstalled && (
          <button className="btn ghost" onClick={() => (serial ? loadInfo(serial) : refresh())} disabled={loadingInfo}>
            {loadingInfo ? <Loader2 size={14} className="spin" /> : <RefreshCw size={14} />} Check again
          </button>
        )}
      </div>

      {error && <div className="banner error">{error}</div>}
      {status?.problem && <div className="banner warn">{status.problem}</div>}

      {!status ? (
        <section className="card empty-state">
          <Loader2 size={24} className="spin" />
        </section>
      ) : !status.toolsInstalled ? (
        <section className="card">
          <div className="card-head">
            <h2>First, get Google's Android tools</h2>
          </div>
          <p>
            DxTool talks to Android phones through <b>Android Debug Bridge (adb)</b>, Google's free tool for this. It's about 8 MB, comes straight from Google, and DxTool checks it against Google's published checksum before using it. It's kept in DxTool's data folder.
          </p>
          <label className="check-row">
            <input type="checkbox" checked={agreed} onChange={(e) => setAgreed(e.target.checked)} disabled={!!dl} /> I agree to Google's{" "}
            <a
              href={TERMS_URL}
              onClick={(e) => {
                e.preventDefault();
                api.openPath(TERMS_URL);
              }}
            >
              Android SDK terms <ExternalLink size={11} />
            </a>
          </label>
          {dl ? (
            <div className="phone-progress">
              <div className="progress-bar">
                <span style={{ width: `${dl.total ? Math.round((dl.done / dl.total) * 100) : 5}%` }} />
              </div>
              <span className="muted small">
                <Loader2 size={13} className="spin" /> Downloading{dl.total ? ` ${(dl.done / 1e6).toFixed(1)} of ${(dl.total / 1e6).toFixed(1)} MB` : "…"}
              </span>
            </div>
          ) : (
            <button className="btn" onClick={install} disabled={!agreed}>
              <CloudDownload size={14} /> Download Android tools
            </button>
          )}
        </section>
      ) : ready.length === 0 ? (
        <section className="card">
          <div className="card-head">
            <h2>{waiting.length ? "Waiting for the phone to allow this PC" : "Connect an Android phone"}</h2>
            <span className="muted small">
              <Loader2 size={12} className="spin" /> Looking for phones…
            </span>
          </div>
          {waiting.length > 0 ? (
            <div className="banner warn">
              A phone is connected but hasn't allowed this PC yet. Unlock it and tap <b>Allow</b> on the "Allow USB debugging?" message. If there's no message, unplug the cable and plug it in again.
            </div>
          ) : other.length > 0 ? (
            <div className="banner warn">A phone is connected but not ready ({other[0].state}). Unplug it and plug it in again.</div>
          ) : null}
          <SetupSteps />
          <p className="muted small">
            Still not showing up? Set the phone's USB mode to <b>File transfer</b> in the notification that appears when it's plugged in. Samsung phones may need Samsung's USB driver for Windows. Turn USB debugging off again afterwards if you like: DxTool doesn't need it for anything else.
          </p>
        </section>
      ) : (
        <>
          {ready.length > 1 && (
            <div className="row-gap phone-picker">
              <span className="muted small">Phone:</span>
              {ready.map((d) => (
                <button key={d.serial} className={`btn small ${d.serial === serial ? "" : "ghost"}`} onClick={() => setSerial(d.serial)}>
                  {d.model || d.serial}
                </button>
              ))}
            </div>
          )}
          <section className="card">
            {!info ? (
              <p className="muted">
                <Loader2 size={14} className="spin" /> Reading the phone…
              </p>
            ) : (
              <>
                <div className="card-head">
                  <h2>
                    <Smartphone size={16} /> {info.manufacturer ? `${info.manufacturer.charAt(0).toUpperCase()}${info.manufacturer.slice(1)} ` : ""}
                    {info.name || info.model}
                    {info.name && info.name !== info.model && <span className="muted small"> ({info.model})</span>}
                  </h2>
                  <span className="muted small">Read {new Date(info.collectedAt).toLocaleTimeString()}</span>
                </div>
                <div className="phone-findings">
                  {info.findings.map((f) => (
                    <FindingRow key={f.title} f={f} />
                  ))}
                </div>
                <Facts p={info} />
                <p className="muted small">Not every phone reports charge cycles or battery capacity; where it doesn't, Android's own health verdict is shown.</p>
              </>
            )}
          </section>
          {serial && <BackupCard serial={serial} />}
        </>
      )}
    </div>
  );
}
