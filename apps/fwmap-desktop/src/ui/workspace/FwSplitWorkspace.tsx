import type { ReactNode } from "react";

export function FwSplitWorkspace({ primary, inspector }: { primary: ReactNode; inspector: ReactNode }) {
  return (
    <section className="fw-split-workspace">
      <div className="fw-split-primary">{primary}</div>
      <div className="fw-split-inspector">{inspector}</div>
    </section>
  );
}
