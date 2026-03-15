import type { ReactNode } from "react";

type Column = {
  key: string;
  label: string;
};

export function FwDataTable({ columns, children }: { columns: Column[]; children: ReactNode }) {
  return (
    <div className="fw-table-shell">
      <table className="data-table fw-data-table">
        <thead>
          <tr>{columns.map((column) => <th key={column.key}>{column.label}</th>)}</tr>
        </thead>
        <tbody>{children}</tbody>
      </table>
    </div>
  );
}
