import type { ReactNode } from "react";

type Props = {
  topbar: ReactNode;
  sidebar: ReactNode;
  banner: ReactNode;
  metrics?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
};

export function FwAppShell({ topbar, sidebar, banner, metrics, children, footer }: Props) {
  return (
    <div className="app-shell v2-app-shell">
      {topbar}
      <div className="app-grid wide workstation-shell v2-shell-grid">
        <aside className="sidebar operation-rail v2-sidebar">{sidebar}</aside>
        <main className="content studio-stage v2-stage">
          <div className="stage-content v2-stage-content">
            {banner}
            {metrics}
            <section className="v2-page-slot">{children}</section>
            {footer}
          </div>
        </main>
      </div>
    </div>
  );
}
