import { Chip } from "@heroui/react";
import type { ReactNode } from "react";

export function FwDetailHeader({ title, subtitle, status, chips, actions }: { title: string; subtitle?: string; status?: string; chips?: string[]; actions?: ReactNode }) {
  return (
    <div className="fw-detail-header">
      <div>
        <div className="section-header">{title}</div>
        {subtitle ? <div className="section-subtitle">{subtitle}</div> : null}
      </div>
      <div className="fw-detail-header-side">
        {status ? <Chip size="sm" variant="flat">{status}</Chip> : null}
        {chips?.map((chip) => <Chip key={chip} size="sm" variant="flat">{chip}</Chip>)}
        {actions}
      </div>
    </div>
  );
}
