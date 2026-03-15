import type { ReactNode } from "react";

export function FwInspectorPanel({ title, subtitle, children }: { title: string; subtitle?: string; children: ReactNode }) {
  return (
    <section className="fw-inspector-panel">
      <div className="fw-panel-heading">
        <div className="section-header">{title}</div>
        {subtitle ? <div className="section-subtitle">{subtitle}</div> : null}
      </div>
      <div className="fw-panel-body">{children}</div>
    </section>
  );
}
