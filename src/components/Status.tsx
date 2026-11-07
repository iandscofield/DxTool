import { AlertTriangle, CheckCircle2, Info, XCircle } from "lucide-react";
import type { Status } from "../lib/types";

const label: Record<Status, string> = { ok: "OK", info: "Note", warn: "Warning", error: "Problem" };

export function StatusDot({ status, title }: { status: Status; title?: string }) {
  return <span className={`status-dot s-${status}`} title={title ?? label[status]} />;
}

export function StatusBadge({ status, text }: { status: Status; text?: string | null }) {
  const Icon = status === "ok" ? CheckCircle2 : status === "info" ? Info : status === "warn" ? AlertTriangle : XCircle;
  return (
    <span className={`badge s-${status}`}>
      <Icon size={13} />
      {text ?? label[status]}
    </span>
  );
}
