/** Circular 0-100 health score. */
export function ScoreRing({ score, grade, size = 132, stroke = 10 }: { score: number; grade?: string; size?: number; stroke?: number }) {
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const tone = score >= 90 ? "excellent" : score >= 75 ? "good" : score >= 50 ? "fair" : "poor";
  const small = size < 90;
  return (
    <div className={`score-ring tone-${tone}`} style={{ width: size, height: size }} role="img" aria-label={`Health score ${score} out of 100${grade ? `, ${grade}` : ""}`}>
      <svg viewBox={`0 0 ${size} ${size}`} width={size} height={size}>
        <circle className="track" cx={size / 2} cy={size / 2} r={r} strokeWidth={stroke} fill="none" />
        <circle
          className="bar"
          cx={size / 2}
          cy={size / 2}
          r={r}
          strokeWidth={stroke}
          fill="none"
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={c * (1 - Math.max(0, Math.min(100, score)) / 100)}
          transform={`rotate(-90 ${size / 2} ${size / 2})`}
        />
      </svg>
      <div className="score-center">
        <div className="score-num" style={{ fontSize: small ? size * 0.32 : size * 0.3 }}>
          {score}
        </div>
        {grade && !small && <div className="score-grade">{grade}</div>}
      </div>
    </div>
  );
}

export function gradeTone(score: number) {
  return score >= 90 ? "excellent" : score >= 75 ? "good" : score >= 50 ? "fair" : "poor";
}
