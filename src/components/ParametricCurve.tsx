import { useId, useMemo } from "react";

import { parametricResponseDb } from "../lib/dsp";
import { formatHz } from "../lib/format";
import type { AutoEqFilter } from "../lib/types";

const WIDTH = 720;
const HEIGHT = 210;
const PAD = { left: 46, right: 16, top: 14, bottom: 28 };
const PLOT_W = WIDTH - PAD.left - PAD.right;
const PLOT_H = HEIGHT - PAD.top - PAD.bottom;
const RANGE_DB = 6;
const MIN_FREQ = 20;
const MAX_FREQ = 20000;
const BAND = { from: 2000, to: 8500 };

const DB_TICKS = [-6, -3, 0, 3, 6];
const FREQ_TICKS = [50, 200, 1000, 2000, 4000, 8000, 16000];

function logPosition(freq: number): number {
  return Math.log(freq / MIN_FREQ) / Math.log(MAX_FREQ / MIN_FREQ);
}

function xFor(freq: number): number {
  return PAD.left + logPosition(freq) * PLOT_W;
}

function yFor(db: number): number {
  const clamped = Math.max(-RANGE_DB, Math.min(RANGE_DB, db));
  return PAD.top + ((RANGE_DB - clamped) / (2 * RANGE_DB)) * PLOT_H;
}

interface ParametricCurveProps {
  filters: AutoEqFilter[];
  ariaLabel?: string;
}

export function ParametricCurve({ filters, ariaLabel }: ParametricCurveProps) {
  const rawId = useId();
  const uid = rawId.replace(/[^a-zA-Z0-9]/g, "");

  const { line, area } = useMemo(() => {
    const count = 260;
    const points: { freq: number; db: number }[] = [];
    for (let index = 0; index < count; index += 1) {
      const freq = MIN_FREQ * Math.pow(MAX_FREQ / MIN_FREQ, index / (count - 1));
      points.push({ freq, db: parametricResponseDb(filters, freq) });
    }
    const path = points
      .map(
        (point, index) =>
          `${index === 0 ? "M" : "L"}${xFor(point.freq).toFixed(2)},${yFor(point.db).toFixed(2)}`,
      )
      .join(" ");
    const zero = yFor(0).toFixed(2);
    return {
      line: path,
      area: `${path} L ${xFor(MAX_FREQ).toFixed(2)},${zero} L ${xFor(MIN_FREQ).toFixed(2)},${zero} Z`,
    };
  }, [filters]);

  return (
    <svg
      className="ear-curve"
      viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
      role="img"
      aria-label={ariaLabel}
    >
      <defs>
        <linearGradient id={`ear-stroke-${uid}`} x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%" stopColor="var(--accent)" />
          <stop offset="100%" stopColor="var(--accent-2)" />
        </linearGradient>
        <linearGradient id={`ear-fill-${uid}`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.24" />
          <stop offset="100%" stopColor="var(--accent)" stopOpacity="0.02" />
        </linearGradient>
        <clipPath id={`ear-clip-${uid}`}>
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

      <rect
        x={xFor(BAND.from)}
        y={PAD.top}
        width={xFor(BAND.to) - xFor(BAND.from)}
        height={PLOT_H}
        fill="var(--accent-soft)"
        opacity="0.55"
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

      {[BAND.from, BAND.to].map((freq) => (
        <line
          key={freq}
          x1={xFor(freq)}
          x2={xFor(freq)}
          y1={PAD.top}
          y2={HEIGHT - PAD.bottom}
          stroke="var(--accent)"
          strokeWidth="0.9"
          strokeDasharray="3 4"
          opacity="0.4"
        />
      ))}

      <g clipPath={`url(#ear-clip-${uid})`}>
        <path d={area} fill={`url(#ear-fill-${uid})`} />
        <path
          d={line}
          fill="none"
          stroke={`url(#ear-stroke-${uid})`}
          strokeWidth="2.4"
          strokeLinecap="round"
          strokeLinejoin="round"
        />
      </g>

      {filters.map((filter, index) => {
        const x = xFor(filter.freq);
        const y = yFor(parametricResponseDb(filters, filter.freq));
        return (
          <g key={`${filter.freq}-${index}`} pointerEvents="none">
            <circle
              cx={x}
              cy={y}
              r="4"
              fill="var(--panel-solid)"
              stroke={`url(#ear-stroke-${uid})`}
              strokeWidth="2"
            />
            <text x={x} y={y - 11} textAnchor="middle" className="eq-node-text">
              {filter.gainDb > 0 ? "+" : ""}
              {filter.gainDb.toFixed(1)}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
