import type { ReactNode } from "react";

export function FwFilterBar({ children, trailing }: { children: ReactNode; trailing?: ReactNode }) {
  return (
    <div className="fw-filter-bar">
      <div className="fw-filter-bar-main">{children}</div>
      {trailing ? <div className="fw-filter-bar-trailing">{trailing}</div> : null}
    </div>
  );
}
