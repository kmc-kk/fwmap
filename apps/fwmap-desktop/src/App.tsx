import { useEffect, useMemo, useState } from "react";
import { Button, Card, CardBody, CardHeader, Chip, Input, Navbar, NavbarBrand, Spinner, Tab, Tabs, Textarea } from "@heroui/react";
import { open } from "@tauri-apps/plugin-dialog";

import { BreakdownBarChart } from "./components/BreakdownBarChart";
import { MetricLineChart } from "./components/MetricLineChart";
import {
  cancelJob,
  compareRuns,
  createProject,
  deleteProject,
  detectRegression,
  exportReport,
  getActiveProject,
  getAppInfo,
  getDashboardSummary,
  getRangeDiff,
  getRunDetail,
  getSettings,
  listProjects,
  listRecentExports,
  loadPolicy,
  getTimeline,
  listBranches,
  listHistory,
  listRecentRuns,
  savePolicy,
  setActiveProject,
  listTags,
  saveSettings,
  startAnalysis,
  updateProject,
  validatePolicy,
} from "./lib/api";
import { listenToJobEvents } from "./lib/events";
import { formatBytes, formatTime, joinParts } from "./lib/format";
import type {
  ActiveProjectState,
  AnalysisRequest,
  CreateProjectRequest,
  DashboardSummary,
  DesktopAppInfo,
  DesktopSettings,
  ExportRequest,
  ExportResult,
  GitRef,
  HistoryItem,
  HistoryQuery,
  JobEvent,
  JobStatus,
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
} from "./lib/types";

