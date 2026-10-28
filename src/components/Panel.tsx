import type { ReactNode } from "react";

interface PanelProps {
  title?: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  padding?: boolean;
}

export function Panel({
  title,
  subtitle,
  actions,
  children,
  className,
  padding = true,
}: PanelProps) {
  return (
    <section className={`panel ${padding ? "has-padding" : ""} ${className ?? ""}`}>
      {(title || actions) && (
        <header className="panel-head">
          <div className="panel-title">
            {title && <h2>{title}</h2>}
            {subtitle && <p>{subtitle}</p>}
          </div>
          {actions && <div className="panel-actions">{actions}</div>}
        </header>
      )}
      {children}
    </section>
  );
}