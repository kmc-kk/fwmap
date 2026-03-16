import type { ReactNode } from "react";

export function PackagesPage({ children }: { children: ReactNode }) {
  return <div className="page-stack">{children}</div>;
}
