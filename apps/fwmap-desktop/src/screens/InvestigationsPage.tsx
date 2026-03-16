import type { ReactNode } from "react";

export function InvestigationsPage({ toolbar, list, detail }: { toolbar: ReactNode; list: ReactNode; detail: ReactNode }) {
  return (
    <div className="page-stack">
      {toolbar}
      <section className="investigation-layout">
        <div>{list}</div>
        <div>{detail}</div>
      </section>
    </div>
  );
}
