import { Spinner } from "@heroui/react";

export function FwLoadingState({ label, detail }: { label: string; detail?: string }) {
  return (
    <div className="fw-feedback-state">
      <Spinner label={label} />
      {detail ? <p>{detail}</p> : null}
    </div>
  );
}
