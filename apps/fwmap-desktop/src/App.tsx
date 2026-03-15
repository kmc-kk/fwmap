import { useEffect, useMemo, useState } from "react";
import { Button, Card, CardBody, CardHeader, Chip, Input, Navbar, NavbarBrand, Spinner, Tab, Tabs, Textarea } from "@heroui/react";
import { open } from "@tauri-apps/plugin-dialog";

import {
  cancelJob,
  compareRuns,
  detectRegression,
  getAppInfo,
  getRangeDiff,
  getRunDetail,
  getSettings,
  getTimeline,
  listBranches,
  listHistory,
  listRecentRuns,
  listTags,
  saveSettings,
  startAnalysis,
} from "./lib/api";
import { listenToJobEvents } from "./lib/events";
import { formatBytes, formatTime, joinParts } from "./lib/format";
import type {
  AnalysisRequest,
  DesktopAppInfo,
  DesktopSettings,
  GitRef,
  HistoryItem,
  HistoryQuery,
  JobEvent,
  JobStatus,
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
        const [info, loadedSettings, loadedRuns] = await Promise.all([getAppInfo(), getSettings(), listRecentRuns(30, 0)]);
        if (disposed) return;
        setAppInfo(info);
        setSettings(loadedSettings);
        setDraftSettings(loadedSettings);
        setRequest((current) => ({
          ...current,
          elfPath: current.elfPath ?? loadedSettings.lastElfPath,
          mapPath: current.mapPath ?? loadedSettings.lastMapPath,
          ruleFilePath: current.ruleFilePath ?? loadedSettings.defaultRuleFilePath,
          gitRepoPath: current.gitRepoPath ?? loadedSettings.defaultGitRepoPath,
        }));
        setHistoryFilters((current) => ({ ...current, repoPath: current.repoPath ?? loadedSettings.defaultGitRepoPath }));
        setRangeQuery((current) => ({ ...current, repoPath: current.repoPath ?? loadedSettings.defaultGitRepoPath }));
        setRegressionQuery((current) => ({ ...current, repoPath: current.repoPath ?? loadedSettings.defaultGitRepoPath }));
        setRuns(loadedRuns);
        const fallbackRunId = loadedRuns[0]?.runId ?? null;
        setSelectedRunId((current) => current ?? fallbackRunId);
        setCompareLeftRunId((current) => current ?? fallbackRunId);
        setCompareRightRunId((current) => current ?? loadedRuns[1]?.runId ?? fallbackRunId);
        if (loadedSettings.defaultGitRepoPath) {
          await refreshGitRefs(loadedSettings.defaultGitRepoPath);
        }
        await Promise.all([refreshHistory(currentOr(historyFilters, loadedSettings.defaultGitRepoPath)), refreshTimeline(currentOr(historyFilters, loadedSettings.defaultGitRepoPath))]);
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
      setNote("Analysis finished. Runs, history, and timeline were refreshed.");
      void refreshRuns(event.runId ?? undefined);
      void refreshHistory(historyFilters);
      void refreshTimeline(historyFilters);
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
      setSettings(saved);
      setDraftSettings(saved);
      await refreshGitRefs(saved.defaultGitRepoPath);
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

  return (
    <div className="app-shell">
      <Navbar maxWidth="full" className="topbar">
        <NavbarBrand>
          <div>
            <div className="brand-title">fwmap desktop</div>
            <div className="brand-subtitle">History, diff, and regression foundation</div>
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
                  <section className="stats-grid">
                    {dashboardStats.map((item) => <Card key={item.label} className="stat-card"><CardBody><div className="stat-label">{item.label}</div><div className="stat-value">{item.value}</div></CardBody></Card>)}
                  </section>
                  <section className="three-column">
                    <Card><CardHeader className="section-header">Current Job</CardHeader><CardBody className="panel-stack compact-text"><div>Status: <strong>{job?.status ?? "idle"}</strong></div><div>Message: {job?.progressMessage ?? "No active job"}</div><div>Updated: {formatTime(job?.updatedAt)}</div></CardBody></Card>
                    <Card><CardHeader className="section-header">Latest Run</CardHeader><CardBody className="panel-stack compact-text"><div>{latestRun ? (latestRun.label || latestRun.gitRevision || `Run #${latestRun.runId}`) : "No runs yet"}</div><div>{latestRun ? `${formatBytes(latestRun.romBytes)} ROM / ${formatBytes(latestRun.ramBytes)} RAM` : "-"}</div><div>{latestRun ? joinParts([latestRun.profile, latestRun.target]) : "-"}</div></CardBody></Card>
                    <Card><CardHeader className="section-header">History Snapshot</CardHeader><CardBody className="panel-stack compact-text"><div>Repo: {timeline?.repoId ?? "-"}</div><div>Rows: {timeline?.rows.length ?? 0}</div><div>Range rows: {rangeResult?.timelineRows.length ?? 0}</div></CardBody></Card>
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
                {compareResult ? <div className="two-column"><Card><CardHeader className="section-header">Summary</CardHeader><CardBody className="panel-stack compact-text"><div>Left: {compareResult.leftRun.label || compareResult.leftRun.gitRevision || `#${compareResult.leftRun.runId}`}</div><div>Right: {compareResult.rightRun.label || compareResult.rightRun.gitRevision || `#${compareResult.rightRun.runId}`}</div><div>ROM delta: {signed(compareResult.summary.romDelta)}</div><div>RAM delta: {signed(compareResult.summary.ramDelta)}</div><div>Warning delta: {signed(compareResult.summary.warningDelta)}</div></CardBody></Card><Card><CardHeader className="section-header">Top deltas</CardHeader><CardBody className="page-stack compact-text"><DeltaList title="Sections" items={compareResult.sectionDeltas} /><DeltaList title="Objects" items={compareResult.objectDeltas} /><DeltaList title="Symbols" items={compareResult.symbolDeltas} /></CardBody></Card></div> : <div className="empty-state">Choose two runs to compare.</div>}
              </div>
            </Tab>

            <Tab key="history" title="History">
              <div className="page-stack">
                <Card><CardHeader className="section-header">Timeline Filters</CardHeader><CardBody className="form-grid"><Input label="Repo path" value={historyFilters.repoPath ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, repoPath: value || null }))} /><Input label="Branch" value={historyFilters.branch ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, branch: value || null }))} /><Input label="Profile" value={historyFilters.profile ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, profile: value || null }))} /><Input label="Target" value={historyFilters.target ?? ""} onValueChange={(value) => setHistoryFilters((current) => ({ ...current, target: value || null }))} /><div><label>Order</label><select className="native-select" value={historyFilters.order ?? "ancestry"} onChange={(event) => setHistoryFilters((current) => ({ ...current, order: event.target.value as "ancestry" | "timestamp" }))}><option value="ancestry">ancestry</option><option value="timestamp">timestamp</option></select></div><div className="button-row"><Button variant="flat" onPress={() => void refreshGitRefs(historyFilters.repoPath ?? settings.defaultGitRepoPath)}>Refresh refs</Button><Button color="primary" isLoading={loadingHistory} onPress={() => void Promise.all([refreshHistory(historyFilters), refreshTimeline(historyFilters)])}>Load timeline</Button></div></CardBody></Card>
                <div className="three-column"><Card><CardHeader className="section-header">Available branches</CardHeader><CardBody className="compact-text badge-column">{branches.length === 0 ? <div>-</div> : branches.map((item) => <button key={item.name} className="chip-button" type="button" onClick={() => setHistoryFilters((current) => ({ ...current, branch: item.name }))}>{item.name}</button>)}</CardBody></Card><Card><CardHeader className="section-header">Available tags</CardHeader><CardBody className="compact-text badge-column">{tags.length === 0 ? <div>-</div> : tags.map((item) => <span key={item.name} className="chip-static">{item.name}</span>)}</CardBody></Card><Card><CardHeader className="section-header">History items</CardHeader><CardBody className="compact-text"><div>{historyItems.length} builds matched</div><div>{timeline?.rows.length ?? 0} timeline rows ready</div></CardBody></Card></div>
                <Card><CardHeader className="section-header">Commit Timeline</CardHeader><CardBody>{loadingHistory ? <div className="loading-state"><Spinner label="Loading history" /></div> : timeline && timeline.rows.length > 0 ? <table className="data-table"><thead><tr><th>Commit</th><th>Subject</th><th>ROM</th><th>RAM</th><th>ROM delta</th><th>RAM delta</th></tr></thead><tbody>{timeline.rows.slice(0, 12).map((row) => <tr key={row.commit}><td>{row.shortCommit}</td><td>{row.subject}</td><td>{formatBytes(row.romTotal)}</td><td>{formatBytes(row.ramTotal)}</td><td>{signedOrDash(row.romDeltaVsPrevious)}</td><td>{signedOrDash(row.ramDeltaVsPrevious)}</td></tr>)}</tbody></table> : <div className="empty-state">Load the timeline to inspect commit history.</div>}</CardBody></Card>
                <div className="two-column"><Card><CardHeader className="section-header">Range Diff</CardHeader><CardBody className="panel-stack"><Input label="Range spec" value={rangeQuery.spec} onValueChange={(value) => setRangeQuery((current) => ({ ...current, spec: value }))} /><div><label>Order</label><select className="native-select" value={rangeQuery.order ?? "ancestry"} onChange={(event) => setRangeQuery((current) => ({ ...current, order: event.target.value as "ancestry" | "timestamp" }))}><option value="ancestry">ancestry</option><option value="timestamp">timestamp</option></select></div><Button color="primary" isLoading={loadingRange} onPress={() => void handleRangeDiff()}>Run range diff</Button>{rangeResult ? <div className="compact-text"><div>ROM: {signed(rangeResult.cumulativeRomDelta)}</div><div>RAM: {signed(rangeResult.cumulativeRamDelta)}</div><div>Worst commit: {rangeResult.worstCommitByRom?.commit ?? "-"}</div><DeltaList title="Changed sections" items={rangeResult.topChangedSections} /></div> : null}</CardBody></Card><Card><CardHeader className="section-header">Regression</CardHeader><CardBody className="panel-stack"><Input label="Metric / rule / entity key" value={regressionQuery.key} onValueChange={(value) => setRegressionQuery((current) => ({ ...current, key: value }))} /><Input label="Range spec" value={regressionQuery.spec} onValueChange={(value) => setRegressionQuery((current) => ({ ...current, spec: value }))} /><div className="form-grid-inline"><div><label>Detector</label><select className="native-select" value={regressionQuery.detectorType} onChange={(event) => setRegressionQuery((current) => ({ ...current, detectorType: event.target.value as RegressionQuery["detectorType"] }))}><option value="metric">metric</option><option value="rule">rule</option><option value="entity">entity</option></select></div><div><label>Mode</label><select className="native-select" value={regressionQuery.mode} onChange={(event) => setRegressionQuery((current) => ({ ...current, mode: event.target.value as RegressionQuery["mode"] }))}><option value="first-crossing">first-crossing</option><option value="first-jump">first-jump</option><option value="first-presence">first-presence</option><option value="first-violation">first-violation</option></select></div><div><label>Threshold</label><input className="native-select" value={regressionQuery.threshold ?? ""} onChange={(event) => setRegressionQuery((current) => ({ ...current, threshold: event.target.value ? Number(event.target.value) : null }))} /></div></div><Button color="primary" isLoading={loadingRegression} onPress={() => void handleRegression()}>Detect regression</Button>{regressionResult ? <div className="compact-text"><div>Confidence: {regressionResult.confidence}</div><div>Last good: {regressionResult.lastGood?.shortCommit ?? "-"}</div><div>First bad: {regressionResult.firstObservedBad?.shortCommit ?? "-"}</div><div>{regressionResult.reasoning}</div></div> : null}</CardBody></Card></div>
              </div>
            </Tab>

            <Tab key="settings" title="Settings">
              <Card><CardHeader className="section-header">Desktop Settings</CardHeader><CardBody className="panel-stack"><Input label="History DB path" value={draftSettings.historyDbPath} onValueChange={(value) => setDraftSettings((current) => ({ ...current, historyDbPath: value }))} /><Button variant="flat" onPress={() => void chooseSettingsPath("historyDbPath")}>Choose history DB</Button><Input label="Default rule file" value={draftSettings.defaultRuleFilePath ?? ""} onValueChange={(value) => setDraftSettings((current) => ({ ...current, defaultRuleFilePath: value || null }))} /><Button variant="flat" onPress={() => void chooseSettingsPath("defaultRuleFilePath")}>Choose rule file</Button><Input label="Default Git repo" value={draftSettings.defaultGitRepoPath ?? ""} onValueChange={(value) => setDraftSettings((current) => ({ ...current, defaultGitRepoPath: value || null }))} /><Button variant="flat" onPress={() => void chooseSettingsPath("defaultGitRepoPath", true)}>Choose repo</Button><Textarea label="Notes" value="Phase D2 adds timeline, run compare, range diff, and regression UI on top of the D1 desktop shell." readOnly /><Button color="primary" isLoading={savingSettings} onPress={() => void handleSaveSettings()}>Save settings</Button></CardBody></Card>
            </Tab>
          </Tabs>

          <section className="message-strip">{note ? <Card className="message-card success"><CardBody>{note}</CardBody></Card> : null}{error ? <Card className="message-card error"><CardBody>{error}</CardBody></Card> : null}</section>
        </main>
      </div>
    </div>
  );
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

function MetricList({ title, items }: { title: string; items: Array<{ name: string; value: number }> }) {
  return <div><strong>{title}</strong><ul className="metric-list">{items.slice(0, 6).map((item) => <li key={item.name}><span>{item.name}</span><span>{formatBytes(item.value)}</span></li>)}</ul></div>;
}

function DeltaList({ title, items }: { title: string; items: Array<{ name: string; delta: number }> }) {
  return <div><strong>{title}</strong><ul className="metric-list">{items.length === 0 ? <li><span>No data</span><span>-</span></li> : null}{items.slice(0, 6).map((item) => <li key={item.name}><span>{item.name}</span><span>{signed(item.delta)}</span></li>)}</ul></div>;
}
