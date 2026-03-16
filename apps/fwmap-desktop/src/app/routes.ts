export type ScreenKey = "home" | "investigations" | "runs" | "compare" | "history" | "inspector" | "plugins" | "packages" | "settings";
export type InvestigationWorkspaceTab = "overview" | "evidence" | "compare" | "timeline" | "verdict" | "package";

export type DesktopRoute =
  | { screen: "home" }
  | { screen: "investigations"; investigationId?: number | null; tab?: InvestigationWorkspaceTab }
  | { screen: "runs"; runId?: number | null }
  | { screen: "compare"; leftRunId?: number | null; rightRunId?: number | null }
  | { screen: "history" }
  | { screen: "inspector" }
  | { screen: "plugins" }
  | { screen: "packages" }
  | { screen: "settings" };

export function parseDesktopRoute(hash: string): DesktopRoute {
  const trimmed = hash.replace(/^#/, "").trim();
  const [first, second, third] = trimmed.split("/");
  switch (first) {
    case "":
    case "home":
    case "dashboard":
      return { screen: "home" };
    case "investigations": {
      const investigationId = Number(second);
      const tab = isInvestigationTab(third) ? third : undefined;
      return {
        screen: "investigations",
        investigationId: Number.isFinite(investigationId) ? investigationId : null,
        tab,
      };
    }
    case "runs": {
      const runId = Number(second);
      return { screen: "runs", runId: Number.isFinite(runId) ? runId : null };
    }
    case "compare": {
      const leftRunId = Number(second);
      const rightRunId = Number(third);
      return {
        screen: "compare",
        leftRunId: Number.isFinite(leftRunId) ? leftRunId : null,
        rightRunId: Number.isFinite(rightRunId) ? rightRunId : null,
      };
    }
    case "history":
      return { screen: "history" };
    case "inspector":
      return { screen: "inspector" };
    case "plugins":
      return { screen: "plugins" };
    case "packages":
      return { screen: "packages" };
    case "settings":
      return { screen: "settings" };
    default:
      return { screen: "home" };
  }
}

export function buildDesktopHash(route: DesktopRoute): string {
  switch (route.screen) {
    case "home":
      return "home";
    case "investigations":
      if (route.investigationId) {
        return route.tab ? `investigations/${route.investigationId}/${route.tab}` : `investigations/${route.investigationId}`;
      }
      return "investigations";
    case "runs":
      return route.runId ? `runs/${route.runId}` : "runs";
    case "compare":
      return route.leftRunId && route.rightRunId ? `compare/${route.leftRunId}/${route.rightRunId}` : "compare";
    default:
      return route.screen;
  }
}

function isInvestigationTab(value: string | undefined): value is InvestigationWorkspaceTab {
  return value === "overview" || value === "evidence" || value === "compare" || value === "timeline" || value === "verdict" || value === "package";
}
