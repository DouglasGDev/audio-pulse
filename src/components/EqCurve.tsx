import { useId, useMemo, useRef, type PointerEvent as ReactPointerEvent } from "react";

import { CURVE_RANGE_DB, curvePoints, logPosition } from "../lib/dsp";
import { formatHz } from "../lib/format";
import { EQ_FREQUENCIES, type ProcessingParams } from "../lib/types";

const WIDTH = 720;
const HEIGHT = 250;
const PAD = { left: 46, right: 16, top: 16, bottom: 30 };
const PLOT_W = WIDTH - PAD.left - PAD.right;
const PLOT_H = HEIGHT - PAD.top - PAD.bottom;

const DB_TICKS = [-12, -6, 0, 6, 12];
const FREQ_TICKS = [31, 125, 500, 2000, 8000, 16000];

function xFor(freq: number): number {
  return PAD.left + logPosition(freq) * PLOT_W;
}

function yFor(db: number): number {
  const clamped = Math.max(-CURVE_RANGE_DB, Math.min(CURVE_RANGE_DB, db));
  return PAD.top + ((CURVE_RANGE_DB - clamped) / (2 * CURVE_RANGE_DB)) * PLOT_H;
}

interface EqCurveProps {
  params: ProcessingParams;
  onBandGain?: (index: number, gainDb: number) => void;
  compact?: boolean;
  ariaLabel?: string;
}

export function EqCurve({ params, onBandGain, compact, ariaLabel }: EqCurveProps) {
  const svgRef = useRef<SVGSVGElement>(null);
  const dragging = useRef<number | null>(null);
  const rawId = useId();
  const uid = rawId.replace(/[^a-zA-Z0-9]/g, "");

  const { line, area } = useMemo(() => {
    const points = curvePoints(params);
    const path = points
      .map(
        (point, index) =>
          `${index === 0 ? "M" : "L"}${xFor(point.freq).toFixed(2)},${yFor(point.db).toFixed(2)}`,
      )
      .join(" ");
    const first = points[0];
    const last = points[points.length - 1];
    const zero = yFor(0).toFixed(2);
    return {
      line: path,
      area: `${path} L ${xFor(last.freq).toFixed(2)},${zero} L ${xFor(first.freq).toFixed(2)},${zero} Z`,
    };
  }, [params]);

  const dbFromPointer = (clientY: number): number => {
    const svg = svgRef.current;
    if (!svg) return 0;
    const rect = svg.getBoundingClientRect();
    const viewY = ((clientY - rect.top) / rect.height) * HEIGHT;
    const ratio = (viewY - PAD.top) / PLOT_H;
    return Math.max(-12, Math.min(12, CURVE_RANGE_DB - ratio * 2 * CURVE_RANGE_DB));
  };

  const startDrag = (index: number) => (event: ReactPointerEvent<SVGCircleElement>) => {
    if (!onBandGain || event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    dragging.current = index;
    svgRef.current?.setPointerCapture(event.pointerId);
    onBandGain(index, dbFromPointer(event.clientY));
  };

  const moveDrag = (event: ReactPointerEvent<SVGSVGElement>) => {
    if (dragging.current === null || !onBandGain) return;
    onBandGain(dragging.current, dbFromPointer(event.clientY));
  };

  const endDrag = (event: ReactPointerEvent<SVGSVGElement>) => {
    if (dragging.current === null) return;
    dragging.current = null;
    if (svgRef.current?.hasPointerCapture(event.pointerId)) {
      svgRef.current.releasePointerCapture(event.pointerId);
    }
  };

  return (
    <svg
      ref={svgRef}
      className={`eq-curve ${compact ? "is-compact" : ""}`}
      viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
      role="img"
      aria-label={ariaLabel}
      onPointerMove={moveDrag}
      onPointerUp={endDrag}
      onPointerCancel={endDrag}
    >
      <defs>
        <linearGradient id={`stroke-${uid}`} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%" stopColor="var(--accent)" />
          <stop offset="100%" stopColor="var(--accent-2)" />
        </linearGradient>
        <linearGradient id={`fill-${uid}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.26" />
          <stop offset="100%" stopColor="var(--accent)" stopOpacity="0.02" />
        </linearGradient>
        <clipPath id={`clip-${uid}`}>
          <rect x={PAD.left} y={PAD.top} width={PLOT_W} height={PLOT_H} rx="8" />
        </clipPath>
      </defs>

      <rect
        x={PAD.left}
        y={PAD.top}
        width={PLOT_W}
        height={PLOT_H}
        rx="10"
        fill="var(--graph-bg)"
      />

      {DB_TICKS.map((db) => (
        <g key={db}>
          <line
            x1={PAD.left}
            x2={WIDTH - PAD.right}
            y1={yFor(db)}
            y2={yFor(db)}
            stroke="var(--line)"
            strokeWidth={db === 0 ? 1.2 : 0.7}
            opacity={db === 0 ? 0.9 : 0.55}
          />
          <text
            x={PAD.left - 8}
            y={yFor(db) + 3.5}
            textAnchor="end"
            className="eq-axis-text"
          >
            {db > 0 ? `+${db}` : db}
          </text>
        </g>
      ))}

      {FREQ_TICKS.map((freq) => (
        <g key={freq}>
          <line
            x1={xFor(freq)}
            x2={xFor(freq)}
            y1={PAD.top}
            y2={HEIGHT - PAD.bottom}
            stroke="var(--line)"
            strokeWidth="0.6"
            opacity="0.45"
          />
          <text
            x={xFor(freq)}
            y={HEIGHT - 10}
            textAnchor="middle"
            className="eq-axis-text"
          >
            {formatHz(freq)}
          </text>
        </g>
      ))}

      <g clipPath={`url(#clip-${uid})`}>
        <path d={area} fill={`url(#fill-${uid})`} />
        <path
          d={line}
          fill="none"
          stroke={`url(#stroke-${uid})`}
          strokeWidth="2.6"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </g>

      {EQ_FREQUENCIES.map((freq, index) => {
        const gain = params.eqDb[index] ?? 0;
        const x = xFor(freq);
        const y = yFor(gain);
        return (
          <g key={freq}>
            {onBandGain && (
              <circle
                cx={x}
                cy={y}
                r="13"
                fill="transparent"
                style={{ cursor: "ns-resize" }}
                onPointerDown={startDrag(index)}
              >
                <title>{`${formatHz(freq)}: ${gain > 0 ? "+" : ""}${gain.toFixed(1)} dB`}</title>
              </circle>
            )}
            <circle
              cx={x}
              cy={y}
              r={Math.abs(gain) > 0.2 ? 5 : 4}
              fill="var(--panel-solid)"
              stroke={`url(#stroke-${uid})`}
              strokeWidth="2"
              pointerEvents="none"
            />
            {Math.abs(gain) >= 0.5 && (
              <text
                x={x}
                y={y - 12}
                textAnchor="middle"
                className="eq-node-text"
                pointerEvents="none"
              >
                {gain > 0 ? "+" : ""}
                {gain.toFixed(1)}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
}