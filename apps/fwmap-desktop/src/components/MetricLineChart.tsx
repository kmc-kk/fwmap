import {
  CategoryScale,
  Chart as ChartJS,
  Filler,
  Legend,
  LineElement,
  LinearScale,
  PointElement,
  Tooltip,
} from "chart.js";
import { Line } from "react-chartjs-2";

import { formatBytes } from "../lib/format";
import type { TrendSeries } from "../lib/types";

ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Tooltip, Legend, Filler);

type Props = {
  title: string;
  series: TrendSeries | null | undefined;
};

export function MetricLineChart({ title, series }: Props) {
  if (!series || series.points.length === 0) {
    return <div className="chart-empty">No trend data yet.</div>;
  }

  const isBytes = series.unit === "bytes";
  const labels = series.points.map((point) => point.label);
  const hasSecondary = series.points.some((point) => point.secondaryValue != null);
  const data = {
    labels,
    datasets: [
      {
        label: series.label,
        data: series.points.map((point) => point.value),
        borderColor: "#60a5fa",
        backgroundColor: "rgba(96, 165, 250, 0.16)",
        fill: true,
        tension: 0.32,
        borderWidth: 3,
        pointRadius: 2,
      },
      ...(hasSecondary
        ? [
            {
              label: title.includes("Warning") ? "Errors" : "Secondary",
              data: series.points.map((point) => point.secondaryValue),
              borderColor: "#34d399",
              backgroundColor: "rgba(52, 211, 153, 0.12)",
              fill: true,
              tension: 0.32,
              borderWidth: 2,
              pointRadius: 2,
            },
          ]
        : []),
    ],
  };

  return (
    <Line
      data={data}
      options={{
        responsive: true,
        maintainAspectRatio: false,
        interaction: { mode: "index", intersect: false },
        plugins: {
          legend: {
            labels: { color: "#dbe7fb" },
          },
          tooltip: {
            callbacks: {
              label(context) {
                const raw = Number(context.raw ?? 0);
                return `${context.dataset.label}: ${isBytes ? formatBytes(raw) : raw}`;
              },
            },
          },
        },
        scales: {
          x: {
            ticks: { color: "#90a4c4", maxRotation: 0, autoSkip: true },
            grid: { color: "rgba(148, 163, 184, 0.08)" },
          },
          y: {
            ticks: {
              color: "#90a4c4",
              callback(value) {
                const numeric = Number(value);
                return isBytes ? formatBytes(numeric) : numeric.toString();
              },
            },
            grid: { color: "rgba(148, 163, 184, 0.08)" },
          },
        },
      }}
    />
  );
}
