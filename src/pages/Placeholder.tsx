import { Construction } from "lucide-react";
import type { Page } from "../lib/types";

const plans: Partial<Record<Page, { title: string; stage: string; items: string[] }>> = {
  history: {
    title: "History",
    stage: "Stage 4",
    items: ["Every scan saved per machine", "Compare any two scans to see what changed", "Works across all your PCs when DxTool runs from a USB stick"],
  },
  reports: {
    title: "Reports",
    stage: "Stage 4",
    items: ["HTML and PDF reports", "Your logo, company name and technician name (each can be hidden)", "Readable by the PC owner and detailed enough for a technician"],
  },
  fixes: {
    title: "Fixes",
    stage: "Stage 5",
    items: [
      "Unlocked after a scan and a risk acknowledgement",
      "A restore point is always created first",
      "Clear temporary files, repair system files (SFC / DISM), update drivers, and more",
    ],
  },
};

export function Placeholder({ page }: { page: Page }) {
  const p = plans[page];
  if (!p) return null;
  return (
    <div className="page">
      <div className="placeholder card">
        <div className="placeholder-icon">
          <Construction size={28} />
        </div>
        <h1>{p.title}</h1>
        <p className="muted">Coming in {p.stage} of version 1. Here's what this section will do:</p>
        <ul>
          {p.items.map((i) => (
            <li key={i}>{i}</li>
          ))}
        </ul>
      </div>
    </div>
  );
}
