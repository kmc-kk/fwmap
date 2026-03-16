import { useEffect, useMemo, useState } from "react";
import { Button, Card, CardBody, CardHeader, Chip, Input, Navbar, NavbarBrand, Spinner, Tab, Tabs, Textarea } from "@heroui/react";
import { open } from "@tauri-apps/plugin-dialog";

import { BreakdownBarChart } from "./components/BreakdownBarChart";
import { FwDataTable } from "./components/FwDataTable";
import { FwEmptyState } from "./components/FwEmptyState";
import { FwInspectorPanel } from "./components/FwInspectorPanel";
import { FwToolbar } from "./components/FwToolbar";
import { InspectorPanel } from "./components/InspectorPanel";
import { MetricLineChart } from "./components/MetricLineChart";
import { parseDesktopRoute, buildDesktopHash, type InvestigationWorkspaceTab } from "./app/routes";
import { FwCommandPalette } from "./ui/overlays/FwCommandPalette";
import { FwSearchField } from "./ui/forms/FwSearchField";
import { FwFilterBar } from "./ui/forms/FwFilterBar";
import { FwLoadingState } from "./ui/feedback/FwLoadingState";
import { FwErrorState } from "./ui/feedback/FwErrorState";
import { FwTimeline } from "./ui/workspace/FwTimeline";
import { FwVerdictEditor } from "./ui/workspace/FwVerdictEditor";
import {
  addInvestigationEvidence,
  addInvestigationNote,
  cancelJob,
  compareRuns,
  createInvestigation,
  createProject,
  deleteInvestigation,
  deleteProject,
  detectRegression,
  createInvestigationPackage,
  exportInvestigationPackage,
  exportReport,
  getActiveProject,
  getAppInfo,
  getDashboardSummary,
  getInvestigation,
  getRangeDiff,
  getPluginDetail,
  getRunDetail,
  getSettings,
  listExtensionPoints,
  listInvestigations,
  listPlugins,
  listProjects,
  listRecentExports,
  listRecentPackages,
  loadPolicy,
  listInvestigationTimeline,
  getTimeline,
  listBranches,
  listHistory,
  listRecentRuns,
  openInvestigationPackage,
  removeInvestigationEvidence,
  runPlugin,
  savePolicy,
  setActiveProject,
  setInvestigationVerdict,
  setPluginEnabled,
  listTags,
  saveSettings,
  startAnalysis,
  updateInvestigation,
  updateInvestigationNote,
  updateProject,
  validatePolicy,
} from "./lib/api";
import { listenToJobEvents } from "./lib/events";
import { formatBytes, formatTime, joinParts } from "./lib/format";
import type {
  ActiveProjectState,
  AddInvestigationEvidenceRequest,
  AddInvestigationNoteRequest,
  AnalysisRequest,
  CreateInvestigationRequest,
  CreateInvestigationPackageRequest,
  CreateProjectRequest,
  DashboardSummary,
  ExportInvestigationPackageRequest,
  InspectorQuery,
  InspectorSelection,
  DesktopAppInfo,
  DesktopSettings,
  ExportRequest,
  ExtensionPoint,
  ExportResult,
  GitRef,
  InvestigationPackageSummary,
  HistoryItem,
  HistoryQuery,
  InvestigationDetail,
  InvestigationNote,
  InvestigationSummary,
  InvestigationVerdict,
  JobEvent,
  JobStatus,
  OpenInvestigationPackageResult,
  PluginDetail,
  PluginExecutionResult,
  PluginSummary,
  PolicyDocument,
  PolicyValidationResult,
  ProjectDetail,
  ProjectSummary,
  RecentExport,
  RangeDiffQuery,
  RangeDiffResult,
  RegressionQuery,
  RegressionResult,
  RunCompareResult,
  RunDetail,
  RunSummary,
  TimelineResult,
  UpdateInvestigationRequest,
} from "./lib/types";

type ScreenKey = "dashboard" | "investigations" | "runs" | "diff" | "history" | "inspector" | "plugins" | "packages" | "settings";

const emptyRequest: AnalysisRequest = {
  elfPath: null,
  mapPath: null,
  debugPath: null,
  ruleFilePath: null,
  gitRepoPath: null,
  label: null,
};

const emptySettings: DesktopSettings = {
  historyDbPath: "",
  defaultRuleFilePath: null,
  defaultGitRepoPath: null,
  lastElfPath: null,
  lastMapPath: null,
};

const defaultHistoryQuery: HistoryQuery = {
  repoPath: null,
  branch: null,
  profile: null,
  toolchain: null,
  target: null,
  limit: 30,
  order: "ancestry",
};

const defaultRangeQuery: RangeDiffQuery = {
  repoPath: null,
  spec: "HEAD~20..HEAD",
  includeChangedFiles: true,
  order: "ancestry",
  profile: null,
  toolchain: null,
  target: null,
};

const emptyProjectDraft: CreateProjectRequest = {
  name: "",
  rootPath: "",
  gitRepoPath: null,
  defaultElfPath: null,
  defaultMapPath: null,
  defaultDebugPath: null,
  defaultRuleFilePath: null,
  defaultTarget: null,
  defaultProfile: null,
  defaultExportDir: null,
};

const emptyPolicyDocument: PolicyDocument = {
  path: null,
  format: "toml",
  content: "",
  projectId: null,
};

const defaultExportRequest: ExportRequest = {
  exportTarget: "dashboard",
  format: "html",
  destinationPath: "",
  projectId: null,
  runId: null,
  compare: null,
  historyQuery: null,
  rangeQuery: null,
  regressionQuery: null,
  dashboardQuery: null,
  title: null,
};

const defaultPackageRequest: CreateInvestigationPackageRequest = {
  projectId: null,
  packageName: "",
  destinationPath: "",
  sourceContext: "dashboard",
  includeSections: ["dashboard"],
  includeChartsSnapshot: true,
  includePolicySnapshot: false,
  includePluginResults: true,
  includeNotes: false,
  notes: null,
  runId: null,
  compare: null,
  historyQuery: null,
  rangeQuery: null,
  regressionQuery: null,
  dashboardQuery: null,
  inspectorQuery: null,
  inspectorSelection: null,
};


const emptyInvestigationRef = {
  kind: "run",
  runId: null,
  buildId: null,
  commit: null,
  label: "Not set",
};

const emptyInvestigationDraft: CreateInvestigationRequest = {
  title: "",
  projectId: null,
  workspaceId: null,
  baselineRef: emptyInvestigationRef,
  targetRef: emptyInvestigationRef,
  status: "open",
};

const emptyVerdictDraft: Omit<InvestigationVerdict, "investigationId" | "updatedAt"> = {
  verdictType: "unknown",
  confidence: 0.5,
  summary: "",
  supportingEvidenceIds: [],
  unresolvedQuestions: "",
  nextActions: "",
};

const defaultInvestigationPackageExport: ExportInvestigationPackageRequest = {
  investigationId: 0,
  packageName: "",
  destinationPath: "",
  includeNotes: true,
  includeTimeline: true,
  includeVerdict: true,
  includeEvidenceSnapshots: true,
};

const defaultInspectorQuery: InspectorQuery = {
  runId: null,
  buildId: null,
  leftRunId: null,
  rightRunId: null,
  viewMode: "region-section",
  groupBy: "section",
  metric: "size",
  search: null,
  topN: 24,
  thresholdMin: null,
  onlyIncreased: false,
  onlyDecreased: false,
  debugInfoOnly: false,
};

const defaultRegressionQuery: RegressionQuery = {
  repoPath: null,
  spec: "HEAD~50..HEAD",
  detectorType: "metric",
  key: "rom_total",
  mode: "first-crossing",
  threshold: 1024,
  thresholdPercent: null,
  jumpThreshold: null,
  order: "ancestry",
  includeEvidence: true,
  includeChangedFiles: true,
  bisectLike: false,
  maxSteps: 8,
  limitCommits: null,
  profile: null,
  toolchain: null,
  target: null,
};

