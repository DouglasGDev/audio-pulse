import { formatHz } from "../lib/format";
import { ADAPTIVE_EQ_FREQUENCIES, ADAPTIVE_EQ_MAX_DB } from "../lib/types";
import { useAppStore } from "../store/appStore";

export function AdaptiveMeter({ active }: { active: boolean }) {
  const gains = useAppStore((state) => state.telemetry.adaptiveGainsDb);
  return (
    <div className={`adaptive-meter ${active ? "" : "is-idle"}`} aria-hidden={!active}>
      {ADAPTIVE_EQ_FREQUENCIES.map((freq, index) => {
        const gain = gains[index] ?? 0;
        const ratio = Math.min(1, Math.abs(gain) / ADAPTIVE_EQ_MAX_DB);
        const boost = gain >= 0;
        return (
          <div className="adaptive-band" key={freq}>
            <span className="adaptive-freq">{formatHz(freq)}</span>
            <div className="adaptive-track">
              <span className="adaptive-center" />
              <span
                className={`adaptive-fill ${boost ? "is-boost" : "is-cut"}`}
                style={{
                  width: `${ratio * 50}%`,
                  left: boost ? "50%" : undefined,
                  right: boost ? undefined : "50%",
                }}
              />
            </div>
            <span className="adaptive-db">
              {gain > 0.05 ? "+" : ""}
              {Math.abs(gain) < 0.05 ? "0.0" : gain.toFixed(1)}
            </span>
          </div>
        );
      })}
    </div>
  );
}
