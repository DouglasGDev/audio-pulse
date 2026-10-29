import { formatDb } from "../lib/format";

export function GainReduction({
  label,
  value,
  max,
}: {
  label: string;
  value: number;
  max: number;
}) {
  const amount = Math.min(1, Math.abs(value) / max);
  return (
    <div className={`gr-meter ${amount > 0.02 ? "is-active" : ""}`}>
      <div className="gr-head">
        <span>{label}</span>
        <strong>{value < -0.01 ? formatDb(value) : "0.0 dB"}</strong>
      </div>
      <div className="gr-track">
        <div className="gr-fill" style={{ width: `${amount * 100}%` }} />
      </div>
    </div>
  );
}
