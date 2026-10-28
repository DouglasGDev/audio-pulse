import type { ReactElement, SVGProps } from "react";

export type IconName =
  | "logo"
  | "dashboard"
  | "equalizer"
  | "spatial"
  | "dynamics"
  | "presets"
  | "ear"
  | "settings"
  | "power"
  | "play"
  | "stop"
  | "upload"
  | "download"
  | "trash"
  | "plus"
  | "close"
  | "volume"
  | "mute"
  | "headphone"
  | "check"
  | "refresh"
  | "sun"
  | "moon"
  | "keyboard"
  | "cable"
  | "sparkle"
  | "speaker"
  | "wave"
  | "chevron-down"
  | "arrow-right";

interface IconProps extends Omit<SVGProps<SVGSVGElement>, "name"> {
  name: IconName;
  size?: number;
}

const paths: Record<Exclude<IconName, "logo">, ReactElement> = {
  dashboard: (
    <>
      <rect x="3" y="3" width="7.5" height="7.5" rx="2" />
      <rect x="13.5" y="3" width="7.5" height="7.5" rx="2" />
      <rect x="3" y="13.5" width="7.5" height="7.5" rx="2" />
      <rect x="13.5" y="13.5" width="7.5" height="7.5" rx="2" />
    </>
  ),
  equalizer: (
    <>
      <path d="M5 4v6.2M5 14.2V20M12 4v2.2M12 10.2V20M19 4v10.2M19 18.2V20" />
      <circle cx="5" cy="12.2" r="2" />
      <circle cx="12" cy="8.2" r="2" />
      <circle cx="19" cy="16.2" r="2" />
    </>
  ),
  spatial: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M6.3 6.3a8 8 0 0 0 0 11.4M17.7 17.7a8 8 0 0 0 0-11.4" />
      <path d="M3.5 3.5a12 12 0 0 0 0 17M20.5 20.5a12 12 0 0 0 0-17" opacity=".55" />
    </>
  ),
  dynamics: (
    <>
      <path d="M3 12h2.4l1.8-5.4L10 18l2-9 1.8 6.6L15 9h1.6" />
      <path d="M19.8 7.2v9.6M21.6 9.6v4.8" />
    </>
  ),
  presets: (
    <>
      <path d="M6 3.8h12a1 1 0 0 1 1 1v15.4l-7-4.2-7 4.2V4.8a1 1 0 0 1 1-1Z" />
      <path d="M9 8h6" opacity=".6" />
    </>
  ),
  ear: (
    <>
      <path d="M5.8 8.6a6.6 6.6 0 0 1 12.9 2c0 5-4.9 5.5-4.9 8.2a2.9 2.9 0 0 1-5.8 0" />
      <path d="M9.2 9.4a2.9 2.9 0 0 1 5.7 0c0 2.6-2.4 2.9-2.4 4.9" opacity=".7" />
      <path d="M8.1 13.9a3.3 3.3 0 0 0 3.3 3.3" opacity=".5" />
    </>
  ),
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1 1.55V21a2 2 0 1 1-4 0v-.09a1.7 1.7 0 0 0-1-1.55 1.7 1.7 0 0 0-1.88.34l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-1.55-1H3a2 2 0 1 1 0-4h.09a1.7 1.7 0 0 0 1.55-1 1.7 1.7 0 0 0-.34-1.88l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-1.55V3a2 2 0 1 1 4 0v.09a1.7 1.7 0 0 0 1 1.55 1.7 1.7 0 0 0 1.88-.34l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06A1.7 1.7 0 0 0 19.4 9v.05a1.7 1.7 0 0 0 1.55 1H21a2 2 0 1 1 0 4h-.09a1.7 1.7 0 0 0-1.55 1Z" />
    </>
  ),
  power: (
    <>
      <path d="M12 3v9" />
      <path d="M6.3 6.3a9 9 0 1 0 11.4 0" />
    </>
  ),
  play: <path d="M8 5.4v13.2l10.5-6.6L8 5.4Z" />,
  stop: <rect x="6.5" y="6.5" width="11" height="11" rx="2" />,
  upload: (
    <>
      <path d="M12 16V4M7.5 8.5 12 4l4.5 4.5" />
      <path d="M4 15v3.5A1.5 1.5 0 0 0 5.5 20h13a1.5 1.5 0 0 0 1.5-1.5V15" />
    </>
  ),
  download: (
    <>
      <path d="M12 4v12M7.5 11.5 12 16l4.5-4.5" />
      <path d="M4 15v3.5A1.5 1.5 0 0 0 5.5 20h13a1.5 1.5 0 0 0 1.5-1.5V15" />
    </>
  ),
  trash: (
    <>
      <path d="M4 7h16M9 7V5.2A1.2 1.2 0 0 1 10.2 4h3.6A1.2 1.2 0 0 1 15 5.2V7" />
      <path d="M6.5 7l.8 12a1.5 1.5 0 0 0 1.5 1.4h6.4a1.5 1.5 0 0 0 1.5-1.4l.8-12" />
      <path d="M10 11v5.5M14 11v5.5" opacity=".6" />
    </>
  ),
  plus: <path d="M12 5v14M5 12h14" />,
  close: <path d="M6 6l12 12M18 6 6 18" />,
  volume: (
    <>
      <path d="M4 9.5h3.2L12 5.5v13l-4.8-4H4a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1Z" />
      <path d="M15.5 8.6a5 5 0 0 1 0 6.8M18.2 6a8.6 8.6 0 0 1 0 12" />
    </>
  ),
  mute: (
    <>
      <path d="M4 9.5h3.2L12 5.5v13l-4.8-4H4a1 1 0 0 1-1-1v-3a1 1 0 0 1 1-1Z" />
      <path d="M16 9.5l5 5M21 9.5l-5 5" />
    </>
  ),
  headphone: (
    <>
      <path d="M4.5 14v-1.5a7.5 7.5 0 0 1 15 0V14" />
      <rect x="3.2" y="13" width="4.2" height="7" rx="2" />
      <rect x="16.6" y="13" width="4.2" height="7" rx="2" />
    </>
  ),
  check: <path d="m5 12.5 4.4 4.4L19 7" />,
  refresh: (
    <>
      <path d="M20 11a8 8 0 1 0-.6 4.5" />
      <path d="M20 4.5V11h-6.2" />
    </>
  ),
  sun: (
    <>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 2v2.5M12 19.5V22M2 12h2.5M19.5 12H22M4.9 4.9 6.7 6.7M17.3 17.3l1.8 1.8M19.1 4.9l-1.8 1.8M6.7 17.3l-1.8 1.8" />
    </>
  ),
  moon: <path d="M20 14.2A8.5 8.5 0 0 1 9.8 4 8.5 8.5 0 1 0 20 14.2Z" />,
  keyboard: (
    <>
      <rect x="2.5" y="6" width="19" height="12" rx="2.5" />
      <path d="M6 10h.01M9.5 10h.01M13 10h.01M16.5 10h.01M7 14h10" />
    </>
  ),
  cable: (
    <>
      <path d="M8 3v5M16 3v5" />
      <path d="M5 8h14v2a6 6 0 0 1-6 6h-2a6 6 0 0 1-6-6V8Z" />
      <path d="M12 16v5" />
    </>
  ),
  sparkle: (
    <>
      <path d="M12 3.5 13.7 9 19 10.7 13.7 12.4 12 18l-1.7-5.6L5 10.7 10.3 9 12 3.5Z" />
      <path d="M18.5 16.5l.7 2.3 2.3.7-2.3.7-.7 2.3-.7-2.3-2.3-.7 2.3-.7.7-2.3Z" opacity=".55" />
    </>
  ),
  speaker: (
    <>
      <rect x="6" y="3" width="12" height="18" rx="3" />
      <circle cx="12" cy="9" r="2.4" />
      <circle cx="12" cy="16" r="3.4" />
    </>
  ),
  wave: (
    <>
      <path d="M3 12c2.2-4.5 4.3-4.5 6.5 0s4.3 4.5 6.5 0 2.8-4.5 5 0" />
    </>
  ),
  "chevron-down": <path d="m6 9.5 6 6 6-6" />,
  "arrow-right": <path d="M4 12h16M14 6l6 6-6 6" />,
};

export function Icon({ name, size = 20, ...rest }: IconProps) {
  if (name === "logo") {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 32 32"
        fill="none"
        aria-hidden="true"
        {...rest}
      >
        <defs>
          <linearGradient id="se-logo" x1="4" y1="28" x2="28" y2="4" gradientUnits="userSpaceOnUse">
            <stop stopColor="var(--accent)" />
            <stop offset="1" stopColor="var(--accent-2)" />
          </linearGradient>
        </defs>
        <circle cx="16" cy="16" r="13" stroke="url(#se-logo)" strokeWidth="1.6" opacity=".45" />
        <path
          d="M9 18.6c1.6-5.4 3.5-7.8 5.2-4.6 1.4 2.6 2.6 5.7 4.1 3.3 1-1.6 2-3.2 4.7-3.2"
          stroke="url(#se-logo)"
          strokeWidth="2.4"
          strokeLinecap="round"
        />
        <circle cx="16" cy="16" r="13" stroke="url(#se-logo)" strokeWidth="6" opacity=".07" />
      </svg>
    );
  }

  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...rest}
    >
      {paths[name]}
    </svg>
  );
}