type ScreenKey = "dashboard" | "runs" | "diff" | "history" | "settings";

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
  const [compareLeftRunId, setCompareLeftRunId] = useState<number | null>(null);
  const [compareRightRunId, setCompareRightRunId] = useState<number | null>(null);
  const [compareResult, setCompareResult] = useState<RunCompareResult | null>(null);
  const [rangeQuery, setRangeQuery] = useState<RangeDiffQuery>(defaultRangeQuery);
  const [rangeResult, setRangeResult] = useState<RangeDiffResult | null>(null);
  const [regressionQuery, setRegressionQuery] = useState<RegressionQuery>(defaultRegressionQuery);
  const [regressionResult, setRegressionResult] = useState<RegressionResult | null>(null);
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
  const [loadingRefs, setLoadingRefs] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const hash = window.location.hash.replace(/^#/, "");
    const [first, second, third] = hash.split("/");
    if (first === "runs" && second) {
      setScreen("runs");
      const parsed = Number(second);
      if (Number.isFinite(parsed)) {
        setSelectedRunId(parsed);
      }
    } else if (first === "diff") {
      setScreen("diff");
      const left = Number(second);
      const right = Number(third);
      if (Number.isFinite(left)) {
        setCompareLeftRunId(left);
      }
      if (Number.isFinite(right)) {
        setCompareRightRunId(right);
      }
    } else if (first === "history") {
      setScreen("history");
    } else if (first === "settings") {
      setScreen("settings");
    }
  }, []);

  useEffect(() => {
    if (screen === "runs" && selectedRunId) {
      window.location.hash = `runs/${selectedRunId}`;
    } else if (screen === "diff" && compareLeftRunId && compareRightRunId) {
      window.location.hash = `diff/${compareLeftRunId}/${compareRightRunId}`;
    } else {
      window.location.hash = screen;
    }
  }, [screen, selectedRunId, compareLeftRunId, compareRightRunId]);

  useEffect(() => {
    let disposed = false;
    async function load() {
      setBusy(true);
      try {
        const [info, loadedSettings, loadedRuns, loadedProjects, loadedActiveProject, loadedExports] = await Promise.all([
          getAppInfo(),
          getSettings(),
          listRecentRuns(30, 0),
          listProjects(),
          getActiveProject(),
          listRecentExports(null, 12),
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
          <Chip variant="flat">CLI {appInfo?.cliVersion ?? "-"}</Chip>
          <Chip variant="flat">History {timeline?.repoId ? "git-aware" : "local"}</Chip>
        </div>
      </Navbar>

      <div className="app-grid wide">
        <aside className="sidebar">
          <Card className="sidebar-card">
            <CardHeader className="section-header">Start Analysis</CardHeader>
            <CardBody className="panel-stack">
              <Input label="ELF path" value={request.elfPath ?? ""} onValueChange={(value) => setRequest((current) => ({ ...current, elfPath: value || null }))} />
              <Button variant="flat" onPress={() => void chooseFile("elfPath")}>Choose ELF</Button>
              <Input label="Map path" value={request.mapPath ?? ""} onValueChange={(value) => setRequest((current) => ({ ...current, mapPath: value || null }))} />
              <Button variant="flat" onPress={() => void chooseFile("mapPath")}>Choose map</Button>
              <Input label="Rule file" value={request.ruleFilePath ?? ""} onValueChange={(value) => setRequest((current) => ({ ...current, ruleFilePath: value || null }))} />
              <Button variant="flat" onPress={() => void chooseFile("ruleFilePath")}>Choose rule file</Button>
              <Input label="Git repo" value={request.gitRepoPath ?? ""} onValueChange={(value) => setRequest((current) => ({ ...current, gitRepoPath: value || null }))} />
              <Button variant="flat" onPress={() => void chooseFile("gitRepoPath", true)}>Choose repo</Button>
              <Input label="Label" value={request.label ?? ""} onValueChange={(value) => setRequest((current) => ({ ...current, label: value || null }))} />
              <Button color="primary" isLoading={starting} onPress={() => void handleStartAnalysis()}>Start analysis</Button>
              <Button variant="bordered" isDisabled={!job} onPress={() => void handleCancelJob()}>Cancel job</Button>
            </CardBody>
          </Card>

          <Card className="sidebar-card muted-card">
            <CardHeader className="section-header">Repository</CardHeader>
            <CardBody className="panel-stack compact-text">
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

        <main className="content">
          <Tabs selectedKey={screen} onSelectionChange={(key) => setScreen(key as ScreenKey)} variant="underlined" className="main-tabs">
            <Tab key="dashboard" title="Dashboard">
              {busy ? <div className="loading-state"><Spinner label="Loading desktop state" /></div> : (
                <div className="page-stack">
                  <section className="dashboard-hero">
                    <div>
                      <div className="dashboard-kicker">Latest build posture</div>
                      <h1>Track size movement before it turns into a regression.</h1>
                      <p>Dashboard combines the latest run, Git-aware history, and recent warning pressure into one view.</p>
                    </div>
                    <div className="hero-chip-row">
                      <Chip variant="flat">{dashboardSummary?.latestHistoryItem?.gitBranch ?? settings.defaultGitRepoPath ?? "No repo"}</Chip>
                      <Chip variant="flat">{dashboardSummary?.latestHistoryItem?.gitRevision ?? "No commit"}</Chip>
                      <Chip variant="flat">{dashboardSummary?.latestRun?.profile ?? "profile -"}</Chip>
                    </div>
                  </section>
                  <section className="stats-grid dashboard-card-grid">
                    {dashboardSummary?.overviewCards.map((item) => <Card key={item.key} className={`stat-card metric-tone-${item.tone}`}><CardBody><div className="stat-label">{item.title}</div><div className="stat-value">{item.value}</div>{item.subtitle ? <div className="stat-subtitle">{item.subtitle}</div> : null}</CardBody></Card>)}
                    {!dashboardSummary?.overviewCards?.length ? dashboardStats.map((item) => <Card key={item.label} className="stat-card"><CardBody><div className="stat-label">{item.label}</div><div className="stat-value">{item.value}</div></CardBody></Card>) : null}
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

            <Tab key="runs" title="Runs">
              <div className="runs-layout">
                <Card><CardHeader className="section-header">Recent Runs</CardHeader><CardBody className="list-stack">{runs.map((run) => <button key={run.runId} className={`run-row ${selectedRunId === run.runId ? "selected" : ""}`} onClick={() => setSelectedRunId(run.runId)} type="button"><div className="run-row-top"><strong>#{run.runId}</strong><Chip size="sm" variant="flat">{run.status}</Chip></div><div>{run.label || run.gitRevision || "Unnamed run"}</div><div className="run-meta"><span>{formatTime(run.createdAt)}</span><span>{formatBytes(run.romBytes)} ROM</span><span>{formatBytes(run.ramBytes)} RAM</span></div></button>)}{runs.length === 0 ? <div className="empty-state">No runs recorded yet.</div> : null}</CardBody></Card>
                <Card><CardHeader className="section-header">Run Detail</CardHeader><CardBody className="page-stack compact-text">{runDetail ? <><div className="detail-grid"><div><strong>ELF</strong><br />{runDetail.elfPath || "-"}</div><div><strong>Arch</strong><br />{runDetail.arch || "-"}</div><div><strong>Linker</strong><br />{joinParts([runDetail.linkerFamily, runDetail.mapFormat]) || "-"}</div><div><strong>Git</strong><br />{joinParts([runDetail.run.gitRevision, runDetail.gitBranch, runDetail.gitDescribe]) || "-"}</div></div><div className="button-row"><Button size="sm" variant="flat" onPress={() => setCompareLeftRunId(runDetail.run.runId)}>Use as left</Button><Button size="sm" variant="flat" onPress={() => setCompareRightRunId(runDetail.run.runId)}>Use as right</Button><Button size="sm" color="primary" variant="flat" onPress={() => setScreen("diff")}>Open diff</Button></div><MetricList title="Top sections" items={runDetail.topSections.map(([name, value]) => ({ name, value }))} /><MetricList title="Top symbols" items={runDetail.topSymbols.map(([name, value]) => ({ name, value }))} /><div><strong>Rule warnings</strong><ul className="warning-list">{runDetail.warnings.length === 0 ? <li>No rule warnings recorded.</li> : null}{runDetail.warnings.slice(0, 8).map(([code, level, related], index) => <li key={`${code}-${index}`}>{level} / {code}{related ? ` / ${related}` : ""}</li>)}</ul></div></> : <div className="empty-state">Select a run to inspect it.</div>}</CardBody></Card>
              </div>
            </Tab>

            <Tab key="diff" title="Diff">
              <div className="page-stack">
                <Card><CardHeader className="section-header">Run Compare</CardHeader><CardBody className="form-grid"><div><label>Left run</label><select className="native-select" value={compareLeftRunId ?? ""} onChange={(event) => setCompareLeftRunId(Number(event.target.value) || null)}>{runs.map((run) => <option key={run.runId} value={run.runId}>#{run.runId} {run.label || run.gitRevision || "run"}</option>)}</select></div><div><label>Right run</label><select className="native-select" value={compareRightRunId ?? ""} onChange={(event) => setCompareRightRunId(Number(event.target.value) || null)}>{runs.map((run) => <option key={run.runId} value={run.runId}>#{run.runId} {run.label || run.gitRevision || "run"}</option>)}</select></div><div className="button-row"><Button color="primary" isLoading={loadingCompare} onPress={() => void handleRunCompare()}>Compare runs</Button></div></CardBody></Card>
                {compareResult ? <div className="two-column"><Card><CardHeader className="section-header">Summary</CardHeader><CardBody className="panel-stack compact-text"><div>Left: {compareResult.leftRun.label || compareResult.leftRun.gitRevision || `#${compareResult.leftRun.runId}`}</div><div>Right: {compareResult.rightRun.label || compareResult.rightRun.gitRevision || `#${compareResult.rightRun.runId}`}</div><div>ROM delta: <span className={deltaTone(compareResult.summary.romDelta)}>{signed(compareResult.summary.romDelta)}</span></div><div>RAM delta: <span className={deltaTone(compareResult.summary.ramDelta)}>{signed(compareResult.summary.ramDelta)}</span></div><div>Warning delta: <span className={deltaTone(compareResult.summary.warningDelta)}>{signed(compareResult.summary.warningDelta)}</span></div></CardBody></Card><Card><CardHeader className="section-header">Top deltas</CardHeader><CardBody className="page-stack compact-text"><DeltaList title="Sections" items={compareResult.sectionDeltas} /><DeltaList title="Objects" items={compareResult.objectDeltas} /><DeltaList title="Symbols" items={compareResult.symbolDeltas} /></CardBody></Card></div> : <div className="empty-state">Choose two runs to compare.</div>}
              </div>
            </Tab>

            <Tab key="history" title="History">
              <div className="page-stack">
                <Card><CardHeader className="section-header">Timeline Filters</CardHeader><CardBody className="form-grid"><Input label="Repo path" value={historyFilters.repoPath ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, repoPath: value || null }))} /><Input label="Branch" value={historyFilters.branch ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, branch: value || null }))} /><Input label="Profile" value={historyFilters.profile ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, profile: value || null }))} /><Input label="Target" value={historyFilters.target ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, target: value || null }))} /><div><label>Order</label><select className="native-select" value={historyFilters.order ?? "ancestry"} onChange={(event) => setHistoryFilters((current) => ({ ...current, order: event.target.value as "ancestry" | "timestamp" }))}><option value="ancestry">ancestry</option><option value="timestamp">timestamp</option></select></div><div className="button-row"><Button variant="flat" onPress={() => void refreshGitRefs(historyFilters.repoPath ?? settings.defaultGitRepoPath)}>Refresh refs</Button><Button color="primary" isLoading={loadingHistory} onPress={() => void Promise.all([refreshHistory(historyFilters), refreshTimeline(historyFilters), refreshDashboard(historyFilters)])}>Load timeline</Button></div></CardBody></Card>
                <div className="three-column"><Card><CardHeader className="section-header">Available branches</CardHeader><CardBody className="compact-text badge-column">{branches.length === 0 ? <div>-</div> : branches.map((item) => <button key={item.name} className="chip-button" type="button" onClick={() => setHistoryFilters((current) => ({ ...current, branch: item.name }))}>{item.name}</button>)}</CardBody></Card><Card><CardHeader className="section-header">Available tags</CardHeader><CardBody className="compact-text badge-column">{tags.length === 0 ? <div>-</div> : tags.map((item) => <span key={item.name} className="chip-static">{item.name}</span>)}</CardBody></Card><Card><CardHeader className="section-header">History items</CardHeader><CardBody className="compact-text"><div>{historyItems.length} builds matched</div><div>{timeline?.rows.length ?? 0} timeline rows ready</div></CardBody></Card></div>
                <Card><CardHeader className="section-header">Commit Timeline</CardHeader><CardBody>{loadingHistory ? <div className="loading-state"><Spinner label="Loading history" /></div> : timeline && timeline.rows.length > 0 ? <table className="data-table"><thead><tr><th>Commit</th><th>Subject</th><th>ROM</th><th>RAM</th><th>ROM delta</th><th>RAM delta</th></tr></thead><tbody>{timeline.rows.slice(0, 12).map((row) => <tr key={row.commit}><td>{row.shortCommit}</td><td>{row.subject}</td><td>{formatBytes(row.romTotal)}</td><td>{formatBytes(row.ramTotal)}</td><td><span className={deltaTone(row.romDeltaVsPrevious)}>{signedOrDash(row.romDeltaVsPrevious)}</span></td><td><span className={deltaTone(row.ramDeltaVsPrevious)}>{signedOrDash(row.ramDeltaVsPrevious)}</span></td></tr>)}</tbody></table> : <div className="empty-state">Load the timeline to inspect commit history.</div>}</CardBody></Card>
                <div className="two-column"><Card><CardHeader className="section-header">Range Diff</CardHeader><CardBody className="panel-stack"><Input label="Range spec" value={rangeQuery.spec} onValueChange={(value) => setRangeQuery((current) => ({ ...current, spec: value }))} /><div><label>Order</label><select className="native-select" value={rangeQuery.order ?? "ancestry"} onChange={(event) => setRangeQuery((current) => ({ ...current, order: event.target.value as "ancestry" | "timestamp" }))}><option value="ancestry">ancestry</option><option value="timestamp">timestamp</option></select></div><Button color="primary" isLoading={loadingRange} onPress={() => void handleRangeDiff()}>Run range diff</Button>{rangeResult ? <div className="compact-text"><div>ROM: <span className={deltaTone(rangeResult.cumulativeRomDelta)}>{signed(rangeResult.cumulativeRomDelta)}</span></div><div>RAM: <span className={deltaTone(rangeResult.cumulativeRamDelta)}>{signed(rangeResult.cumulativeRamDelta)}</span></div><div>Worst commit: {rangeResult.worstCommitByRom?.commit ?? "-"}</div><DeltaList title="Changed sections" items={rangeResult.topChangedSections} /></div> : null}</CardBody></Card><Card><CardHeader className="section-header">Regression</CardHeader><CardBody className="panel-stack"><Input label="Metric / rule / entity key" value={regressionQuery.key} onValueChange={(value) => setRegressionQuery((current) => ({ ...current, key: value }))} /><Input label="Range spec" value={regressionQuery.spec} onValueChange={(value) => setRegressionQuery((current) => ({ ...current, spec: value }))} /><div className="form-grid-inline"><div><label>Detector</label><select className="native-select" value={regressionQuery.detectorType} onChange={(event) => setRegressionQuery((current) => ({ ...current, detectorType: event.target.value as RegressionQuery["detectorType"] }))}><option value="metric">metric</option><option value="rule">rule</option><option value="entity">entity</option></select></div><div><label>Mode</label><select className="native-select" value={regressionQuery.mode} onChange={(event) => setRegressionQuery((current) => ({ ...current, mode: event.target.value as RegressionQuery["mode"] }))}><option value="first-crossing">first-crossing</option><option value="first-jump">first-jump</option><option value="first-presence">first-presence</option><option value="first-violation">first-violation</option></select></div><div><label>Threshold</label><input className="native-select" value={regressionQuery.threshold ?? ""} onChange={(event) => setRegressionQuery((current) => ({ ...current, threshold: event.target.value ? Number(event.target.value) : null }))} /></div></div><Button color="primary" isLoading={loadingRegression} onPress={() => void handleRegression()}>Detect regression</Button>{regressionResult ? <div className="compact-text"><div>Confidence: {regressionResult.confidence}</div><div>Last good: {regressionResult.lastGood?.shortCommit ?? "-"}</div><div>First bad: {regressionResult.firstObservedBad?.shortCommit ?? "-"}</div><div>{regressionResult.reasoning}</div></div> : null}</CardBody></Card></div>
              </div>
            </Tab>

            <Tab key="settings" title="Settings">
              <div className="page-stack">
                <Card><CardHeader className="section-header">Desktop Settings</CardHeader><CardBody className="panel-stack"><Input label="History DB path" value={draftSettings.historyDbPath} onValueChange={(value) => setDraftSettings((current) => ({ ...current, historyDbPath: value }))} /><Button variant="flat" onPress={() => void chooseSettingsPath("historyDbPath")}>Choose history DB</Button><Input label="Default rule file" value={draftSettings.defaultRuleFilePath ?? ""} onValueChange={(value) => setDraftSettings((current) => ({ ...current, defaultRuleFilePath: value || null }))} /><Button variant="flat" onPress={() => void chooseSettingsPath("defaultRuleFilePath")}>Choose rule file</Button><Input label="Default Git repo" value={draftSettings.defaultGitRepoPath ?? ""} onValueChange={(value) => setDraftSettings((current) => ({ ...current, defaultGitRepoPath: value || null }))} /><Button variant="flat" onPress={() => void chooseSettingsPath("defaultGitRepoPath", true)}>Choose repo</Button><Textarea label="Notes" value="Phase D4 adds project workspace, policy editing, and export foundations on top of the D3 desktop shell." readOnly /><Button color="primary" isLoading={savingSettings} onPress={() => void handleSaveSettings()}>Save settings</Button></CardBody></Card>
                <div className="two-column">
                  <Card><CardHeader className="section-header">Workspace / Project</CardHeader><CardBody className="panel-stack compact-text"><div className="badge-row">{loadingProjects ? <Chip size="sm">loading</Chip> : null}{activeProjectState?.activeProject ? <Chip color="primary" variant="flat">Active: {activeProjectState.activeProject.name}</Chip> : <Chip variant="flat">No active project</Chip>}</div><select className="native-select" value={activeProjectState?.activeProjectId ?? ""} onChange={(event) => void handleSelectProject(event.target.value ? Number(event.target.value) : null)}><option value="">No active project</option>{projects.map((project) => <option key={project.projectId} value={project.projectId}>{project.name}</option>)}</select><Input label="Project name" value={projectDraft.name} onValueChange={(value) => setProjectDraft((current) => ({ ...current, name: value }))} /><Input label="Root path" value={projectDraft.rootPath} onValueChange={(value) => setProjectDraft((current) => ({ ...current, rootPath: value }))} /><Input label="Git repo path" value={projectDraft.gitRepoPath ?? ""} onValueChange={(value) => setProjectDraft((current) => ({ ...current, gitRepoPath: value || null }))} /><Input label="Default ELF" value={projectDraft.defaultElfPath ?? ""} onValueChange={(value) => setProjectDraft((current) => ({ ...current, defaultElfPath: value || null }))} /><Input label="Default map" value={projectDraft.defaultMapPath ?? ""} onValueChange={(value) => setProjectDraft((current) => ({ ...current, defaultMapPath: value || null }))} /><Input label="Default export dir" value={projectDraft.defaultExportDir ?? ""} onValueChange={(value) => setProjectDraft((current) => ({ ...current, defaultExportDir: value || null }))} /><div className="button-row"><Button color="primary" onPress={() => void handleSaveProject()}>Save project</Button><Button variant="flat" onPress={() => void handleCreateProject()}>Create new</Button><Button color="danger" variant="flat" isDisabled={!activeProjectState?.activeProjectId} onPress={() => void handleDeleteProject()}>Delete</Button></div></CardBody></Card>
                  <Card><CardHeader className="section-header">Policy Editor</CardHeader><CardBody className="panel-stack compact-text"><Input label="Policy path" value={policyDocument.path ?? ""} onValueChange={(value) => setPolicyDocument((current) => ({ ...current, path: value || null }))} /><div><label>Format</label><select className="native-select" value={policyDocument.format} onChange={(event) => setPolicyDocument((current) => ({ ...current, format: event.target.value }))}><option value="toml">toml</option><option value="json">json</option></select></div><Textarea minRows={12} label="Policy content" value={policyDocument.content} onValueChange={(value) => setPolicyDocument((current) => ({ ...current, content: value, projectId: activeProjectState?.activeProjectId ?? null }))} /><div className="button-row"><Button variant="flat" isLoading={loadingPolicy} onPress={() => void handleLoadPolicy()}>Load</Button><Button variant="flat" onPress={() => void handleValidatePolicy()}>Validate</Button><Button color="primary" onPress={() => void handleSavePolicy()}>Save policy</Button></div>{policyValidation ? <div><strong>{policyValidation.ok ? "Validation passed" : "Validation issues"}</strong><ul className="warning-list">{policyValidation.issues.length === 0 ? <li>No issues</li> : null}{policyValidation.issues.map((issue, index) => <li key={`${issue.level}-${index}`}>{issue.level}: {issue.message}</li>)}</ul></div> : null}</CardBody></Card>
                </div>
                <Card><CardHeader className="section-header">Report Export</CardHeader><CardBody className="panel-stack compact-text"><div className="form-grid"><div><label>Target</label><select className="native-select" value={exportDraft.exportTarget} onChange={(event) => setExportDraft((current) => ({ ...current, exportTarget: event.target.value as ExportRequest["exportTarget"] }))}><option value="dashboard">dashboard</option><option value="run">run</option><option value="diff">diff</option><option value="history">history</option><option value="regression">regression</option></select></div><div><label>Format</label><select className="native-select" value={exportDraft.format} onChange={(event) => setExportDraft((current) => ({ ...current, format: event.target.value as ExportRequest["format"] }))}><option value="html">html</option><option value="json">json</option><option value="print-html">print-html</option></select></div><Input label="Destination path" value={exportDraft.destinationPath} onValueChange={(value) => setExportDraft((current) => ({ ...current, destinationPath: value }))} /></div><Input label="Title" value={exportDraft.title ?? ""} onValueChange={(value) => setExportDraft((current) => ({ ...current, title: value || null }))} /><div className="button-row"><Button color="primary" isLoading={exporting} onPress={() => void handleExport()}>Export</Button><Button variant="flat" onPress={() => void refreshProjects()}>Refresh projects/exports</Button></div><div><strong>Recent exports</strong><ul className="warning-list">{recentExports.length === 0 ? <li>No exports yet.</li> : null}{recentExports.slice(0, 8).map((item) => <li key={item.exportId}>{item.createdAt} / {item.exportTarget} / {item.destinationPath}</li>)}</ul></div></CardBody></Card>
              </div>
            </Tab>
          </Tabs>

          <section className="message-strip">{note ? <Card className="message-card success"><CardBody>{note}</CardBody></Card> : null}{error ? <Card className="message-card error"><CardBody>{error}</CardBody></Card> : null}</section>
        </main>
      </div>
    </div>
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
  return value.length <= maxLength ? value : `${value.slice(0, maxLength - 1)}?`;
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
