import type { ReactNode } from "react";

export function InvestigationWorkspacePage({ header, tabs, content }: { header: ReactNode; tabs: ReactNode; content: ReactNode }) {
  return (
    <div className="page-stack">
      {header}
      {tabs}
      {content}
    </div>
  );
}
