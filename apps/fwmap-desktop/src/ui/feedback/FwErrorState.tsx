import { Button } from "@heroui/react";

export function FwErrorState({ title, detail, actionLabel, onRetry }: { title: string; detail: string; actionLabel?: string; onRetry?: () => void }) {
  return (
    <div className="fw-feedback-state fw-error-state">
      <strong>{title}</strong>
      <p>{detail}</p>
      {onRetry ? <Button variant="flat" color="danger" onPress={onRetry}>{actionLabel ?? "Retry"}</Button> : null}
    </div>
  );
}
