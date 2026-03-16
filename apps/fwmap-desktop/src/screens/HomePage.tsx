import type { ReactNode } from "react";

export function HomePage({ children }: { children: ReactNode }) {
  return <div className="page-stack dashboard-dense-stack">{children}</div>;
}
