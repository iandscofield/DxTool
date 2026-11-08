import { useCallback, useState } from "react";
import { api } from "./api";
import type { AvEngine, AvEvent, AvJob, AvKind, VirusScan } from "./types";

function applyAvEvent(j: AvJob, e: AvEvent): AvJob {
  switch (e.type) {
    case "phase":
      return { ...j, phase: e.text, current: "" };
    case "output":
      return { ...j, lines: [...j.lines, e.line].slice(-400) };
    case "progress":
      return { ...j, done: e.done, total: e.total, current: e.current };
    case "threat":
      return { ...j, threats: [...j.threats, e.threat] };
  }
}

const engineName: Record<AvEngine, string> = { defender: "Microsoft Defender", clamav: "ClamAV" };
const kindName: Record<AvKind, string> = { quick: "Quick scan", full: "Full scan", custom: "Custom scan" };

/** Keeps the running virus scan / download alive while the user moves between pages. */
export function useAvJob(onFinished: () => void) {
  const [job, setJob] = useState<AvJob | null>(null);
  const running = !!job && !job.finished;

  const start = useCallback(
    async (init: Pick<AvJob, "op" | "engine" | "kind" | "title">, run: (onEvent: (e: AvEvent) => void) => Promise<VirusScan | string>) => {
      if (job && !job.finished) return;
      setJob({ ...init, phase: "Starting…", lines: [], done: 0, total: 0, current: "", threats: [], startedAt: Date.now() });
      try {
        const r = await run((e) => setJob((j) => (j ? applyAvEvent(j, e) : j)));
        setJob((j) => (j ? { ...j, finished: true, ...(typeof r === "string" ? { message: r } : { result: r, threats: r.threats }) } : j));
      } catch (e) {
        const text = String(e).replace(/^Error: /, "");
        setJob((j) => (j ? { ...j, finished: true, error: text } : j));
      } finally {
        onFinished();
      }
    },
    [job, onFinished],
  );

  return {
    job,
    running,
    scan: (engine: AvEngine, kind: AvKind, paths: string[] = []) =>
      start({ op: "scan", engine, kind, title: `${engineName[engine]} · ${kindName[kind]}` }, (on) => api.virusScan(engine, kind, paths, on)),
    update: (engine: AvEngine) =>
      start({ op: "update", engine, title: engine === "defender" ? "Defender definitions update" : "ClamAV virus database update" }, (on) => api.avUpdate(engine, on)),
    install: () => start({ op: "install", engine: "clamav", title: "ClamAV download" }, (on) => api.clamavInstall(on)),
    cancel: () => {
      setJob((j) => (j ? { ...j, cancelling: true } : j));
      api.avCancel();
    },
    clear: () => setJob((j) => (j && !j.finished ? j : null)),
    /** Replaces the finished job's result after the user acts on a threat. */
    updateResult: (r: VirusScan) => setJob((j) => (j?.result?.id === r.id ? { ...j, result: r, threats: r.threats } : j)),
  };
}

export type AvJobApi = ReturnType<typeof useAvJob>;
