import type { ReactNode } from "react";

export function ComparePage({ children }: { children: ReactNode }) {
  return <div className="page-stack">{children}</div>;
}
