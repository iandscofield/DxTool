import { useEffect, useState } from "react";
import { Bot, Check, CheckCircle2, Copy, Eye, Loader2, Plug, Unplug } from "lucide-react";
import { api } from "../lib/api";
import type { ClaudeStatus } from "../lib/types";

/** Settings card that connects DxTool to Claude Desktop / Claude Code through MCP. */
export function ClaudeSettings() {
  const [st, setSt] = useState<ClaudeStatus | null>(null);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [copied, setCopied] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);
  const [loadingPreview, setLoadingPreview] = useState(false);

  const load = () => api.claudeStatus().then(setSt).catch(() => setSt(null));
  useEffect(() => {
    load();
  }, []);

  const setup = async (remove: boolean) => {
    setMsg(null);
    try {
      setMsg({ ok: true, text: await api.claudeSetup(remove) });
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
    }
    load();
  };
  const copy = async () => {
    if (!st) return;
    try {
      await navigator.clipboard.writeText(st.claudeCodeCommand);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      setMsg({ ok: false, text: "Couldn't copy. Select the command and copy it by hand." });
    }
  };
  const tryIt = async () => {
    setLoadingPreview(true);
    try {
      setPreview(await api.mcpTryTool("get_pc_overview"));
    } catch (e) {
      setPreview(String(e));
    } finally {
      setLoadingPreview(false);
    }
  };

  return (
    <section className="card claude-card">
      <h2>
        <Bot size={18} /> Claude AI assistant
      </h2>
      <p className="muted small">
        Connect DxTool to Claude so you can ask questions like "why does my PC keep crashing?" in plain English. Claude can run health scans, read the results, hardware details and event logs, and explain them. It can't
        run repairs or delete files: those stay here in DxTool, where you confirm them. This uses MCP, the standard way Claude connects to tools on your PC. It only works while Claude is open and needs a Claude account.
      </p>

      <div className="setting-row">
        <div>
          <div className="setting-name">Claude Desktop</div>
          <div className="muted small">
            {!st
              ? "Checking…"
              : st.state === "set_up"
                ? "Connected. In Claude, look for DxTool under the tools (slider) icon."
                : st.state === "moved"
                  ? "Set up for a DxTool in a different folder. Update it to point to this copy."
                  : st.desktopInstalled
                    ? "Installed, not connected to DxTool yet."
                    : "Claude Desktop doesn't seem to be installed. Get it from claude.ai/download, then come back here."}
          </div>
        </div>
        <div className="row-gap">
          {st?.state === "set_up" ? (
            <>
              <span className="badge s-ok">
                <CheckCircle2 size={13} /> Connected
              </span>
              <button className="btn small ghost" onClick={() => setup(true)}>
                <Unplug size={13} /> Disconnect
              </button>
            </>
          ) : (
            <button className="btn small" onClick={() => setup(false)} disabled={!st}>
              <Plug size={13} /> {st?.state === "moved" ? "Update connection" : "Connect to Claude Desktop"}
            </button>
          )}
        </div>
      </div>
      {msg && <div className={`banner ${msg.ok ? "ok" : "error"} claude-msg`}>{msg.text}</div>}

      <div className="setting-row">
        <div>
          <div className="setting-name">Claude Code</div>
          <div className="muted small">For Claude in the terminal or the Code tab: run this command once.</div>
        </div>
      </div>
      {st && (
        <div className="cmd-box">
          <code>{st.claudeCodeCommand}</code>
          <button className="icon-btn ghost" onClick={copy} title="Copy">
            {copied ? <Check size={14} /> : <Copy size={14} />}
          </button>
        </div>
      )}

      <div className="setting-row">
        <div>
          <div className="setting-name">See what Claude sees</div>
          <div className="muted small">Shows the PC summary DxTool hands to Claude when asked. Nothing is sent anywhere by this button.</div>
        </div>
        <button className="btn small ghost" onClick={tryIt} disabled={loadingPreview}>
          {loadingPreview ? <Loader2 size={13} className="spin" /> : <Eye size={13} />} Preview
        </button>
      </div>
      {preview && <pre className="console">{preview}</pre>}
      {st && <div className="muted small claude-foot">If you move the DxTool folder, come back here and update the connection. Program: {st.exe}</div>}
    </section>
  );
}
