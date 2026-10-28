import type { SpatialMode } from "../lib/types";

interface SpatialStageProps {
  mode: SpatialMode;
  width: number;
  room: number;
  active?: boolean;
  ariaLabel?: string;
}

interface SpeakerSpec {
  x: number;
  y: number;
  label: string;
  small?: boolean;
}

const CX = 240;
const CY = 168;

function speakerLayout(mode: SpatialMode, width: number, room: number): SpeakerSpec[] {
  if (mode === "off") {
    return [
      { x: CX - 96, y: CY - 46, label: "L" },
      { x: CX + 96, y: CY - 46, label: "R" },
    ];
  }
  if (mode === "binaural") {
    const spread = 62 + width * 34;
    return [
      { x: CX - spread, y: CY - 86, label: "L" },
      { x: CX + spread, y: CY - 86, label: "R" },
      { x: CX, y: CY - 112, label: "C", small: true },
    ];
  }
  if (mode === "hrtf") {
    const spread = 86 + width * 44;
    return [
      { x: CX - spread, y: CY - 92, label: "L" },
      { x: CX + spread, y: CY - 92, label: "R" },
      { x: CX, y: CY - 116, label: "C", small: true },
    ];
  }
  const s = 0.9 + width * 0.16;
  const depth = 0.85 + room * 0.3;
  return [
    { x: CX - 112 * s, y: CY - 82 * depth, label: "L" },
    { x: CX + 112 * s, y: CY - 82 * depth, label: "R" },
    { x: CX, y: CY - 104 * depth, label: "C", small: true },
    { x: CX - 152 * s, y: CY - 16, label: "SL", small: true },
    { x: CX + 152 * s, y: CY - 16, label: "SR", small: true },
    { x: CX - 104 * s, y: CY + 52, label: "RL", small: true },
    { x: CX + 104 * s, y: CY + 52, label: "RR", small: true },
    { x: CX, y: CY + 64, label: "LFE", small: true },
  ];
}

function Speaker({ spec }: { spec: SpeakerSpec }) {
  const scale = spec.small ? 0.7 : 1;
  return (
    <g className="stage-speaker" transform={`translate(${spec.x} ${spec.y})`}>
      <rect
        x={-11 * scale}
        y={-7.5 * scale}
        width={22 * scale}
        height={15 * scale}
        rx={4 * scale}
      />
      <circle cx={-4 * scale} cy={0} r={2.6 * scale} />
      <circle cx={4 * scale} cy={0} r={2.6 * scale} />
      <text y={19 * scale} textAnchor="middle">
        {spec.label}
      </text>
    </g>
  );
}

export function SpatialStage({ mode, width, room, active, ariaLabel }: SpatialStageProps) {
  const speakers = speakerLayout(mode, width, room);
  const binaural = mode === "binaural" || mode === "hrtf";
  const surround = mode === "surround";

  return (
    <svg
      className={`spatial-stage ${active ? "is-active" : ""}`}
      viewBox="0 0 480 280"
      role="img"
      aria-label={ariaLabel}
    >
      <defs>
        <radialGradient id="stage-floor" cx="50%" cy="62%" r="55%">
          <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.13" />
          <stop offset="100%" stopColor="var(--accent)" stopOpacity="0" />
        </radialGradient>
      </defs>

      <rect width="480" height="280" fill="transparent" />
      <ellipse cx={CX} cy={CY - 26} rx={205} ry={126} fill="url(#stage-floor)" />

      {[0, 1, 2].map((ring) => (
        <ellipse
          key={ring}
          cx={CX}
          cy={CY - 24}
          rx={62 + ring * (34 + room * 18)}
          ry={38 + ring * (20 + room * 12)}
          className="stage-ring"
          style={{ opacity: 0.1 + room * 0.12 - ring * 0.02 }}
        />
      ))}

      {mode !== "off" && (
        <path
          d={`M ${CX - 86 - width * 26} ${CY - 40} Q ${CX} ${CY - 150 - width * 20} ${CX + 86 + width * 26} ${CY - 40}`}
          className="stage-arc"
        />
      )}

      {surround && (
        <ellipse
          cx={CX}
          cy={CY - 26}
          rx={160 * (0.9 + width * 0.16)}
          ry={84 * (0.85 + room * 0.3)}
          className="stage-field"
        />
      )}

      <g className="stage-head">
        <circle cx={CX} cy={CY} r={17} />
        <path d={`M ${CX - 26} ${CY + 34} Q ${CX} ${CY + 2} ${CX + 26} ${CY + 34}`} />
        <path d={`M ${CX} ${CY - 17} l -4 -7 h 8 Z`} className="stage-nose" />
      </g>

      {binaural && (
        <>
          <path
            d={`M ${CX - 20} ${CY - 20} Q ${CX - 46} ${CY - 88} ${CX - 70 - width * 20} ${CY - 70}`}
            className="stage-wave"
          />
          <path
            d={`M ${CX + 20} ${CY - 20} Q ${CX + 46} ${CY - 88} ${CX + 70 + width * 20} ${CY - 70}`}
            className="stage-wave"
          />
        </>
      )}

      {speakers.map((spec, index) => (
        <Speaker key={`${spec.label}-${index}`} spec={spec} />
      ))}
    </svg>
  );
}