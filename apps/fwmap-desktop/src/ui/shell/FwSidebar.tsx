import type { ReactNode } from "react";

export function FwSidebar({ brand, nav, actions, context }: { brand: ReactNode; nav: ReactNode; actions?: ReactNode; context?: ReactNode }) {
  return (
    <>
      <section className="rail-brand v2-rail-brand">{brand}</section>
      <nav className="rail-nav v2-rail-nav" aria-label="Primary screens">{nav}</nav>
      {actions ? <div className="v2-sidebar-section">{actions}</div> : null}
      {context ? <div className="v2-sidebar-section">{context}</div> : null}
    </>
  );
}
