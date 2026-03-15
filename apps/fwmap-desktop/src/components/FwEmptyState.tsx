export function FwEmptyState({ title, detail }: { title: string; detail: string }) {
  return (
    <div className="fw-empty-state">
      <strong>{title}</strong>
      <p>{detail}</p>
    </div>
  );
}
