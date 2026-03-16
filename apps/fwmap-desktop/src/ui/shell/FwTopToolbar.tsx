import type { ReactNode } from "react";

export function FwTopToolbar({ eyebrow, title, description, chips, actions }: { eyebrow: string; title: string; description: string; chips?: ReactNode; actions?: ReactNode }) {
  return (
    <section className="stage-banner compact-stage-banner v2-stage-banner">
      <div className="stage-banner-copy">
        <div className="dashboard-kicker">{eyebrow}</div>
        <h1>{title}</h1>
        <p>{description}</p>
      </div>
      <div className="v2-stage-toolbar-side">
        {chips ? <div className="hero-chip-row compact-hero-chip-row">{chips}</div> : null}
        {actions ? <div className="button-row compact-wrap v2-stage-actions">{actions}</div> : null}
      </div>
    </section>
  );
}
