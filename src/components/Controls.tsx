import { useId, type CSSProperties, type ReactNode } from "react";

interface SliderProps {
  label?: ReactNode;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (value: number) => void;
  format?: (value: number) => string;
  disabled?: boolean;
  accent?: "cyan" | "violet" | "amber";
  className?: string;
}

export function Slider({
  label,
  value,
  min,
  max,
  step = 1,
  onChange,
  format,
  disabled,
  accent = "cyan",
  className,
}: SliderProps) {
  const id = useId();
  const percent = ((value - min) / (max - min)) * 100;
  const style = { "--fill": `${Math.min(100, Math.max(0, percent))}%` } as CSSProperties;
  return (
    <div className={`slider ${disabled ? "is-disabled" : ""} ${className ?? ""}`}>
      {(label || format) && (
        <div className="slider-head">
          <label htmlFor={id}>{label}</label>
          <span className="slider-value">{format ? format(value) : value}</span>
        </div>
      )}
      <input
        id={id}
        type="range"
        className={`range range-${accent}`}
        min={min}
        max={max}
        step={step}
        value={value}
        disabled={disabled}
        style={style}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </div>
  );
}

interface VerticalSliderProps {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (value: number) => void;
  label: string;
  sublabel: string;
  format: (value: number) => string;
}

export function VerticalSlider({
  value,
  min,
  max,
  step = 0.5,
  onChange,
  label,
  sublabel,
  format,
}: VerticalSliderProps) {
  const id = useId();
  const percent = ((value - min) / (max - min)) * 100;
  const style = { "--fill": `${Math.min(100, Math.max(0, percent))}%` } as CSSProperties;
  return (
    <div className="vslider">
      <span className="vslider-value">{format(value)}</span>
      <input
        id={id}
        type="range"
        className="range range-cyan range-vertical"
        min={min}
        max={max}
        step={step}
        value={value}
        style={style}
        aria-label={label}
        onChange={(event) => onChange(Number(event.target.value))}
        onDoubleClick={() => onChange(0)}
      />
      <label htmlFor={id} className="vslider-label">
        {label}
      </label>
      <span className="vslider-sub">{sublabel}</span>
    </div>
  );
}

interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label?: ReactNode;
  description?: ReactNode;
  disabled?: boolean;
}

export function Toggle({ checked, onChange, label, description, disabled }: ToggleProps) {
  return (
    <div className={`toggle-row ${disabled ? "is-disabled" : ""}`}>
      {(label || description) && (
        <div className="toggle-text">
          {label && <span className="toggle-label">{label}</span>}
          {description && <span className="toggle-desc">{description}</span>}
        </div>
      )}
      <button
        type="button"
        role="switch"
        aria-checked={checked}
        aria-label={typeof label === "string" ? label : undefined}
        className={`switch ${checked ? "is-on" : ""}`}
        disabled={disabled}
        onClick={() => onChange(!checked)}
      >
        <span className="switch-thumb" />
      </button>
    </div>
  );
}

export interface SegmentedOption<T extends string> {
  value: T;
  label: ReactNode;
  icon?: ReactNode;
}

interface SegmentedProps<T extends string> {
  value: T;
  options: SegmentedOption<T>[];
  onChange: (value: T) => void;
  ariaLabel?: string;
  className?: string;
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
  className,
}: SegmentedProps<T>) {
  return (
    <div className={`segmented ${className ?? ""}`} role="tablist" aria-label={ariaLabel}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="tab"
          aria-selected={value === option.value}
          className={`segment ${value === option.value ? "is-active" : ""}`}
          onClick={() => onChange(option.value)}
        >
          {option.icon}
          <span>{option.label}</span>
        </button>
      ))}
    </div>
  );
}