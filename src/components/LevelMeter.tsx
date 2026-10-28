interface LevelMeterProps {
  left: number;
  right: number;
  peakLeft: number;
  peakRight: number;
  compact?: boolean;
}

function toWidth(value: number): number {
  if (!Number.isFinite(value) || value <= 1e-5) return 0;
  const db = 20 * Math.log10(value);
  return Math.min(1, Math.max(0, (db + 60) / 60));
}

function Meter({ channel, value, peak }: { channel: string; value: number; peak: number }) {
  return (
    <div className="meter-row">
      <span className="meter-channel">{channel}</span>
      <div className="meter-track">
        <div className="meter-fill" style={{ width: `${toWidth(value) * 100}%` }} />
        <div className="meter-peak" style={{ left: `${toWidth(peak) * 100}%` }} />
      </div>
    </div>
  );
}

export function LevelMeter({ left, right, peakLeft, peakRight, compact }: LevelMeterProps) {
  return (
    <div className={`level-meter ${compact ? "is-compact" : ""}`}>
      <Meter channel="L" value={left} peak={peakLeft} />
      <Meter channel="R" value={right} peak={peakRight} />
    </div>
  );
}