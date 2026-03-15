import type { ReactNode } from "react";

export function FwToolbar({ children }: { children: ReactNode }) {
  return <div className="fw-toolbar">{children}</div>;
}
