import { BarElement, CategoryScale, Chart as ChartJS, Legend, LinearScale, Tooltip } from "chart.js";
import { Bar } from "react-chartjs-2";

import { formatBytes } from "../lib/format";

type Item = {
  label: string;
  value: number;
};

type Props = {
  title: string;
  items: Item[];
  bytes?: boolean;
  color?: string;
};

ChartJS.register(CategoryScale, LinearScale, BarElement, Tooltip, Legend);

export function BreakdownBarChart({ title, items, bytes = true, color = "#f59e0b" }: Props) {
  if (items.length === 0) {
    return <div className="chart-empty">No {title.toLowerCase()} data yet.</div>;
  }

  return (
    <Bar
      data={{
        labels: items.map((item) => item.label),
        datasets: [
          {
            label: title,
            data: items.map((item) => item.value),
            backgroundColor: color,
            borderRadius: 8,
            maxBarThickness: 26,
          },
        ],
      }}
      options={{
        indexAxis: "y",
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: { display: false },
          tooltip: {
            callbacks: {
              label(context) {
                const value = Number(context.raw ?? 0);
                return bytes ? formatBytes(value) : `${value}`;
              },
            },
          },
        },
        scales: {
          x: {
            ticks: {
              color: "#90a4c4",
              callback(value) {
                const numeric = Number(value);
                return bytes ? formatBytes(numeric) : `${numeric}`;
              },
            },
            grid: { color: "rgba(148, 163, 184, 0.08)" },
          },
          y: {
            ticks: { color: "#dbe7fb" },
            grid: { display: false },
          },
        },
      }}
    />
  );
}