export default function App() {
  const [screen, setScreen] = useState<ScreenKey>("dashboard");
  const [appInfo, setAppInfo] = useState<DesktopAppInfo | null>(null);
  const [settings, setSettings] = useState<DesktopSettings>(emptySettings);
  const [draftSettings, setDraftSettings] = useState<DesktopSettings>(emptySettings);
  const [request, setRequest] = useState<AnalysisRequest>(emptyRequest);
  const [job, setJob] = useState<JobStatus | null>(null);
  const [runs, setRuns] = useState<RunSummary[]>([]);
  const [selectedRunId, setSelectedRunId] = useState<number | null>(null);
  const [runDetail, setRunDetail] = useState<RunDetail | null>(null);
  const [historyFilters, setHistoryFilters] = useState<HistoryQuery>(defaultHistoryQuery);
  const [historyItems, setHistoryItems] = useState<HistoryItem[]>([]);
  const [timeline, setTimeline] = useState<TimelineResult | null>(null);
  const [dashboardSummary, setDashboardSummary] = useState<DashboardSummary | null>(null);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [activeProjectState, setActiveProjectState] = useState<ActiveProjectState | null>(null);
  const [projectDraft, setProjectDraft] = useState<CreateProjectRequest>(emptyProjectDraft);
  const [policyDocument, setPolicyDocument] = useState<PolicyDocument>(emptyPolicyDocument);
  const [policyValidation, setPolicyValidation] = useState<PolicyValidationResult | null>(null);
  const [exportDraft, setExportDraft] = useState<ExportRequest>(defaultExportRequest);
  const [recentExports, setRecentExports] = useState<RecentExport[]>([]);
  const [plugins, setPlugins] = useState<PluginSummary[]>([]);
  const [extensionPoints, setExtensionPoints] = useState<ExtensionPoint[]>([]);
  const [selectedPluginId, setSelectedPluginId] = useState<string | null>(null);
  const [pluginDetail, setPluginDetail] = useState<PluginDetail | null>(null);
  const [pluginExecution, setPluginExecution] = useState<PluginExecutionResult | null>(null);
  const [packageDraft, setPackageDraft] = useState<CreateInvestigationPackageRequest>(defaultPackageRequest);
  const [recentPackages, setRecentPackages] = useState<InvestigationPackageSummary[]>([]);
  const [openedPackage, setOpenedPackage] = useState<OpenInvestigationPackageResult | null>(null);
  const [investigations, setInvestigations] = useState<InvestigationSummary[]>([]);
  const [selectedInvestigationId, setSelectedInvestigationId] = useState<number | null>(null);
  const [investigationDetail, setInvestigationDetail] = useState<InvestigationDetail | null>(null);
  const [investigationDraft, setInvestigationDraft] = useState<CreateInvestigationRequest>(emptyInvestigationDraft);
  const [investigationNoteDraft, setInvestigationNoteDraft] = useState<string>("");
  const [investigationVerdictDraft, setInvestigationVerdictDraft] = useState<Omit<InvestigationVerdict, "investigationId" | "updatedAt">>(emptyVerdictDraft);
  const [investigationExportDraft, setInvestigationExportDraft] = useState<ExportInvestigationPackageRequest>(defaultInvestigationPackageExport);
  const [showArchivedInvestigations, setShowArchivedInvestigations] = useState(false);
  const [investigationSearch, setInvestigationSearch] = useState("");
  const [investigationStatusFilter, setInvestigationStatusFilter] = useState<"all" | "open" | "archived">("all");
  const [investigationWorkspaceTab, setInvestigationWorkspaceTab] = useState<InvestigationWorkspaceTab>("overview");
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [compareLeftRunId, setCompareLeftRunId] = useState<number | null>(null);
  const [compareRightRunId, setCompareRightRunId] = useState<number | null>(null);
  const [compareResult, setCompareResult] = useState<RunCompareResult | null>(null);
  const [rangeQuery, setRangeQuery] = useState<RangeDiffQuery>(defaultRangeQuery);
  const [rangeResult, setRangeResult] = useState<RangeDiffResult | null>(null);
  const [regressionQuery, setRegressionQuery] = useState<RegressionQuery>(defaultRegressionQuery);
  const [regressionResult, setRegressionResult] = useState<RegressionResult | null>(null);
  const [inspectorQuery, setInspectorQuery] = useState<InspectorQuery>(defaultInspectorQuery);
  const [inspectorSelection, setInspectorSelection] = useState<InspectorSelection | null>(null);
  const [branches, setBranches] = useState<GitRef[]>([]);
  const [tags, setTags] = useState<GitRef[]>([]);
  const [busy, setBusy] = useState(true);
  const [savingSettings, setSavingSettings] = useState(false);
  const [starting, setStarting] = useState(false);
  const [loadingHistory, setLoadingHistory] = useState(false);
  const [loadingCompare, setLoadingCompare] = useState(false);
  const [loadingRange, setLoadingRange] = useState(false);
  const [loadingRegression, setLoadingRegression] = useState(false);
  const [loadingDashboard, setLoadingDashboard] = useState(false);
  const [loadingProjects, setLoadingProjects] = useState(false);
  const [loadingPolicy, setLoadingPolicy] = useState(false);
  const [exporting, setExporting] = useState(false);
  const [loadingPlugins, setLoadingPlugins] = useState(false);
  const [loadingInvestigations, setLoadingInvestigations] = useState(false);
  const [savingInvestigation, setSavingInvestigation] = useState(false);
  const [packaging, setPackaging] = useState(false);
  const [openingPackage, setOpeningPackage] = useState(false);
  const [loadingRefs, setLoadingRefs] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const route = parseDesktopRoute(window.location.hash);
    switch (route.screen) {
      case "home":
        setScreen("dashboard");
        break;
      case "runs":
        setScreen("runs");
        if (route.runId) {
          setSelectedRunId(route.runId);
        }
        break;
      case "compare":
        setScreen("diff");
        if (route.leftRunId) {
          setCompareLeftRunId(route.leftRunId);
        }
        if (route.rightRunId) {
          setCompareRightRunId(route.rightRunId);
        }
        break;
      case "investigations":
        setScreen("investigations");
        if (route.investigationId) {
          setSelectedInvestigationId(route.investigationId);
        }
        if (route.tab) {
          setInvestigationWorkspaceTab(route.tab);
        }
        break;
      case "history":
      case "inspector":
      case "plugins":
      case "packages":
      case "settings":
        setScreen(route.screen);
        break;
    }
  }, []);

  useEffect(() => {
    let nextHash = "home";
    switch (screen) {
      case "dashboard":
        nextHash = buildDesktopHash({ screen: "home" });
        break;
      case "runs":
        nextHash = buildDesktopHash({ screen: "runs", runId: selectedRunId });
        break;
      case "investigations":
        nextHash = buildDesktopHash({ screen: "investigations", investigationId: selectedInvestigationId, tab: investigationWorkspaceTab });
        break;
      case "diff":
        nextHash = buildDesktopHash({ screen: "compare", leftRunId: compareLeftRunId, rightRunId: compareRightRunId });
        break;
      case "history":
      case "inspector":
      case "plugins":
      case "packages":
      case "settings":
        nextHash = buildDesktopHash({ screen });
        break;
    }
    window.location.hash = nextHash;
  }, [screen, selectedRunId, selectedInvestigationId, compareLeftRunId, compareRightRunId, investigationWorkspaceTab]);

  useEffect(() => {
    let disposed = false;
    async function load() {
      setBusy(true);
      try {
        const [info, loadedSettings, loadedRuns, loadedProjects, loadedActiveProject, loadedExports, loadedPlugins, loadedExtensionPoints, loadedPackages, loadedInvestigations] = await Promise.all([
          getAppInfo(),
          getSettings(),
          listRecentRuns(30, 0),
          listProjects(),
          getActiveProject(),
          listRecentExports(null, 12),
          listPlugins(),
          listExtensionPoints(),
          listRecentPackages(null, 12),
          listInvestigations(false),
        ]);
        if (disposed) return;
        const projectRepoPath = loadedActiveProject.activeProject?.gitRepoPath ?? null;
        const repoPath = projectRepoPath ?? loadedSettings.defaultGitRepoPath;
        const initialHistoryQuery = currentOr(defaultHistoryQuery, repoPath);
        setAppInfo(info);
        setSettings(loadedSettings);
        setDraftSettings(loadedSettings);
        setRequest((current) => ({
          ...current,
          elfPath: current.elfPath ?? loadedActiveProject.activeProject?.defaultElfPath ?? loadedSettings.lastElfPath,
          mapPath: current.mapPath ?? loadedActiveProject.activeProject?.defaultMapPath ?? loadedSettings.lastMapPath,
          ruleFilePath: current.ruleFilePath ?? loadedActiveProject.activeProject?.defaultRuleFilePath ?? loadedSettings.defaultRuleFilePath,
          gitRepoPath: current.gitRepoPath ?? repoPath,
        }));
        setHistoryFilters(initialHistoryQuery);
        setRangeQuery((current) => ({ ...current, repoPath }));
        setRegressionQuery((current) => ({ ...current, repoPath }));
        setProjects(loadedProjects);
        setActiveProjectState(loadedActiveProject);
        setProjectDraft(projectToDraft(loadedActiveProject.activeProject));
        setExportDraft((current) => ({ ...current, projectId: loadedActiveProject.activeProjectId, dashboardQuery: initialHistoryQuery }));
        setRecentExports(loadedExports);
        setPlugins(loadedPlugins);
        setExtensionPoints(loadedExtensionPoints);
        setSelectedPluginId((current) => current ?? loadedPlugins[0]?.pluginId ?? null);
        setRecentPackages(loadedPackages);
        setInvestigations(loadedInvestigations);
        setSelectedInvestigationId((current) => current ?? loadedInvestigations[0]?.investigationId ?? null);
        setPackageDraft((current) => ({
          ...current,
          projectId: loadedActiveProject.activeProjectId,
          dashboardQuery: initialHistoryQuery,
          destinationPath: current.destinationPath || loadedActiveProject.activeProject?.defaultExportDir || "",
        }));
        setInvestigationDraft((current) => ({ ...current, projectId: loadedActiveProject.activeProjectId }));
        setInvestigationExportDraft((current) => ({ ...current, destinationPath: current.destinationPath || loadedActiveProject.activeProject?.defaultExportDir || "" }));
        setRuns(loadedRuns);
        const fallbackRunId = loadedRuns[0]?.runId ?? null;
        setSelectedRunId((current) => current ?? fallbackRunId);
        setCompareLeftRunId((current) => current ?? fallbackRunId);
        setCompareRightRunId((current) => current ?? loadedRuns[1]?.runId ?? fallbackRunId);
        if (repoPath) {
          await refreshGitRefs(repoPath);
        }
        if (loadedActiveProject.activeProjectId) {
          const policy = await loadPolicy(loadedActiveProject.activeProjectId, null);
          setPolicyDocument(policy);
        }
        await Promise.all([refreshHistory(initialHistoryQuery), refreshTimeline(initialHistoryQuery), refreshDashboard(initialHistoryQuery)]);
      } catch (loadError) {
        setError(String(loadError));
      } finally {
        if (!disposed) {
          setBusy(false);
        }
      }
    }
    void load();
    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    if (!selectedRunId) {
      setRunDetail(null);
      return;
    }
    const runId = selectedRunId;
    let disposed = false;
    async function loadDetail() {
      try {
        const detail = await getRunDetail(runId);
        if (!disposed) setRunDetail(detail);
      } catch (loadError) {
        if (!disposed) setError(String(loadError));
      }
    }
    void loadDetail();
    return () => {
      disposed = true;
    };
  }, [selectedRunId]);

  useEffect(() => {
    if (!selectedInvestigationId) {
      setInvestigationDetail(null);
      return;
    }
    const investigationId = selectedInvestigationId;
    let disposed = false;
    async function loadInvestigationDetail() {
      try {
        const detail = await getInvestigation(investigationId);
        if (disposed) return;
        setInvestigationDetail(detail);
        setInvestigationVerdictDraft(detail.verdict ?? emptyVerdictDraft);
        setInvestigationExportDraft((current) => ({
          ...current,
          investigationId,
          packageName: current.packageName || `${detail.summary.title.replace(/\s+/g, "-").toLowerCase()}-bundle`,
        }));
      } catch (loadError) {
        if (!disposed) setError(String(loadError));
      }
    }
    void loadInvestigationDetail();
    return () => {
      disposed = true;
    };
  }, [selectedInvestigationId]);

  useEffect(() => {
    if (!selectedPluginId) {
      setPluginDetail(null);
      return;
    }
    const pluginId = selectedPluginId;
    let disposed = false;
    async function loadPluginDetail() {
      try {
        const detail = await getPluginDetail(pluginId);
        if (!disposed) setPluginDetail(detail);
      } catch (loadError) {
        if (!disposed) setError(String(loadError));
      }
    }
    void loadPluginDetail();
    return () => {
      disposed = true;
    };
  }, [selectedPluginId]);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setCommandPaletteOpen((current) => !current);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    void refreshInvestigations(selectedInvestigationId);
  }, [showArchivedInvestigations]);

  useEffect(() => {
    let unlisteners: Array<() => void> = [];
    void listenToJobEvents({
      onCreated: handleJobEvent,
      onProgress: handleJobEvent,
      onFinished: handleJobEvent,
      onFailed: handleJobEvent,
    }).then((items) => {
      unlisteners = items;
    });
    return () => {
      for (const dispose of unlisteners) dispose();
    };
  }, [runs, request.label]);

  function handleJobEvent(event: JobEvent) {
    setJob((current) => ({
      jobId: event.jobId,
      status: event.status,
      createdAt: current?.createdAt ?? new Date().toISOString(),
      updatedAt: new Date().toISOString(),
      label: current?.label ?? request.label,
      progressMessage: event.message,
      errorMessage: event.errorMessage,
      runId: event.runId,
    }));
    if (event.status === "finished") {
      setNote("Analysis finished. Dashboard and history were refreshed.");
      void refreshRuns(event.runId ?? undefined);
      void refreshHistory(historyFilters);
      void refreshTimeline(historyFilters);
      void refreshDashboard(historyFilters);
    }
    if (event.status === "failed") {
      setError(event.errorMessage ?? "Analysis failed.");
    }
  }

  async function refreshRuns(preferredRunId?: number) {
    const loadedRuns = await listRecentRuns(30, 0);
    setRuns(loadedRuns);
    const nextRunId = preferredRunId ?? selectedRunId ?? loadedRuns[0]?.runId ?? null;
    setSelectedRunId(nextRunId);
    if (nextRunId) {
      const detail = await getRunDetail(nextRunId);
      setRunDetail(detail);
    }
  }

  async function refreshGitRefs(repoPath?: string | null) {
    if (!repoPath) {
      setBranches([]);
      setTags([]);
      return;
    }
    setLoadingRefs(true);
    try {
      const [branchItems, tagItems] = await Promise.all([listBranches(repoPath), listTags(repoPath)]);
      setBranches(branchItems);
      setTags(tagItems);
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingRefs(false);
    }
  }

  async function refreshHistory(query: HistoryQuery) {
    setLoadingHistory(true);
    try {
      setHistoryItems(await listHistory(query));
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingHistory(false);
    }
  }

  async function refreshTimeline(query: HistoryQuery) {
    setLoadingHistory(true);
    try {
      setTimeline(await getTimeline(query));
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingHistory(false);
    }
  }

  async function refreshDashboard(query: HistoryQuery) {
    setLoadingDashboard(true);
    try {
      setDashboardSummary(await getDashboardSummary(query));
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingDashboard(false);
    }
  }

  async function refreshProjects() {
    setLoadingProjects(true);
    try {
      const [items, active, exports] = await Promise.all([listProjects(), getActiveProject(), listRecentExports(null, 12)]);
      setProjects(items);
      setActiveProjectState(active);
      setProjectDraft(projectToDraft(active.activeProject));
      setRecentExports(exports);
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingProjects(false);
    }
  }

  async function handleSelectProject(projectId: number | null) {
    try {
      const active = await setActiveProject(projectId);
      setActiveProjectState(active);
      setProjectDraft(projectToDraft(active.activeProject));
      if (active.activeProject?.gitRepoPath) {
        setRequest((current) => ({
          ...current,
          elfPath: current.elfPath ?? active.activeProject?.defaultElfPath ?? null,
          mapPath: current.mapPath ?? active.activeProject?.defaultMapPath ?? null,
          ruleFilePath: active.activeProject?.defaultRuleFilePath ?? current.ruleFilePath,
          gitRepoPath: active.activeProject?.gitRepoPath ?? current.gitRepoPath,
        }));
        const nextHistory = currentOr(historyFilters, active.activeProject.gitRepoPath);
        setHistoryFilters(nextHistory);
        setRangeQuery((current) => ({ ...current, repoPath: active.activeProject?.gitRepoPath ?? current.repoPath }));
        setRegressionQuery((current) => ({ ...current, repoPath: active.activeProject?.gitRepoPath ?? current.repoPath }));
        await refreshGitRefs(active.activeProject.gitRepoPath);
        await Promise.all([refreshHistory(nextHistory), refreshTimeline(nextHistory), refreshDashboard(nextHistory)]);
      }
      const policy = await loadPolicy(active.activeProjectId, null);
      setPolicyDocument(policy);
      setPolicyValidation(null);
      setExportDraft((current) => ({ ...current, projectId: active.activeProjectId }));
      setNote(active.activeProject ? `Switched to project ${active.activeProject.name}.` : "Cleared active project.");
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleCreateProject() {
    if (!projectDraft.name.trim() || !projectDraft.rootPath.trim()) {
      setError("Project name and root path are required.");
      return;
    }
    try {
      const project = await createProject(projectDraft);
      await refreshProjects();
      await handleSelectProject(project.projectId);
      setScreen("settings");
      setNote(`Created project ${project.name}.`);
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleSaveProject() {
    if (!activeProjectState?.activeProjectId) {
      await handleCreateProject();
      return;
    }
    try {
      const project = await updateProject(activeProjectState.activeProjectId, projectDraft as unknown as Partial<ProjectDetail>);
      await refreshProjects();
      setProjectDraft(projectToDraft(project));
      setNote(`Saved project ${project.name}.`);
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleDeleteProject() {
    if (!activeProjectState?.activeProjectId) return;
    try {
      await deleteProject(activeProjectState.activeProjectId);
      setProjectDraft(emptyProjectDraft);
      setPolicyDocument(emptyPolicyDocument);
      await refreshProjects();
      setNote("Project deleted.");
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleLoadPolicy() {
    setLoadingPolicy(true);
    try {
      const policy = await loadPolicy(activeProjectState?.activeProjectId ?? null, policyDocument.path);
      setPolicyDocument(policy);
      setPolicyValidation(null);
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingPolicy(false);
    }
  }

  async function handleValidatePolicy() {
    try {
      setPolicyValidation(await validatePolicy(policyDocument));
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleSavePolicy() {
    try {
      const saved = await savePolicy({ ...policyDocument, projectId: activeProjectState?.activeProjectId ?? null });
      setPolicyDocument(saved);
      setPolicyValidation(await validatePolicy(saved));
      await refreshProjects();
      setNote("Policy saved.");
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleExport() {
    if (!exportDraft.destinationPath.trim()) {
      setError("Export destination path is required.");
      return;
    }
    setExporting(true);
    try {
      const request: ExportRequest = {
        ...exportDraft,
        projectId: activeProjectState?.activeProjectId ?? null,
        runId: exportDraft.runId ?? selectedRunId,
        compare: exportDraft.exportTarget === "diff" && compareLeftRunId && compareRightRunId ? { leftRunId: compareLeftRunId, rightRunId: compareRightRunId } : exportDraft.compare,
        historyQuery: exportDraft.exportTarget === "history" ? historyFilters : exportDraft.historyQuery,
        rangeQuery: exportDraft.exportTarget === "history" && rangeResult ? rangeQuery : exportDraft.rangeQuery,
        regressionQuery: exportDraft.exportTarget === "regression" ? regressionQuery : exportDraft.regressionQuery,
        dashboardQuery: exportDraft.exportTarget === "dashboard" ? historyFilters : exportDraft.dashboardQuery,
      };
      const result = await exportReport(request);
      setNote(`Exported ${result.exportTarget} to ${result.destinationPath}.`);
      setRecentExports(await listRecentExports(activeProjectState?.activeProjectId ?? null, 12));
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setExporting(false);
    }
  }

  async function refreshPluginData() {
    setLoadingPlugins(true);
    try {
      const [loadedPlugins, loadedExtensionPoints] = await Promise.all([listPlugins(), listExtensionPoints()]);
      setPlugins(loadedPlugins);
      setExtensionPoints(loadedExtensionPoints);
      if (selectedPluginId) {
        setPluginDetail(await getPluginDetail(selectedPluginId));
      }
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingPlugins(false);
    }
  }

  async function refreshInvestigations(preferredId?: number | null) {
    setLoadingInvestigations(true);
    try {
      const items = await listInvestigations(showArchivedInvestigations);
      setInvestigations(items);
      const nextId = preferredId ?? selectedInvestigationId ?? items[0]?.investigationId ?? null;
      setSelectedInvestigationId(nextId);
      if (nextId) {
        setInvestigationDetail(await getInvestigation(nextId));
      }
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingInvestigations(false);
    }
  }

  function buildRunRef(runId: number | null): CreateInvestigationRequest["baselineRef"] {
    const run = runs.find((item) => item.runId === runId);
    return {
      kind: "run",
      runId,
      buildId: run?.buildId ?? null,
      commit: run?.gitRevision ?? null,
      label: run ? `${run.label || `run #${run.runId}`} / ${formatBytes(run.romBytes)} ROM` : "Not set",
    };
  }

  async function handleCreateInvestigation() {
    if (!investigationDraft.title.trim()) {
      setError("Investigation title is required.");
      return;
    }
    setSavingInvestigation(true);
    try {
      const detail = await createInvestigation({
        ...investigationDraft,
        projectId: activeProjectState?.activeProjectId ?? investigationDraft.projectId,
      });
      setScreen("investigations");
      setSelectedInvestigationId(detail.summary.investigationId);
      setInvestigationDetail(detail);
      await refreshInvestigations(detail.summary.investigationId);
      setInvestigationDraft((current) => ({ ...current, title: "" }));
      setNote(`Created investigation ${detail.summary.title}.`);
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setSavingInvestigation(false);
    }
  }

  async function handleCreateInvestigationFromDiff() {
    if (!compareLeftRunId || !compareRightRunId) {
      setError("Choose both runs before creating an investigation.");
      return;
    }
    try {
      setInvestigationDraft((current) => ({
        ...current,
        title: current.title || `Diff ${compareLeftRunId} to ${compareRightRunId}`,
        projectId: activeProjectState?.activeProjectId ?? null,
        baselineRef: buildRunRef(compareLeftRunId),
        targetRef: buildRunRef(compareRightRunId),
        status: "open",
      }));
      const detail = await createInvestigation({
        title: investigationDraft.title || `Diff ${compareLeftRunId} to ${compareRightRunId}`,
        projectId: activeProjectState?.activeProjectId ?? null,
        workspaceId: null,
        baselineRef: buildRunRef(compareLeftRunId),
        targetRef: buildRunRef(compareRightRunId),
        status: "open",
      });
      if (compareResult) {
        const topItems = compareResult.sectionDeltas.slice(0, 3);
        for (const item of topItems) {
          await addInvestigationEvidence(detail.summary.investigationId, {
            evidenceType: "Section Diff",
            title: item.name,
            delta: item.delta,
            severity: item.delta > 0 ? "warning" : "info",
            confidence: 0.8,
            sourceView: "diff",
            linkedView: "diff",
            stableRef: { section: item.name, leftRunId: compareResult.leftRun.runId, rightRunId: compareResult.rightRun.runId },
            snapshot: { delta: item.delta, metric: "size", kind: "section" },
          });
        }
      }
      await refreshInvestigations(detail.summary.investigationId);
      setScreen("investigations");
      setNote(`Created investigation ${detail.summary.title} from the current diff.`);
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handlePinRegressionEvidence() {
    if (!selectedInvestigationId || !regressionResult) {
      setError("Load a regression result and select an investigation first.");
      return;
    }
    try {
      await addInvestigationEvidence(selectedInvestigationId, {
        evidenceType: "Regression Candidate",
        title: regressionResult.firstObservedBad?.subject ?? regressionQuery.key,
        delta: regressionResult.firstObservedBad?.value ?? null,
        severity: regressionResult.confidence === "high" ? "warning" : "info",
        confidence: regressionResult.confidence === "high" ? 0.9 : regressionResult.confidence === "medium" ? 0.7 : 0.5,
        sourceView: "regression",
        linkedView: "history",
        stableRef: { key: regressionQuery.key, detectorType: regressionQuery.detectorType, commit: regressionResult.firstObservedBad?.commit ?? null },
        snapshot: { reasoning: regressionResult.reasoning, confidence: regressionResult.confidence, lastGood: regressionResult.lastGood, firstObservedBad: regressionResult.firstObservedBad },
      });
      await refreshInvestigations(selectedInvestigationId);
      setScreen("investigations");
      setNote("Pinned regression evidence to the investigation.");
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handlePinInspectorEvidence() {
    if (!selectedInvestigationId || !inspectorSelection) {
      setError("Select an investigation and an inspector item first.");
      return;
    }
    try {
      await addInvestigationEvidence(selectedInvestigationId, {
        evidenceType: "Source Line",
        title: `${inspectorSelection.kind}: ${inspectorSelection.stableId}`,
        delta: null,
        severity: "info",
        confidence: 0.75,
        sourceView: "inspector",
        linkedView: "inspector",
        stableRef: inspectorSelection,
        snapshot: { query: inspectorQuery, selection: inspectorSelection },
      });
      await refreshInvestigations(selectedInvestigationId);
      setScreen("investigations");
      setNote("Pinned inspector evidence to the investigation.");
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleAddInvestigationNote() {
    if (!selectedInvestigationId || !investigationNoteDraft.trim()) {
      setError("Select an investigation and enter a note.");
      return;
    }
    try {
      await addInvestigationNote(selectedInvestigationId, { body: investigationNoteDraft, linkedEntityType: null, linkedEntityId: null });
      setInvestigationNoteDraft("");
      await refreshInvestigations(selectedInvestigationId);
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleSaveInvestigationVerdict() {
    if (!selectedInvestigationId) {
      setError("Select an investigation first.");
      return;
    }
    try {
      await setInvestigationVerdict(selectedInvestigationId, investigationVerdictDraft);
      await refreshInvestigations(selectedInvestigationId);
      setNote("Verdict saved.");
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleArchiveInvestigation(archived: boolean) {
    if (!selectedInvestigationId) return;
    try {
      await updateInvestigation(selectedInvestigationId, { archived });
      await refreshInvestigations(selectedInvestigationId);
      setNote(archived ? "Investigation archived." : "Investigation restored.");
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleRemoveInvestigationEvidence(evidenceId: number) {
    if (!selectedInvestigationId) return;
    try {
      await removeInvestigationEvidence(selectedInvestigationId, evidenceId);
      await refreshInvestigations(selectedInvestigationId);
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleExportSelectedInvestigationPackage() {
    if (!selectedInvestigationId) {
      setError("Select an investigation first.");
      return;
    }
    if (!investigationExportDraft.destinationPath.trim() || !investigationExportDraft.packageName.trim()) {
      setError("Package name and destination are required.");
      return;
    }
    setPackaging(true);
    try {
      await exportInvestigationPackage({ ...investigationExportDraft, investigationId: selectedInvestigationId });
      await refreshInvestigations(selectedInvestigationId);
      setRecentPackages(await listRecentPackages(activeProjectState?.activeProjectId ?? null, 12));
      setNote("Investigation package exported.");
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setPackaging(false);
    }
  }

  async function handleSelectPlugin(pluginId: string) {
    setSelectedPluginId(pluginId);
    try {
      setPluginDetail(await getPluginDetail(pluginId));
      setPluginExecution(null);
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function handleTogglePlugin(pluginId: string, enabled: boolean) {
    setLoadingPlugins(true);
    try {
      const updated = await setPluginEnabled(pluginId, enabled);
      setPlugins((current) => current.map((item) => (item.pluginId === updated.pluginId ? updated : item)));
      if (selectedPluginId === updated.pluginId) {
        setPluginDetail(await getPluginDetail(updated.pluginId));
      }
      setNote(`${updated.displayName} ${enabled ? "enabled" : "disabled"}.`);
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingPlugins(false);
    }
  }

  async function handleRunPlugin(pluginId: string) {
    try {
      const result = await runPlugin(pluginId, {
        contextKind: packageDraft.sourceContext,
        runId: selectedRunId,
        leftRunId: compareLeftRunId,
        rightRunId: compareRightRunId,
        historyQuery: historyFilters,
        rangeQuery: rangeResult ? rangeQuery : null,
        regressionQuery: regressionResult ? regressionQuery : null,
        inspectorQuery,
        inspectorSelection,
        packagePath: openedPackage?.summary.packagePath ?? null,
      });
      setPluginExecution(result);
      setScreen("plugins");
    } catch (loadError) {
      setError(String(loadError));
    }
  }

  async function choosePackageDestination() {
    const value = await open({ directory: true, multiple: false });
    if (typeof value === "string") {
      setPackageDraft((current) => ({ ...current, destinationPath: value }));
      setInvestigationExportDraft((current) => ({ ...current, destinationPath: value }));
    }
  }

  async function choosePackageToOpen() {
    const value = await open({ directory: true, multiple: false });
    if (typeof value === "string") {
      await handleOpenPackage(value);
    }
  }

  async function handleCreatePackage() {
    if (!packageDraft.packageName.trim()) {
      setError("Package name is required.");
      return;
    }
    if (!packageDraft.destinationPath.trim()) {
      setError("Package destination is required.");
      return;
    }
    setPackaging(true);
    try {
      const request: CreateInvestigationPackageRequest = {
        ...packageDraft,
        projectId: activeProjectState?.activeProjectId ?? null,
        runId: packageDraft.sourceContext === "run" ? (packageDraft.runId ?? selectedRunId) : packageDraft.runId,
        compare: packageDraft.sourceContext === "diff" && compareLeftRunId && compareRightRunId ? { leftRunId: compareLeftRunId, rightRunId: compareRightRunId } : packageDraft.compare,
        historyQuery: packageDraft.sourceContext === "history" ? historyFilters : packageDraft.historyQuery,
        rangeQuery: packageDraft.sourceContext === "range" ? rangeQuery : packageDraft.rangeQuery,
        regressionQuery: packageDraft.sourceContext === "regression" ? regressionQuery : packageDraft.regressionQuery,
        dashboardQuery: packageDraft.sourceContext === "dashboard" ? historyFilters : packageDraft.dashboardQuery,
        inspectorQuery: packageDraft.sourceContext === "inspector" ? inspectorQuery : packageDraft.inspectorQuery,
        inspectorSelection: packageDraft.sourceContext === "inspector" ? inspectorSelection : packageDraft.inspectorSelection,
      };
      const summary = await createInvestigationPackage(request);
      setRecentPackages(await listRecentPackages(activeProjectState?.activeProjectId ?? null, 12));
      setNote(`Created package ${summary.packageName}.`);
      await handleOpenPackage(summary.packagePath);
      setScreen("packages");
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setPackaging(false);
    }
  }

  async function handleOpenPackage(pathValue: string) {
    setOpeningPackage(true);
    try {
      setOpenedPackage(await openInvestigationPackage(pathValue));
      setRecentPackages(await listRecentPackages(activeProjectState?.activeProjectId ?? null, 12));
      setScreen("packages");
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setOpeningPackage(false);
    }
  }

  function preparePackageFromContext(sourceContext: CreateInvestigationPackageRequest["sourceContext"]) {
    setPackageDraft((current) => ({
      ...current,
      projectId: activeProjectState?.activeProjectId ?? null,
      packageName: current.packageName || `${sourceContext}-investigation`,
      destinationPath: current.destinationPath || activeProjectState?.activeProject?.defaultExportDir || "",
      sourceContext,
      includeSections: sourceContext === "dashboard" ? ["dashboard"] : [sourceContext, "dashboard"],
      runId: sourceContext === "run" ? selectedRunId : null,
      compare: sourceContext === "diff" && compareLeftRunId && compareRightRunId ? { leftRunId: compareLeftRunId, rightRunId: compareRightRunId } : null,
      historyQuery: sourceContext === "history" ? historyFilters : null,
      rangeQuery: sourceContext === "range" ? rangeQuery : null,
      regressionQuery: sourceContext === "regression" ? regressionQuery : null,
      dashboardQuery: historyFilters,
      inspectorQuery: sourceContext === "inspector" ? inspectorQuery : null,
      inspectorSelection: sourceContext === "inspector" ? inspectorSelection : null,
    }));
    setScreen("packages");
  }

  async function handleRunCompare() {
    if (!compareLeftRunId || !compareRightRunId) return;
    setLoadingCompare(true);
    try {
      setCompareResult(await compareRuns({ leftRunId: compareLeftRunId, rightRunId: compareRightRunId }));
      setScreen("diff");
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingCompare(false);
    }
  }

  async function handleRangeDiff() {
    setLoadingRange(true);
    try {
      setRangeResult(await getRangeDiff(rangeQuery));
      setScreen("history");
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingRange(false);
    }
  }

  async function handleRegression() {
    setLoadingRegression(true);
    try {
      setRegressionResult(await detectRegression(regressionQuery));
      setScreen("history");
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoadingRegression(false);
    }
  }

  function openRunInspector(runId: number) {
    setInspectorQuery((current) => ({
      ...current,
      runId,
      buildId: null,
      leftRunId: null,
      rightRunId: null,
    }));
    setInspectorSelection(null);
    setScreen("inspector");
  }

  function openDiffInspector(leftRunId: number, rightRunId: number) {
    setInspectorQuery((current) => ({
      ...current,
      runId: null,
      buildId: null,
      leftRunId,
      rightRunId,
      metric: "delta",
    }));
    setInspectorSelection(null);
    setScreen("inspector");
  }

  function openHistoryInspector(buildId: number) {
    setInspectorQuery((current) => ({
      ...current,
      runId: null,
      buildId,
      leftRunId: null,
      rightRunId: null,
      metric: "size",
    }));
    setInspectorSelection(null);
    setScreen("inspector");
  }

  async function chooseFile(field: keyof AnalysisRequest, directory = false) {
    const value = await open({ directory, multiple: false });
    if (typeof value === "string") {
      setRequest((current) => ({ ...current, [field]: value }));
    }
  }

  async function chooseSettingsPath(field: keyof DesktopSettings, directory = false) {
    const value = await open({ directory, multiple: false });
    if (typeof value === "string") {
      setDraftSettings((current) => ({ ...current, [field]: value }));
    }
  }

  async function handleStartAnalysis() {
    setStarting(true);
    setError(null);
    setNote(null);
    try {
      const started = await startAnalysis(request);
      setJob(started);
      setNote("Analysis job started.");
      setScreen("dashboard");
    } catch (startError) {
      setError(String(startError));
    } finally {
      setStarting(false);
    }
  }

  async function handleSaveSettings() {
    setSavingSettings(true);
    setError(null);
    setNote(null);
    try {
      const saved = await saveSettings(draftSettings);
      const nextHistoryQuery = currentOr(historyFilters, saved.defaultGitRepoPath);
      setSettings(saved);
      setDraftSettings(saved);
      setHistoryFilters(nextHistoryQuery);
      await refreshGitRefs(saved.defaultGitRepoPath);
      await refreshDashboard(nextHistoryQuery);
      setNote("Settings saved.");
    } catch (saveError) {
      setError(String(saveError));
    } finally {
      setSavingSettings(false);
    }
  }

  async function handleCancelJob() {
    if (!job) return;
    try {
      const updated = await cancelJob(job.jobId);
      if (updated) {
        setJob(updated);
        setNote(updated.progressMessage);
      }
    } catch (cancelError) {
      setError(String(cancelError));
    }
  }

  const latestRun = runs[0] ?? null;
  const dashboardStats = useMemo(
    () => [
      { label: "Recent runs", value: String(runs.length) },
      { label: "Latest ROM", value: latestRun ? formatBytes(latestRun.romBytes) : "-" },
      { label: "Latest RAM", value: latestRun ? formatBytes(latestRun.ramBytes) : "-" },
      { label: "Timeline rows", value: String(timeline?.rows.length ?? 0) },
    ],
    [latestRun, runs.length, timeline?.rows.length],
  );
  const romRamTrend = dashboardSummary?.recentTrends.find((series) => series.key === "rom-ram") ?? dashboardSummary?.recentTrends[0] ?? null;
  const warningTrend = dashboardSummary?.recentTrends.find((series) => series.key === "warnings") ?? dashboardSummary?.recentTrends[1] ?? null;
  const activeProjectName = activeProjectState?.activeProject?.name ?? "No active project";
  const filteredInvestigations = useMemo(() => investigations.filter((item) => {
    const matchesSearch = !investigationSearch.trim() || `${item.title} ${item.baselineRef.label} ${item.targetRef.label}`.toLowerCase().includes(investigationSearch.trim().toLowerCase());
    const matchesStatus = investigationStatusFilter === "all"
      ? true
      : investigationStatusFilter === "archived"
        ? item.archived
        : item.status === "open" && !item.archived;
    return matchesSearch && matchesStatus;
  }), [investigationSearch, investigationStatusFilter, investigations]);
  const commandPaletteItems = useMemo(() => {
    const screenItems = [
      { id: "cmd-home", title: "Home", subtitle: "Launch surface and dashboard", group: "Navigate", onSelect: () => setScreen("dashboard") },
      { id: "cmd-investigations", title: "Investigations", subtitle: "Open the investigation list", group: "Navigate", onSelect: () => setScreen("investigations") },
      { id: "cmd-compare", title: "Compare", subtitle: "Jump to run comparison", group: "Navigate", onSelect: () => setScreen("diff") },
      { id: "cmd-history", title: "History", subtitle: "Review commit timeline and regressions", group: "Navigate", onSelect: () => setScreen("history") },
      { id: "cmd-packages", title: "Packages", subtitle: "Open package export and reopen flows", group: "Navigate", onSelect: () => setScreen("packages") },
      { id: "cmd-settings", title: "Settings", subtitle: "Project, policy, and export defaults", group: "Navigate", onSelect: () => setScreen("settings") },
    ];
    const investigationItems = investigations.slice(0, 10).map((item) => ({
      id: `investigation-${item.investigationId}`,
      title: item.title,
      subtitle: `${item.baselineRef.label} -> ${item.targetRef.label}`,
      group: "Investigations",
      onSelect: () => {
        setSelectedInvestigationId(item.investigationId);
        setInvestigationWorkspaceTab("overview");
        setScreen("investigations");
      },
    }));
    const packageItems = recentPackages.slice(0, 6).map((item) => ({
      id: `package-${item.packagePath}`,
      title: item.packageName,
      subtitle: item.packagePath,
      group: "Packages",
      onSelect: () => {
        void handleOpenPackage(item.packagePath);
        setScreen("packages");
      },
    }));
    return [...screenItems, ...investigationItems, ...packageItems];
  }, [investigations, recentPackages]);
  const screenMeta: Record<ScreenKey, { eyebrow: string; title: string; description: string }> = {
    dashboard: {
      eyebrow: "Overview",
      title: "Size posture at a glance",
      description: "Track the latest build, history pressure, and the signals worth acting on next.",
    },
    investigations: {
      eyebrow: "Investigations",
      title: "Track a regression as a case",
      description: "Set a baseline and target, pin evidence, keep notes, and carry the verdict through to export.",
    },
    runs: {
      eyebrow: "Runs",
      title: "Inspect individual analyses",
      description: "Review the latest captured runs, inspect one in detail, and send it into a comparison flow.",
    },
    diff: {
      eyebrow: "Diff",
      title: "Compare two analysis snapshots",
      description: "Contrast footprint deltas and identify which sections, objects, or symbols moved the build.",
    },
    history: {
      eyebrow: "History",
      title: "Read the timeline before a regression lands",
      description: "Filter repository history, scan commit movement, then run range and regression queries.",
    },
    inspector: {
      eyebrow: "Inspector",
      title: "Investigate the build at source depth",
      description: "Pivot the current run or diff through region, file, function, symbol, and Rust ownership views.",
    },
    plugins: {
      eyebrow: "Plugins",
      title: "Understand and control desktop extensions",
      description: "Review built-in extension points, toggle plugins safely, and run supplementary plugin outputs on demand.",
    },
    packages: {
      eyebrow: "Packages",
      title: "Bundle investigations for someone else to reopen",
      description: "Create a reusable investigation bundle, inspect what it contains, and reopen prior packages locally.",
    },
    settings: {
      eyebrow: "Workspace",
      title: "Keep the desktop predictable",
      description: "Manage project defaults, policies, and export destinations without losing local context.",
    },
  };
  const stageMetrics = useMemo(() => {
    switch (screen) {
      case "dashboard":
        return [
          { label: "Active project", value: activeProjectName, detail: activeProjectState?.activeProject?.rootPath ?? "Select a project to reuse defaults." },
          { label: "Latest run", value: latestRun ? `#${latestRun.runId}` : "No runs", detail: latestRun ? formatTime(latestRun.createdAt) : "Start an analysis to populate the timeline." },
          { label: "Signals", value: `${dashboardSummary?.recentRegressions.length ?? 0}`, detail: `${timeline?.rows.length ?? 0} timeline rows loaded` },
        ];
      case "runs":
        return [
          { label: "Run count", value: String(runs.length), detail: latestRun ? `Latest: #${latestRun.runId}` : "No runs recorded yet." },
          { label: "Selected run", value: selectedRunId ? `#${selectedRunId}` : "None", detail: runDetail?.run.gitRevision ?? runDetail?.elfPath ?? "Choose a run to inspect it." },
          { label: "Warnings", value: runDetail ? String(runDetail.warnings.length) : "-", detail: runDetail ? `${runDetail.topSymbols.length} top symbols ready` : "Run detail appears here." },
        ];
      case "diff":
        return [
          { label: "Baseline", value: compareLeftRunId ? `#${compareLeftRunId}` : "Unset", detail: "The run you compare against." },
          { label: "Candidate", value: compareRightRunId ? `#${compareRightRunId}` : "Unset", detail: "The run expected to change." },
          { label: "ROM delta", value: compareResult ? signed(compareResult.summary.romDelta) : "-", detail: compareResult ? `RAM ${signed(compareResult.summary.ramDelta)}` : "Run a comparison to populate deltas." },
        ];
      case "history":
        return [
          { label: "Repo", value: historyFilters.repoPath ?? settings.defaultGitRepoPath ?? "Not set", detail: historyFilters.branch ?? "All branches" },
          { label: "Timeline", value: `${timeline?.rows.length ?? 0}`, detail: `${historyItems.length} history items matched` },
          { label: "Range", value: rangeResult ? signed(rangeResult.cumulativeRomDelta) : "Not run", detail: rangeResult ? `RAM ${signed(rangeResult.cumulativeRamDelta)}` : "Run range diff or regression from this workspace." },
        ];
      case "inspector":
        return [
          { label: "View", value: inspectorQuery.viewMode, detail: inspectorQuery.groupBy },
          { label: "Metric", value: inspectorQuery.metric, detail: inspectorQuery.runId ? `Run #${inspectorQuery.runId}` : inspectorQuery.leftRunId && inspectorQuery.rightRunId ? `Diff #${inspectorQuery.leftRunId} -> #${inspectorQuery.rightRunId}` : "Latest run" },
          { label: "Selection", value: inspectorSelection?.kind ?? "None", detail: inspectorSelection?.stableId ?? "Choose a node from the visualization or table." },
        ];
      case "investigations":
        return [
          { label: "Open cases", value: String(investigations.length), detail: `${investigations.filter((item) => item.status === "open").length} still active` },
          { label: "Selected", value: investigationDetail?.summary.title ?? "None", detail: investigationDetail ? `${investigationDetail.evidence.length} evidence / ${investigationDetail.notes.length} notes` : "Create an investigation to start collecting evidence." },
          { label: "Verdict", value: investigationDetail?.verdict?.verdictType ?? "Unset", detail: investigationDetail?.verdict?.summary ?? "No structured conclusion saved yet." },
        ];
      case "plugins":
        return [
          { label: "Installed", value: String(plugins.length), detail: `${plugins.filter((item) => item.enabled).length} enabled` },
          { label: "Selected plugin", value: pluginDetail?.summary.displayName ?? selectedPluginId ?? "None", detail: pluginDetail?.summary.layer ?? "Pick a plugin to inspect it." },
          { label: "Extension points", value: String(extensionPoints.length), detail: extensionPoints[0]?.displayName ?? "No extension points registered." },
        ];
      case "packages":
        return [
          { label: "Recent packages", value: String(recentPackages.length), detail: recentPackages[0]?.packageName ?? "No packages created yet." },
          { label: "Open package", value: openedPackage?.summary.packageName ?? "None", detail: openedPackage?.summary.sourceContext ?? "Open a package bundle to review it." },
          { label: "Included items", value: String(openedPackage?.summary.includedCount ?? 0), detail: `${openedPackage?.summary.omittedCount ?? 0} omitted` },
        ];
      case "settings":
        return [
          { label: "Active project", value: activeProjectName, detail: activeProjectState?.activeProject?.rootPath ?? "Projects keep defaults and export locations together." },
          { label: "Policy", value: policyValidation ? (policyValidation.ok ? "Ready" : "Needs attention") : "Not checked", detail: policyDocument.path ?? "Load or create a policy file." },
          { label: "Recent exports", value: String(recentExports.length), detail: recentExports[0]?.destinationPath ?? "Exported snapshots will appear here." },
        ];
    }
  }, [
    activeProjectName,
    activeProjectState?.activeProject?.rootPath,
    compareLeftRunId,
    compareResult,
    compareRightRunId,
    dashboardSummary?.recentRegressions.length,
    historyFilters.branch,
    historyFilters.repoPath,
    historyItems.length,
    latestRun,
    policyDocument.path,
    policyValidation,
    rangeResult,
    recentExports,
    recentPackages,
    runDetail,
    inspectorQuery.groupBy,
    inspectorQuery.metric,
    inspectorQuery.runId,
    inspectorQuery.leftRunId,
    inspectorQuery.rightRunId,
    inspectorQuery.viewMode,
    inspectorSelection,
    runs.length,
    screen,
    selectedPluginId,
    selectedRunId,
    settings.defaultGitRepoPath,
    timeline?.rows.length,
    extensionPoints,
    investigations,
    investigationDetail,
    openedPackage?.summary.includedCount,
    openedPackage?.summary.omittedCount,
    openedPackage?.summary.packageName,
    openedPackage?.summary.sourceContext,
  ]);

  return (
    <div className="app-shell">
      <Navbar maxWidth="full" className="topbar">
        <NavbarBrand>
          <div>
            <div className="brand-title">fwmap desktop</div>
            <div className="brand-subtitle">Visual dashboard for binary size, history, and regressions</div>
          </div>
        </NavbarBrand>
        <div className="topbar-meta">
          <Button size="sm" variant="flat" onPress={() => setCommandPaletteOpen(true)}>Search / Jump</Button>
          <Chip variant="flat">CLI {appInfo?.cliVersion ?? "-"}</Chip>
          <Chip variant="flat">History {timeline?.repoId ? "git-aware" : "local"}</Chip>
        </div>
      </Navbar>

      <div className="app-grid wide workstation-shell">
        <aside className="sidebar operation-rail">
          <section className="rail-brand">
            <div className="rail-kicker">Command deck</div>
            <h2>fwmap studio</h2>
            <p>One place to start analysis, scan history, and keep local policy and exports aligned.</p>
          </section>

          <nav className="rail-nav" aria-label="Primary screens">
            <ScreenButton active={screen === "dashboard"} label="Dashboard" detail="Live size posture" onPress={() => setScreen("dashboard")} />
            <ScreenButton active={screen === "investigations"} label="Investigations" detail={`${investigations.length} saved cases`} onPress={() => setScreen("investigations")} />
            <ScreenButton active={screen === "runs"} label="Runs" detail={`${runs.length} captured runs`} onPress={() => setScreen("runs")} />
            <ScreenButton active={screen === "diff"} label="Diff" detail="Compare snapshots" onPress={() => setScreen("diff")} />
            <ScreenButton active={screen === "history"} label="History" detail={`${timeline?.rows.length ?? 0} timeline rows`} onPress={() => setScreen("history")} />
            <ScreenButton active={screen === "inspector"} label="Inspector" detail="Source-level drill-down" onPress={() => setScreen("inspector")} />
            <ScreenButton active={screen === "plugins"} label="Plugins" detail={`${plugins.filter((item) => item.enabled).length} active plugins`} onPress={() => setScreen("plugins")} />
            <ScreenButton active={screen === "packages"} label="Packages" detail={`${recentPackages.length} recent bundles`} onPress={() => setScreen("packages")} />
            <ScreenButton active={screen === "settings"} label="Workspace" detail="Projects, policy, exports" onPress={() => setScreen("settings")} />
          </nav>

          <Card className="sidebar-card action-panel compact-action-panel">
            <CardHeader className="section-header">Start analysis</CardHeader>
            <CardBody className="panel-stack compact-panel-stack">
              <div className="compact-input-row"><Input label="ELF path" value={request.elfPath ?? ""} onValueChange={(value) => setRequest((current) => ({ ...current, elfPath: value || null }))} /><Button size="sm" variant="flat" onPress={() => void chooseFile("elfPath")}>Browse</Button></div>
              <div className="compact-input-row"><Input label="Map path" value={request.mapPath ?? ""} onValueChange={(value) => setRequest((current) => ({ ...current, mapPath: value || null }))} /><Button size="sm" variant="flat" onPress={() => void chooseFile("mapPath")}>Browse</Button></div>
              <div className="compact-input-row"><Input label="Rule file" value={request.ruleFilePath ?? ""} onValueChange={(value) => setRequest((current) => ({ ...current, ruleFilePath: value || null }))} /><Button size="sm" variant="flat" onPress={() => void chooseFile("ruleFilePath")}>Browse</Button></div>
              <div className="compact-input-row"><Input label="Git repo" value={request.gitRepoPath ?? ""} onValueChange={(value) => setRequest((current) => ({ ...current, gitRepoPath: value || null }))} /><Button size="sm" variant="flat" onPress={() => void chooseFile("gitRepoPath", true)}>Browse</Button></div>
              <Input label="Run label" value={request.label ?? ""} onValueChange={(value) => setRequest((current) => ({ ...current, label: value || null }))} />
              <div className="button-row">
                <Button color="primary" isLoading={starting} onPress={() => void handleStartAnalysis()}>Analyze build</Button>
                <Button variant="bordered" isDisabled={!job} onPress={() => void handleCancelJob()}>Stop job</Button>
              </div>
            </CardBody>
          </Card>

          <Card className="sidebar-card muted-card">
            <CardHeader className="section-header">Repository context</CardHeader>
            <CardBody className="panel-stack compact-text">
              <div>Active project</div>
              <div className="rail-context-value">{activeProjectName}</div>
              <div className="rail-context-copy">{activeProjectState?.activeProject?.rootPath ?? "Choose a project in Workspace to reuse defaults."}</div>
              <div>Default repo</div>
              <code>{settings.defaultGitRepoPath ?? "-"}</code>
              <div className="badge-row">
                {loadingRefs ? <Chip size="sm">loading refs</Chip> : null}
                {branches.slice(0, 4).map((item) => <Chip key={item.name} size="sm" variant="flat">{item.name}</Chip>)}
              </div>
              <div className="badge-row">
                {tags.slice(0, 4).map((item) => <Chip key={item.name} size="sm" variant="flat">{item.name}</Chip>)}
              </div>
            </CardBody>
          </Card>
        </aside>

        <main className="content studio-stage">
          <section className="stage-banner compact-stage-banner">
            <div className="stage-banner-copy">
              <div className="dashboard-kicker">{screenMeta[screen].eyebrow}</div>
              <h1>{screenMeta[screen].title}</h1>
              <p>{screenMeta[screen].description}</p>
            </div>
            <div className="hero-chip-row compact-hero-chip-row">
              <Chip variant="flat">{activeProjectName}</Chip>
              <Chip variant="flat">{dashboardSummary?.latestHistoryItem?.gitRevision ?? latestRun?.gitRevision ?? "No commit"}</Chip>
              <Chip variant="flat">{dashboardSummary?.latestRun?.profile ?? latestRun?.profile ?? "profile -"}</Chip>
            </div>
          </section>

          <section className="stage-metric-strip compact-stage-metric-strip">
            {stageMetrics.map((item) => (
              <article key={item.label} className="metric-slab">
                <div className="metric-slab-label">{item.label}</div>
                <div className="metric-slab-value">{item.value}</div>
                <p>{item.detail}</p>
              </article>
            ))}
          </section>

          <Tabs selectedKey={screen} onSelectionChange={(key) => setScreen(key as ScreenKey)} variant="underlined" className="main-tabs">
            <Tab key="dashboard" title="Dashboard">
              {busy ? <div className="loading-state"><Spinner label="Loading desktop state" /></div> : (
                <div className="page-stack dashboard-dense-stack">
                  <div className="button-row compact-wrap"><Button variant="flat" onPress={() => preparePackageFromContext("dashboard")}>Bundle dashboard</Button><Button variant="flat" onPress={() => void handleRunPlugin("timeline-signal-adapter")}>Run signal adapter</Button></div>
                  <section className="stats-grid dashboard-card-grid">
                    {dashboardSummary?.overviewCards.map((item) => <Card key={item.key} className={`stat-card metric-tone-${item.tone}`}><CardBody><div className="stat-card-content"><div className="stat-label">{item.title}</div><div className="stat-value">{item.value}</div>{item.subtitle ? <div className="stat-subtitle">{item.subtitle}</div> : null}</div></CardBody></Card>)}
                    {!dashboardSummary?.overviewCards?.length ? dashboardStats.map((item) => <Card key={item.label} className="stat-card"><CardBody><div className="stat-card-content"><div className="stat-label">{item.label}</div><div className="stat-value">{item.value}</div></div></CardBody></Card>) : null}
                  </section>
                  <section className="dashboard-main-grid">
                    <Card className="feature-card feature-card-wide"><CardHeader className="feature-header"><div><div className="section-header">ROM / RAM trend</div><div className="section-subtitle">Recent analyzed builds with both footprints over time.</div></div>{loadingDashboard ? <Chip size="sm">refreshing</Chip> : null}</CardHeader><CardBody><div className="chart-frame"><MetricLineChart title="ROM / RAM" series={romRamTrend} /></div></CardBody></Card>
                    <Card className="feature-card"><CardHeader className="feature-header"><div><div className="section-header">Warning pressure</div><div className="section-subtitle">Rule warnings and errors across recent builds.</div></div></CardHeader><CardBody><div className="chart-frame compact-chart"><MetricLineChart title="Warnings" series={warningTrend} /></div></CardBody></Card>
                    <Card className="feature-card"><CardHeader className="feature-header"><div><div className="section-header">Memory regions</div><div className="section-subtitle">Current region usage from the latest analyzed build.</div></div></CardHeader><CardBody><div className="chart-frame compact-chart"><BreakdownBarChart title="Region usage" items={(dashboardSummary?.regionUsage ?? []).map((item) => ({ label: `${item.regionName} ${(item.usageRatio * 100).toFixed(0)}%`, value: item.usedBytes }))} color="#34d399" /></div></CardBody></Card>
                  </section>
                  <section className="dashboard-side-grid">
                    <Card className="feature-card"><CardHeader className="feature-header"><div><div className="section-header">Top growth contributors</div><div className="section-subtitle">Largest size movers between the latest two analyzed builds.</div></div></CardHeader><CardBody><div className="chart-frame compact-chart"><BreakdownBarChart title="Growth" items={(dashboardSummary?.topGrowthSources ?? []).map((item) => ({ label: `${item.scope}:${shorten(item.name, 28)}`, value: Math.abs(item.delta) }))} color="#f59e0b" /></div><ul className="metric-list trend-list">{(dashboardSummary?.topGrowthSources ?? []).slice(0, 6).map((item) => <li key={`${item.scope}-${item.name}`}><span>{item.scope} / {shorten(item.name, 42)}</span><strong>{signed(item.delta)}</strong></li>)}{(dashboardSummary?.topGrowthSources ?? []).length === 0 ? <li><span>No comparison baseline</span><span>-</span></li> : null}</ul></CardBody></Card>
                    <Card className="feature-card"><CardHeader className="feature-header"><div><div className="section-header">Recent regressions</div><div className="section-subtitle">Latest warning-bearing or suspicious builds surfaced from desktop history.</div></div></CardHeader><CardBody className="panel-stack compact-text">{(dashboardSummary?.recentRegressions ?? []).map((item) => <div key={`${item.commit}-${item.key}`} className="regression-row"><div className="regression-row-top"><strong>{item.commit}</strong><Chip size="sm" variant="flat" color={item.confidence === "high" ? "danger" : "warning"}>{item.confidence}</Chip></div><div>{item.subject}</div><div className="regression-meta">{item.reasoning}</div></div>)}{(dashboardSummary?.recentRegressions ?? []).length === 0 ? <div className="empty-state compact-empty">No recent regressions detected.</div> : null}</CardBody></Card>
                    <Card className="feature-card"><CardHeader className="feature-header"><div><div className="section-header">Current job</div><div className="section-subtitle">Live analysis state from the local desktop service.</div></div></CardHeader><CardBody className="panel-stack compact-text"><div>Status: <strong>{job?.status ?? "idle"}</strong></div><div>Message: {job?.progressMessage ?? "No active job"}</div><div>Updated: {formatTime(job?.updatedAt)}</div><div>Run count: {runs.length}</div></CardBody></Card>
                  </section>
                </div>
              )}
            </Tab>

            <Tab key="investigations" title="Investigations">
              <div className="page-stack">
                <Card>
                  <CardHeader className="section-header">Investigation workspace</CardHeader>
                  <CardBody className="page-stack compact-text">
                    <FwToolbar>
                      <div className="form-grid investigation-toolbar-grid">
                        <Input label="Title" value={investigationDraft.title} onValueChange={(value) => setInvestigationDraft((current) => ({ ...current, title: value }))} />
                        <Input label="Baseline" value={investigationDraft.baselineRef.label} readOnly />
                        <Input label="Target" value={investigationDraft.targetRef.label} readOnly />
                      </div>
                      <div className="button-row compact-wrap">
                        <Button variant="flat" onPress={() => setInvestigationDraft((current) => ({ ...current, baselineRef: buildRunRef(compareLeftRunId), targetRef: buildRunRef(compareRightRunId || selectedRunId) }))}>Use current diff refs</Button>
                        <Button color="primary" isLoading={savingInvestigation} onPress={() => void handleCreateInvestigation()}>New investigation</Button>
                        <Button variant="flat" onPress={() => void handleCreateInvestigationFromDiff()} isDisabled={!compareLeftRunId || !compareRightRunId}>Create from diff</Button>
                        <Button variant="flat" onPress={() => setShowArchivedInvestigations((current) => !current)}>{showArchivedInvestigations ? "Hide archived" : "Show archived"}</Button>
                      </div>
                    </FwToolbar>
                  </CardBody>
                </Card>
                <section className="investigation-layout">
                  <Card>
                    <CardHeader className="section-header">Investigation list</CardHeader>
                    <CardBody className="page-stack compact-text">
                      <FwFilterBar trailing={<Button variant="flat" onPress={() => setShowArchivedInvestigations((current) => !current)}>{showArchivedInvestigations ? "Hide archived" : "Show archived"}</Button>}>
                        <FwSearchField label="Search" placeholder="Search title, baseline, or target" value={investigationSearch} onValueChange={setInvestigationSearch} />
                        <div>
                          <label>Status</label>
                          <select className="native-select" value={investigationStatusFilter} onChange={(event) => setInvestigationStatusFilter(event.target.value as "all" | "open" | "archived")}>
                            <option value="all">all</option>
                            <option value="open">open</option>
                            <option value="archived">archived</option>
                          </select>
                        </div>
                      </FwFilterBar>
                      {loadingInvestigations ? <FwLoadingState label="Loading investigations" detail="Refreshing saved cases and workspace state." /> : filteredInvestigations.length > 0 ? (
                        <FwDataTable columns={[{ key: "title", label: "Title" }, { key: "baseline", label: "Baseline" }, { key: "target", label: "Target" }, { key: "status", label: "Status" }, { key: "evidence", label: "Evidence" }, { key: "updated", label: "Updated" }]}>
                          {filteredInvestigations.map((item) => (
                            <tr key={item.investigationId} className={selectedInvestigationId === item.investigationId ? "table-row-selected" : ""} onClick={() => setSelectedInvestigationId(item.investigationId)}>
                              <td><strong>{item.title}</strong></td>
                              <td>{shorten(item.baselineRef.label, 26)}</td>
                              <td>{shorten(item.targetRef.label, 26)}</td>
                              <td><Chip size="sm" variant="flat">{item.status}</Chip></td>
                              <td>{item.evidenceCount}</td>
                              <td>{formatTime(item.updatedAt)}</td>
                            </tr>
                          ))}
                        </FwDataTable>
                      ) : <FwEmptyState title="No investigations yet" detail="Create one from the current diff, or start a blank case and pin evidence as you go." />}
                    </CardBody>
                  </Card>
                  <Card>
                    <CardHeader className="section-header">Investigation detail</CardHeader>
                    <CardBody className="page-stack compact-text">
                      {investigationDetail ? (
                        <>
                          <div className="button-row compact-wrap">
                            <Chip variant="flat">{investigationDetail.summary.status}</Chip>
                            <Chip variant="flat">{investigationDetail.summary.baselineRef.label}</Chip>
                            <Chip variant="flat">{investigationDetail.summary.targetRef.label}</Chip>
                            <Button size="sm" variant="flat" onPress={() => void handleArchiveInvestigation(!investigationDetail.summary.archived)}>{investigationDetail.summary.archived ? "Restore" : "Archive"}</Button>
                          </div>
                          <div className="investigation-detail-grid">
                            <FwInspectorPanel title="Overview" subtitle="Quiet summary for the current case.">
                              <div className="three-column package-summary-grid">
                                <article className="metric-slab plugin-slab"><div className="metric-slab-label">Evidence</div><div className="metric-slab-value">{investigationDetail.evidence.length}</div><p>Pinned references and snapshots.</p></article>
                                <article className="metric-slab plugin-slab"><div className="metric-slab-label">Notes</div><div className="metric-slab-value">{investigationDetail.notes.length}</div><p>Working context and follow-ups.</p></article>
                                <article className="metric-slab plugin-slab"><div className="metric-slab-label">Verdict</div><div className="metric-slab-value">{investigationDetail.verdict?.verdictType ?? "unset"}</div><p>{investigationDetail.verdict?.summary ?? "No structured conclusion yet."}</p></article>
                              </div>
                            </FwInspectorPanel>
                            <FwInspectorPanel title="Evidence" subtitle="Stable references with snapshots captured from diff, regression, or inspector views.">
                              <div className="button-row compact-wrap">
                                <Button size="sm" variant="flat" onPress={() => void handlePinRegressionEvidence()} isDisabled={!regressionResult}>Pin regression</Button>
                                <Button size="sm" variant="flat" onPress={() => void handlePinInspectorEvidence()} isDisabled={!inspectorSelection}>Pin inspector item</Button>
                              </div>
                              {investigationDetail.evidence.length > 0 ? (
                                <FwDataTable columns={[{ key: "type", label: "Type" }, { key: "title", label: "Title" }, { key: "delta", label: "Delta" }, { key: "severity", label: "Severity" }, { key: "source", label: "Source" }, { key: "added", label: "Added" }, { key: "action", label: "" }]}>
                                  {investigationDetail.evidence.map((item) => (
                                    <tr key={item.evidenceId}>
                                      <td>{item.evidenceType}</td>
                                      <td>{item.title}</td>
                                      <td className={deltaTone(item.delta)}>{signedOrDash(item.delta)}</td>
                                      <td><Chip size="sm" variant="flat">{item.severity}</Chip></td>
                                      <td>{item.sourceView}</td>
                                      <td>{formatTime(item.createdAt)}</td>
                                      <td><Button size="sm" variant="light" onPress={() => void handleRemoveInvestigationEvidence(item.evidenceId)}>Remove</Button></td>
                                    </tr>
                                  ))}
                                </FwDataTable>
                              ) : <FwEmptyState title="No evidence pinned" detail="Pin section diffs, regression candidates, or inspector items to build the case." />}
                            </FwInspectorPanel>
                            <FwInspectorPanel title="Timeline" subtitle="The investigation keeps a readable log of what changed and why.">
                              {investigationDetail.timeline.length > 0 ? <FwTimeline events={investigationDetail.timeline} /> : <FwEmptyState title="No timeline events yet" detail="Events appear automatically as you pin evidence, add notes, and export packages." />}
                            </FwInspectorPanel>
                            <FwInspectorPanel title="Verdict" subtitle="Store the current conclusion and what still needs follow-up.">
                              <FwVerdictEditor value={investigationVerdictDraft} onChange={setInvestigationVerdictDraft} onSave={() => void handleSaveInvestigationVerdict()} />
                            </FwInspectorPanel>
                            <FwInspectorPanel title="Notes and package" subtitle="Capture working notes, then export a shareable bundle for someone else to reopen.">
                              <Textarea minRows={4} label="Add note" value={investigationNoteDraft} onValueChange={setInvestigationNoteDraft} />
                              <div className="button-row compact-wrap">
                                <Button variant="flat" onPress={() => void handleAddInvestigationNote()}>Add note</Button>
                                <Button variant="flat" onPress={() => void choosePackageDestination()}>Choose package folder</Button>
                              </div>
                              <Input label="Package name" value={investigationExportDraft.packageName} onValueChange={(value) => setInvestigationExportDraft((current) => ({ ...current, packageName: value }))} />
                              <Input label="Destination" value={investigationExportDraft.destinationPath} onValueChange={(value) => setInvestigationExportDraft((current) => ({ ...current, destinationPath: value }))} />
                              <div className="badge-row compact-wrap">
                                <button type="button" className={`chip-button ${investigationExportDraft.includeNotes ? "active-chip" : ""}`} onClick={() => setInvestigationExportDraft((current) => ({ ...current, includeNotes: !current.includeNotes }))}>notes</button>
                                <button type="button" className={`chip-button ${investigationExportDraft.includeTimeline ? "active-chip" : ""}`} onClick={() => setInvestigationExportDraft((current) => ({ ...current, includeTimeline: !current.includeTimeline }))}>timeline</button>
                                <button type="button" className={`chip-button ${investigationExportDraft.includeVerdict ? "active-chip" : ""}`} onClick={() => setInvestigationExportDraft((current) => ({ ...current, includeVerdict: !current.includeVerdict }))}>verdict</button>
                                <button type="button" className={`chip-button ${investigationExportDraft.includeEvidenceSnapshots ? "active-chip" : ""}`} onClick={() => setInvestigationExportDraft((current) => ({ ...current, includeEvidenceSnapshots: !current.includeEvidenceSnapshots }))}>evidence</button>
                              </div>
                              <Button color="primary" isLoading={packaging} onPress={() => void handleExportSelectedInvestigationPackage()}>Export package</Button>
                              {investigationDetail.notes.length > 0 ? <ul className="warning-list">{investigationDetail.notes.map((item) => <li key={item.noteId}>{item.body}</li>)}</ul> : null}
                            </FwInspectorPanel>
                          </div>
                        </>
                      ) : <FwEmptyState title="No investigation selected" detail="Select an investigation from the list, or create one from the current diff." />}
                    </CardBody>
                  </Card>
                </section>
              </div>
            </Tab>

            <Tab key="runs" title="Runs">
              <div className="runs-layout">
                <Card><CardHeader className="section-header">Recent Runs</CardHeader><CardBody className="list-stack">{runs.map((run) => <button key={run.runId} className={`run-row ${selectedRunId === run.runId ? "selected" : ""}`} onClick={() => setSelectedRunId(run.runId)} type="button"><div className="run-row-top"><strong>#{run.runId}</strong><Chip size="sm" variant="flat">{run.status}</Chip></div><div>{run.label || run.gitRevision || "Unnamed run"}</div><div className="run-meta"><span>{formatTime(run.createdAt)}</span><span>{formatBytes(run.romBytes)} ROM</span><span>{formatBytes(run.ramBytes)} RAM</span></div></button>)}{runs.length === 0 ? <div className="empty-state">No runs recorded yet.</div> : null}</CardBody></Card>
                <Card><CardHeader className="section-header">Run Detail</CardHeader><CardBody className="page-stack compact-text">{runDetail ? <><div className="detail-grid"><div><strong>ELF</strong><br />{runDetail.elfPath || "-"}</div><div><strong>Arch</strong><br />{runDetail.arch || "-"}</div><div><strong>Linker</strong><br />{joinParts([runDetail.linkerFamily, runDetail.mapFormat]) || "-"}</div><div><strong>Git</strong><br />{joinParts([runDetail.run.gitRevision, runDetail.gitBranch, runDetail.gitDescribe]) || "-"}</div></div><div className="button-row"><Button size="sm" variant="flat" onPress={() => setCompareLeftRunId(runDetail.run.runId)}>Use as left</Button><Button size="sm" variant="flat" onPress={() => setCompareRightRunId(runDetail.run.runId)}>Use as right</Button><Button size="sm" color="primary" variant="flat" onPress={() => setScreen("diff")}>Open diff</Button><Button size="sm" variant="flat" onPress={() => openRunInspector(runDetail.run.runId)}>Inspect run</Button></div><MetricList title="Top sections" items={runDetail.topSections.map(([name, value]) => ({ name, value }))} /><MetricList title="Top symbols" items={runDetail.topSymbols.map(([name, value]) => ({ name, value }))} /><div><strong>Rule warnings</strong><ul className="warning-list">{runDetail.warnings.length === 0 ? <li>No rule warnings recorded.</li> : null}{runDetail.warnings.slice(0, 8).map(([code, level, related], index) => <li key={`${code}-${index}`}>{level} / {code}{related ? ` / ${related}` : ""}</li>)}</ul></div></> : <div className="empty-state">Select a run to inspect it.</div>}</CardBody></Card>
              </div>
            </Tab>

            <Tab key="diff" title="Diff">
              <div className="page-stack">
                <Card><CardHeader className="section-header">Run Compare</CardHeader><CardBody className="form-grid"><div><label>Left run</label><select className="native-select" value={compareLeftRunId ?? ""} onChange={(event) => setCompareLeftRunId(Number(event.target.value) || null)}>{runs.map((run) => <option key={run.runId} value={run.runId}>#{run.runId} {run.label || run.gitRevision || "run"}</option>)}</select></div><div><label>Right run</label><select className="native-select" value={compareRightRunId ?? ""} onChange={(event) => setCompareRightRunId(Number(event.target.value) || null)}>{runs.map((run) => <option key={run.runId} value={run.runId}>#{run.runId} {run.label || run.gitRevision || "run"}</option>)}</select></div><div className="button-row"><Button color="primary" isLoading={loadingCompare} onPress={() => void handleRunCompare()}>Compare runs</Button></div></CardBody></Card>
                {compareResult ? <div className="two-column"><Card><CardHeader className="section-header">Summary</CardHeader><CardBody className="panel-stack compact-text"><div>Left: {compareResult.leftRun.label || compareResult.leftRun.gitRevision || `#${compareResult.leftRun.runId}`}</div><div>Right: {compareResult.rightRun.label || compareResult.rightRun.gitRevision || `#${compareResult.rightRun.runId}`}</div><div>ROM delta: <span className={deltaTone(compareResult.summary.romDelta)}>{signed(compareResult.summary.romDelta)}</span></div><div>RAM delta: <span className={deltaTone(compareResult.summary.ramDelta)}>{signed(compareResult.summary.ramDelta)}</span></div><div>Warning delta: <span className={deltaTone(compareResult.summary.warningDelta)}>{signed(compareResult.summary.warningDelta)}</span></div><div className="button-row"><Button size="sm" variant="flat" onPress={() => openDiffInspector(compareResult.leftRun.runId, compareResult.rightRun.runId)}>Inspect diff</Button></div></CardBody></Card><Card><CardHeader className="section-header">Top deltas</CardHeader><CardBody className="page-stack compact-text"><DeltaList title="Sections" items={compareResult.sectionDeltas} /><DeltaList title="Objects" items={compareResult.objectDeltas} /><DeltaList title="Symbols" items={compareResult.symbolDeltas} /></CardBody></Card></div> : <div className="empty-state">Choose two runs to compare.</div>}
              </div>
            </Tab>

            <Tab key="history" title="History">
              <div className="page-stack">
                <Card><CardHeader className="section-header">Timeline Filters</CardHeader><CardBody className="form-grid"><Input label="Repo path" value={historyFilters.repoPath ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, repoPath: value || null }))} /><Input label="Branch" value={historyFilters.branch ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, branch: value || null }))} /><Input label="Profile" value={historyFilters.profile ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, profile: value || null }))} /><Input label="Target" value={historyFilters.target ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, target: value || null }))} /><div><label>Order</label><select className="native-select" value={historyFilters.order ?? "ancestry"} onChange={(event) => setHistoryFilters((current) => ({ ...current, order: event.target.value as "ancestry" | "timestamp" }))}><option value="ancestry">ancestry</option><option value="timestamp">timestamp</option></select></div><div className="button-row"><Button variant="flat" onPress={() => void refreshGitRefs(historyFilters.repoPath ?? settings.defaultGitRepoPath)}>Refresh refs</Button><Button color="primary" isLoading={loadingHistory} onPress={() => void Promise.all([refreshHistory(historyFilters), refreshTimeline(historyFilters), refreshDashboard(historyFilters)])}>Load timeline</Button><Button variant="flat" onPress={() => { const buildId = historyItems[0]?.buildId; if (buildId) openHistoryInspector(buildId); }}>Inspect latest</Button><Button variant="flat" onPress={() => preparePackageFromContext("history")}>Bundle history</Button></div></CardBody></Card>
                <div className="three-column"><Card><CardHeader className="section-header">Available branches</CardHeader><CardBody className="compact-text badge-column">{branches.length === 0 ? <div>-</div> : branches.map((item) => <button key={item.name} className="chip-button" type="button" onClick={() => setHistoryFilters((current) => ({ ...current, branch: item.name }))}>{item.name}</button>)}</CardBody></Card><Card><CardHeader className="section-header">Available tags</CardHeader><CardBody className="compact-text badge-column">{tags.length === 0 ? <div>-</div> : tags.map((item) => <span key={item.name} className="chip-static">{item.name}</span>)}</CardBody></Card><Card><CardHeader className="section-header">History items</CardHeader><CardBody className="compact-text"><div>{historyItems.length} builds matched</div><div>{timeline?.rows.length ?? 0} timeline rows ready</div></CardBody></Card></div>
                <Card><CardHeader className="section-header">Commit Timeline</CardHeader><CardBody>{loadingHistory ? <div className="loading-state"><Spinner label="Loading history" /></div> : timeline && timeline.rows.length > 0 ? <table className="data-table"><thead><tr><th>Commit</th><th>Subject</th><th>ROM</th><th>RAM</th><th>ROM delta</th><th>RAM delta</th></tr></thead><tbody>{timeline.rows.slice(0, 12).map((row) => <tr key={row.commit}><td>{row.shortCommit}</td><td>{row.subject}</td><td>{formatBytes(row.romTotal)}</td><td>{formatBytes(row.ramTotal)}</td><td><span className={deltaTone(row.romDeltaVsPrevious)}>{signedOrDash(row.romDeltaVsPrevious)}</span></td><td><span className={deltaTone(row.ramDeltaVsPrevious)}>{signedOrDash(row.ramDeltaVsPrevious)}</span></td></tr>)}</tbody></table> : <div className="empty-state">Load the timeline to inspect commit history.</div>}</CardBody></Card>
                <div className="two-column"><Card><CardHeader className="section-header">Range Diff</CardHeader><CardBody className="panel-stack"><Input label="Range spec" value={rangeQuery.spec} onValueChange={(value) => setRangeQuery((current) => ({ ...current, spec: value }))} /><div><label>Order</label><select className="native-select" value={rangeQuery.order ?? "ancestry"} onChange={(event) => setRangeQuery((current) => ({ ...current, order: event.target.value as "ancestry" | "timestamp" }))}><option value="ancestry">ancestry</option><option value="timestamp">timestamp</option></select></div><Button color="primary" isLoading={loadingRange} onPress={() => void handleRangeDiff()}>Run range diff</Button><Button variant="flat" onPress={() => preparePackageFromContext("range")}>Bundle range</Button>{rangeResult ? <div className="compact-text"><div>ROM: <span className={deltaTone(rangeResult.cumulativeRomDelta)}>{signed(rangeResult.cumulativeRomDelta)}</span></div><div>RAM: <span className={deltaTone(rangeResult.cumulativeRamDelta)}>{signed(rangeResult.cumulativeRamDelta)}</span></div><div>Worst commit: {rangeResult.worstCommitByRom?.commit ?? "-"}</div><DeltaList title="Changed sections" items={rangeResult.topChangedSections} /></div> : null}</CardBody></Card><Card><CardHeader className="section-header">Regression</CardHeader><CardBody className="panel-stack"><Input label="Metric / rule / entity key" value={regressionQuery.key} onValueChange={(value) => setRegressionQuery((current) => ({ ...current, key: value }))} /><Input label="Range spec" value={regressionQuery.spec} onValueChange={(value) => setRegressionQuery((current) => ({ ...current, spec: value }))} /><div className="form-grid-inline"><div><label>Detector</label><select className="native-select" value={regressionQuery.detectorType} onChange={(event) => setRegressionQuery((current) => ({ ...current, detectorType: event.target.value as RegressionQuery["detectorType"] }))}><option value="metric">metric</option><option value="rule">rule</option><option value="entity">entity</option></select></div><div><label>Mode</label><select className="native-select" value={regressionQuery.mode} onChange={(event) => setRegressionQuery((current) => ({ ...current, mode: event.target.value as RegressionQuery["mode"] }))}><option value="first-crossing">first-crossing</option><option value="first-jump">first-jump</option><option value="first-presence">first-presence</option><option value="first-violation">first-violation</option></select></div><div><label>Threshold</label><input className="native-select" value={regressionQuery.threshold ?? ""} onChange={(event) => setRegressionQuery((current) => ({ ...current, threshold: event.target.value ? Number(event.target.value) : null }))} /></div></div><Button color="primary" isLoading={loadingRegression} onPress={() => void handleRegression()}>Detect regression</Button><Button variant="flat" onPress={() => preparePackageFromContext("regression")}>Bundle regression</Button>{regressionResult ? <div className="compact-text"><div>Confidence: {regressionResult.confidence}</div><div>Last good: {regressionResult.lastGood?.shortCommit ?? "-"}</div><div>First bad: {regressionResult.firstObservedBad?.shortCommit ?? "-"}</div><div>{regressionResult.reasoning}</div></div> : null}</CardBody></Card></div>
              </div>
            </Tab>


            <Tab key="inspector" title="Inspector">
              <div className="page-stack">
                <div className="button-row compact-wrap">
                  <Button variant="flat" onPress={() => preparePackageFromContext("inspector")}>Bundle this inspector context</Button>
                </div>
                <InspectorPanel query={inspectorQuery} onQueryChange={setInspectorQuery} selection={inspectorSelection} onSelectionChange={setInspectorSelection} />
              </div>
            </Tab>

            <Tab key="plugins" title="Plugins">
              <div className="page-stack">
                <section className="plugin-layout">
                  <Card>
                    <CardHeader className="section-header">Installed plugins</CardHeader>
                    <CardBody className="list-stack compact-text">
                      <div className="button-row compact-wrap">
                        <Button variant="flat" isLoading={loadingPlugins} onPress={() => void refreshPluginData()}>Refresh registry</Button>
                      </div>
                      {plugins.map((plugin) => (
                        <button key={plugin.pluginId} className={`run-row ${selectedPluginId === plugin.pluginId ? "selected" : ""}`} onClick={() => void handleSelectPlugin(plugin.pluginId)} type="button">
                          <div className="run-row-top"><strong>{plugin.displayName}</strong><Chip size="sm" variant="flat">{plugin.status}</Chip></div>
                          <div>{plugin.description}</div>
                          <div className="run-meta"><span>{plugin.layer}</span><span>{plugin.capabilities.length} capabilities</span><span>{plugin.enabled ? "enabled" : "disabled"}</span></div>
                        </button>
                      ))}
                      {plugins.length === 0 ? <div className="empty-state">No plugins registered.</div> : null}
                    </CardBody>
                  </Card>
                  <Card>
                    <CardHeader className="section-header">Plugin detail</CardHeader>
                    <CardBody className="page-stack compact-text">
                      {pluginDetail ? (
                        <>
                          <div className="button-row compact-wrap">
                            <Chip variant="flat">{pluginDetail.summary.safetyLevel}</Chip>
                            <Chip variant="flat">{pluginDetail.summary.stabilityLevel}</Chip>
                            <Chip variant="flat">{pluginDetail.summary.layer}</Chip>
                          </div>
                          <div><strong>{pluginDetail.summary.displayName}</strong> v{pluginDetail.summary.version}</div>
                          <div>{pluginDetail.summary.description}</div>
                          <div className="button-row compact-wrap">
                            <Button size="sm" color={pluginDetail.summary.enabled ? "warning" : "primary"} onPress={() => void handleTogglePlugin(pluginDetail.summary.pluginId, !pluginDetail.summary.enabled)}>
                              {pluginDetail.summary.enabled ? "Disable" : "Enable"}
                            </Button>
                            <Button size="sm" variant="flat" isDisabled={!pluginDetail.summary.enabled} onPress={() => void handleRunPlugin(pluginDetail.summary.pluginId)}>Run plugin</Button>
                          </div>
                          <div><strong>Execution model</strong><div>{pluginDetail.executionModel}</div></div>
                          <div><strong>Failure behavior</strong><div>{pluginDetail.failureBehavior}</div></div>
                          <div><strong>Capabilities</strong><ul className="warning-list">{pluginDetail.summary.capabilities.map((item) => <li key={item.capabilityId}><strong>{item.label}</strong>: {item.description}</li>)}</ul></div>
                          <div><strong>Notes</strong><ul className="warning-list">{pluginDetail.notes.map((item) => <li key={item}>{item}</li>)}</ul></div>
                          {pluginExecution ? <div className="validation-panel"><strong>{pluginExecution.summary}</strong><ul className="warning-list">{pluginExecution.outputItems.map((item) => <li key={`${item.kind}-${item.title}`}><strong>{item.title}</strong>: {item.summary}{item.detail ? ` / ${item.detail}` : ""}</li>)}</ul></div> : null}
                        </>
                      ) : <div className="empty-state">Select a plugin to inspect its capabilities.</div>}
                    </CardBody>
                  </Card>
                </section>
                <Card>
                  <CardHeader className="section-header">Extension points</CardHeader>
                  <CardBody className="page-stack compact-text">
                    <div className="three-column plugin-cap-grid">
                      {extensionPoints.map((point) => (
                        <article key={point.extensionPointId} className="metric-slab plugin-slab">
                          <div className="metric-slab-label">{point.displayName}</div>
                          <div className="metric-slab-value">{point.layer}</div>
                          <p>{point.description}</p>
                        </article>
                      ))}
                    </div>
                  </CardBody>
                </Card>
              </div>
            </Tab>

            <Tab key="packages" title="Packages">
              <div className="page-stack">
                <section className="package-layout">
                  <Card>
                    <CardHeader className="section-header">Create investigation package</CardHeader>
                    <CardBody className="page-stack compact-text">
                      <div className="form-grid">
                        <Input label="Package name" value={packageDraft.packageName} onValueChange={(value) => setPackageDraft((current) => ({ ...current, packageName: value }))} />
                        <Input label="Destination folder" value={packageDraft.destinationPath} onValueChange={(value) => setPackageDraft((current) => ({ ...current, destinationPath: value }))} />
                        <div><label>Source context</label><select className="native-select" value={packageDraft.sourceContext} onChange={(event) => setPackageDraft((current) => ({ ...current, sourceContext: event.target.value, includeSections: [event.target.value, "dashboard"] }))}><option value="dashboard">dashboard</option><option value="run">run</option><option value="diff">diff</option><option value="history">history</option><option value="range">range</option><option value="regression">regression</option><option value="inspector">inspector</option></select></div>
                      </div>
                      <div className="badge-row compact-wrap">
                        {(["dashboard", "run", "diff", "history", "range", "regression", "inspector"] as const).map((value) => <button key={value} type="button" className={`chip-button ${packageDraft.includeSections.includes(value) ? "active-chip" : ""}`} onClick={() => setPackageDraft((current) => ({ ...current, includeSections: current.includeSections.includes(value) ? current.includeSections.filter((item) => item !== value) : [...current.includeSections, value] }))}>{value}</button>)}
                      </div>
                      <Textarea minRows={4} label="Notes" value={packageDraft.notes ?? ""} onValueChange={(value) => setPackageDraft((current) => ({ ...current, notes: value || null, includeNotes: Boolean(value) }))} />
                      <div className="button-row compact-wrap">
                        <Button variant="flat" onPress={() => void choosePackageDestination()}>Choose destination</Button>
                        <Button color="primary" isLoading={packaging} onPress={() => void handleCreatePackage()}>Create package</Button>
                        <Button variant="flat" isLoading={openingPackage} onPress={() => void choosePackageToOpen()}>Open existing package</Button>
                      </div>
                    </CardBody>
                  </Card>
                  <Card>
                    <CardHeader className="section-header">Recent packages</CardHeader>
                    <CardBody className="list-stack compact-text">
                      {recentPackages.map((item) => (
                        <button key={`${item.packagePath}-${item.createdAt}`} className={`run-row ${openedPackage?.summary.packagePath === item.packagePath ? "selected" : ""}`} onClick={() => void handleOpenPackage(item.packagePath)} type="button">
                          <div className="run-row-top"><strong>{item.packageName}</strong><Chip size="sm" variant="flat">{item.sourceContext}</Chip></div>
                          <div>{item.packagePath}</div>
                          <div className="run-meta"><span>{item.createdAt}</span><span>{item.includedCount} included</span><span>{item.omittedCount} omitted</span></div>
                        </button>
                      ))}
                      {recentPackages.length === 0 ? <div className="empty-state">No packages created yet.</div> : null}
                    </CardBody>
                  </Card>
                </section>
                <Card>
                  <CardHeader className="section-header">Package viewer</CardHeader>
                  <CardBody className="page-stack compact-text">
                    {openedPackage ? (
                      <>
                        <div className="button-row compact-wrap">
                          <Chip variant="flat">schema {openedPackage.summary.schemaVersion}</Chip>
                          <Chip variant="flat">{openedPackage.summary.sourceContext}</Chip>
                          <Chip variant="flat">fwmap {openedPackage.summary.fwmapVersion}</Chip>
                        </div>
                        <div><strong>{openedPackage.summary.packageName}</strong></div>
                        <div>{openedPackage.summary.packagePath}</div>
                        <div className="three-column package-summary-grid">
                          <article className="metric-slab plugin-slab"><div className="metric-slab-label">Included</div><div className="metric-slab-value">{openedPackage.summary.includedCount}</div><p>Manifest-tracked resources in the bundle.</p></article>
                          <article className="metric-slab plugin-slab"><div className="metric-slab-label">Omitted</div><div className="metric-slab-value">{openedPackage.summary.omittedCount}</div><p>Resources left out deliberately for reproducibility and size.</p></article>
                          <article className="metric-slab plugin-slab"><div className="metric-slab-label">Related runs</div><div className="metric-slab-value">{openedPackage.manifest.relatedRunIds.length}</div><p>{openedPackage.manifest.relatedCommitRefs.slice(0, 3).join(", ") || "No commits recorded."}</p></article>
                        </div>
                        <div className="two-column package-view-grid">
                          <div>
                            <strong>Contents</strong>
                            <ul className="warning-list">{openedPackage.manifest.includedItems.map((item) => <li key={item.relativePath}>{item.title} / {item.relativePath}</li>)}</ul>
                          </div>
                          <div>
                            <strong>Missing or omitted</strong>
                            <ul className="warning-list">{openedPackage.manifest.omittedItems.map((item) => <li key={item.relativePath}>{item.title}: {item.missingReason ?? "omitted"}</li>)}</ul>
                          </div>
                        </div>
                        {openedPackage.manifest.notes ? <div className="validation-panel"><strong>Notes</strong><div>{openedPackage.manifest.notes}</div></div> : null}
                        {openedPackage.manifest.pluginResults.length > 0 ? <div><strong>Plugin results</strong><ul className="warning-list">{openedPackage.manifest.pluginResults.map((item) => <li key={item.pluginId}><strong>{item.pluginId}</strong>: {item.summary}</li>)}</ul></div> : null}
                      </>
                    ) : <div className="empty-state">Create or open a package to inspect its manifest and captured summaries.</div>}
                  </CardBody>
                </Card>
              </div>
            </Tab>

            <Tab key="settings" title="Settings">
              <div className="page-stack">
                <section className="three-column settings-overview-grid">
                  <Card className="feature-card"><CardBody className="panel-stack compact-text"><div className="stat-label">Active project</div><div className="stat-value settings-stat-value">{activeProjectState?.activeProject?.name ?? "No project"}</div><div className="stat-subtitle">{activeProjectState?.activeProject?.rootPath ?? "Create or select a project to reuse defaults."}</div></CardBody></Card>
                  <Card className="feature-card"><CardBody className="panel-stack compact-text"><div className="stat-label">Policy status</div><div className="stat-value settings-stat-value">{policyValidation ? (policyValidation.ok ? "Ready" : "Needs attention") : "Not checked"}</div><div className="stat-subtitle">{policyDocument.path ?? "No policy file selected."}</div></CardBody></Card>
                  <Card className="feature-card"><CardBody className="panel-stack compact-text"><div className="stat-label">Recent exports</div><div className="stat-value settings-stat-value">{recentExports.length}</div><div className="stat-subtitle">{recentExports[0]?.destinationPath ?? "Nothing exported yet."}</div></CardBody></Card>
                </section>
                <Card className="feature-card">
                  <CardHeader className="feature-header"><div><div className="section-header">Workspace Settings</div><div className="section-subtitle">Keep setup lightweight, predictable, and reusable across projects.</div></div></CardHeader>
                  <CardBody>
                    <Tabs variant="underlined" className="sub-tabs">
                      <Tab key="desktop-settings" title="Desktop">
                        <div className="panel-stack settings-panel compact-text">
                          <Input label="History DB path" value={draftSettings.historyDbPath} onValueChange={(value) => setDraftSettings((current) => ({ ...current, historyDbPath: value }))} />
                          <Button variant="flat" onPress={() => void chooseSettingsPath("historyDbPath")}>Choose history DB</Button>
                          <Input label="Default rule file" value={draftSettings.defaultRuleFilePath ?? ""} onValueChange={(value) => setDraftSettings((current) => ({ ...current, defaultRuleFilePath: value || null }))} />
                          <Button variant="flat" onPress={() => void chooseSettingsPath("defaultRuleFilePath")}>Choose rule file</Button>
                          <Input label="Default Git repo" value={draftSettings.defaultGitRepoPath ?? ""} onValueChange={(value) => setDraftSettings((current) => ({ ...current, defaultGitRepoPath: value || null }))} />
                          <Button variant="flat" onPress={() => void chooseSettingsPath("defaultGitRepoPath", true)}>Choose repo</Button>
                          <Textarea label="Notes" value="Phase D4 adds project workspace, policy editing, and export foundations on top of the D3 desktop shell." readOnly />
                          <div className="button-row"><Button color="primary" isLoading={savingSettings} onPress={() => void handleSaveSettings()}>Save desktop settings</Button></div>
                        </div>
                      </Tab>
                      <Tab key="workspace-settings" title="Workspace">
                        <div className="panel-stack settings-panel compact-text">
                          <div className="badge-row">{loadingProjects ? <Chip size="sm">loading</Chip> : null}{activeProjectState?.activeProject ? <Chip color="primary" variant="flat">Active: {activeProjectState.activeProject.name}</Chip> : <Chip variant="flat">No active project</Chip>}</div>
                          <div><label>Project switcher</label><select className="native-select" value={activeProjectState?.activeProjectId ?? ""} onChange={(event) => void handleSelectProject(event.target.value ? Number(event.target.value) : null)}><option value="">No active project</option>{projects.map((project) => <option key={project.projectId} value={project.projectId}>{project.name}</option>)}</select></div>
                          <Input label="Project name" value={projectDraft.name} onValueChange={(value) => setProjectDraft((current) => ({ ...current, name: value }))} />
                          <Input label="Root path" value={projectDraft.rootPath} onValueChange={(value) => setProjectDraft((current) => ({ ...current, rootPath: value }))} />
                          <Input label="Git repo path" value={projectDraft.gitRepoPath ?? ""} onValueChange={(value) => setProjectDraft((current) => ({ ...current, gitRepoPath: value || null }))} />
                          <Input label="Default ELF" value={projectDraft.defaultElfPath ?? ""} onValueChange={(value) => setProjectDraft((current) => ({ ...current, defaultElfPath: value || null }))} />
                          <Input label="Default map" value={projectDraft.defaultMapPath ?? ""} onValueChange={(value) => setProjectDraft((current) => ({ ...current, defaultMapPath: value || null }))} />
                          <Input label="Default export dir" value={projectDraft.defaultExportDir ?? ""} onValueChange={(value) => setProjectDraft((current) => ({ ...current, defaultExportDir: value || null }))} />
                          <div className="button-row"><Button color="primary" onPress={() => void handleSaveProject()}>Save project</Button><Button variant="flat" onPress={() => void handleCreateProject()}>Create new</Button><Button color="danger" variant="light" isDisabled={!activeProjectState?.activeProjectId} onPress={() => void handleDeleteProject()}>Delete</Button></div>
                        </div>
                      </Tab>
                      <Tab key="policy-settings" title="Policy">
                        <div className="panel-stack settings-panel compact-text">
                          <Input label="Policy path" value={policyDocument.path ?? ""} onValueChange={(value) => setPolicyDocument((current) => ({ ...current, path: value || null }))} />
                          <div><label>Format</label><select className="native-select" value={policyDocument.format} onChange={(event) => setPolicyDocument((current) => ({ ...current, format: event.target.value }))}><option value="toml">toml</option><option value="json">json</option></select></div>
                          <Textarea minRows={12} label="Policy content" value={policyDocument.content} onValueChange={(value) => setPolicyDocument((current) => ({ ...current, content: value, projectId: activeProjectState?.activeProjectId ?? null }))} />
                          <div className="button-row"><Button variant="flat" isLoading={loadingPolicy} onPress={() => void handleLoadPolicy()}>Load</Button><Button variant="flat" onPress={() => void handleValidatePolicy()}>Validate</Button><Button color="primary" onPress={() => void handleSavePolicy()}>Save policy</Button></div>
                          {policyValidation ? <div className="validation-panel"><strong>{policyValidation.ok ? "Validation passed" : "Validation issues"}</strong><ul className="warning-list">{policyValidation.issues.length === 0 ? <li>No issues</li> : null}{policyValidation.issues.map((issue, index) => <li key={`${issue.level}-${index}`}>{issue.level}: {issue.message}</li>)}</ul></div> : null}
                        </div>
                      </Tab>
                      <Tab key="export-settings" title="Export">
                        <div className="panel-stack settings-panel compact-text">
                          <div className="form-grid"><div><label>Target</label><select className="native-select" value={exportDraft.exportTarget} onChange={(event) => setExportDraft((current) => ({ ...current, exportTarget: event.target.value as ExportRequest["exportTarget"] }))}><option value="dashboard">dashboard</option><option value="run">run</option><option value="diff">diff</option><option value="history">history</option><option value="regression">regression</option></select></div><div><label>Format</label><select className="native-select" value={exportDraft.format} onChange={(event) => setExportDraft((current) => ({ ...current, format: event.target.value as ExportRequest["format"] }))}><option value="html">html</option><option value="json">json</option><option value="print-html">print-html</option></select></div><Input label="Destination path" value={exportDraft.destinationPath} onValueChange={(value) => setExportDraft((current) => ({ ...current, destinationPath: value }))} /></div>
                          <Input label="Title" value={exportDraft.title ?? ""} onValueChange={(value) => setExportDraft((current) => ({ ...current, title: value || null }))} />
                          <div className="button-row"><Button color="primary" isLoading={exporting} onPress={() => void handleExport()}>Export snapshot</Button><Button variant="flat" onPress={() => void refreshProjects()}>Refresh list</Button></div>
                          <div><strong>Recent exports</strong><ul className="warning-list">{recentExports.length === 0 ? <li>No exports yet.</li> : null}{recentExports.slice(0, 8).map((item) => <li key={item.exportId}>{item.createdAt} / {item.exportTarget} / {item.destinationPath}</li>)}</ul></div>
                        </div>
                      </Tab>
                    </Tabs>
                  </CardBody>
                </Card>
              </div>
            </Tab>
          </Tabs>

          <section className="message-strip">{note ? <Card className="message-card success"><CardBody>{note}</CardBody></Card> : null}{error ? <Card className="message-card error"><CardBody>{error}</CardBody></Card> : null}</section>
        </main>
      </div>
      <FwCommandPalette open={commandPaletteOpen} items={commandPaletteItems} onClose={() => setCommandPaletteOpen(false)} />
    </div>
  );
}

function ScreenButton({ active, label, detail, onPress }: { active: boolean; label: string; detail: string; onPress: () => void }) {
  return (
    <button type="button" className={`screen-button ${active ? "active" : ""}`} onClick={onPress}>
      <span className="screen-button-label">{label}</span>
      <span className="screen-button-detail">{detail}</span>
    </button>
  );
}

function projectToDraft(project: ProjectDetail | null | undefined): CreateProjectRequest {
  if (!project) return emptyProjectDraft;
  return {
    name: project.name,
    rootPath: project.rootPath,
    gitRepoPath: project.gitRepoPath,
    defaultElfPath: project.defaultElfPath,
    defaultMapPath: project.defaultMapPath,
    defaultDebugPath: project.defaultDebugPath,
    defaultRuleFilePath: project.defaultRuleFilePath,
    defaultTarget: project.defaultTarget,
    defaultProfile: project.defaultProfile,
    defaultExportDir: project.defaultExportDir,
  };
}

function currentOr(query: HistoryQuery, repoPath?: string | null): HistoryQuery {
  return { ...query, repoPath: query.repoPath ?? repoPath ?? null };
}

function signed(value: number): string {
  return `${value >= 0 ? "+" : ""}${value}`;
}

function signedOrDash(value: number | null): string {
  return value == null ? "-" : signed(value);
}

function shorten(value: string, maxLength: number): string {
  return value.length <= maxLength ? value : `${value.slice(0, maxLength - 3)}...`;
}

function deltaTone(value: number | null): string {
  if (value == null) return "delta-pill delta-pill-neutral";
  if (value > 0) return "delta-pill delta-pill-up";
  if (value < 0) return "delta-pill delta-pill-down";
  return "delta-pill delta-pill-neutral";
}

function MetricList({ title, items }: { title: string; items: Array<{ name: string; value: number }> }) {
  return <div><strong>{title}</strong><ul className="metric-list">{items.slice(0, 6).map((item) => <li key={item.name}><span>{item.name}</span><span>{formatBytes(item.value)}</span></li>)}</ul></div>;
}

function DeltaList({ title, items }: { title: string; items: Array<{ name: string; delta: number }> }) {
  return <div><strong>{title}</strong><ul className="metric-list">{items.length === 0 ? <li><span>No data</span><span>-</span></li> : null}{items.slice(0, 6).map((item) => <li key={item.name}><span>{item.name}</span><span className={deltaTone(item.delta)}>{signed(item.delta)}</span></li>)}</ul></div>;
}

