import { useEffect, useMemo, useState } from "react";
import {
  Button,
  Card,
  CardBody,
  CardHeader,
  Chip,
  Input,
  Navbar,
  NavbarBrand,
  Spinner,
  Tab,
  Tabs,
  Textarea,
} from "@heroui/react";
import { open } from "@tauri-apps/plugin-dialog";

import {
  cancelJob,
  getAppInfo,
  getRunDetail,
  getSettings,
  listRecentRuns,
  saveSettings,
  startAnalysis,
} from "./lib/api";
import { listenToJobEvents } from "./lib/events";
import { formatBytes, formatTime, joinParts } from "./lib/format";
import type {
  AnalysisRequest,
  DesktopAppInfo,
  DesktopSettings,
  JobEvent,
  JobStatus,
  RunDetail,
  RunSummary,
} from "./lib/types";

type ScreenKey = "dashboard" | "runs" | "settings";

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
  const [busy, setBusy] = useState(true);
  const [savingSettings, setSavingSettings] = useState(false);
  const [starting, setStarting] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    async function load() {
      setBusy(true);
      try {
        const [info, loadedSettings, loadedRuns] = await Promise.all([
          getAppInfo(),
          getSettings(),
          listRecentRuns(20, 0),
        ]);
        if (disposed) {
          return;
        }
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
        setRuns(loadedRuns);
        if (loadedRuns.length > 0) {
          setSelectedRunId((current) => current ?? loadedRuns[0].runId);
        }
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
        if (!disposed) {
          setRunDetail(detail);
        }
      } catch (loadError) {
        if (!disposed) {
          setError(String(loadError));
        }
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
      onCreated: (event) => handleJobEvent(event),
      onProgress: (event) => handleJobEvent(event),
      onFinished: (event) => handleJobEvent(event),
      onFailed: (event) => handleJobEvent(event),
    }).then((items) => {
      unlisteners = items;
    });
    return () => {
      for (const dispose of unlisteners) {
        dispose();
      }
    };
  }, []);

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
      setNote("Analysis finished. Recent runs were refreshed.");
      void refreshRuns(event.runId ?? undefined);
    }
    if (event.status === "failed") {
      setError(event.errorMessage ?? "Analysis failed.");
    }
  }

  async function refreshRuns(preferredRunId?: number) {
    const loadedRuns = await listRecentRuns(20, 0);
    setRuns(loadedRuns);
    const nextRunId = preferredRunId ?? selectedRunId ?? loadedRuns[0]?.runId ?? null;
    setSelectedRunId(nextRunId);
    if (nextRunId) {
      const detail = await getRunDetail(nextRunId);
      setRunDetail(detail);
    }
  }

  const latestRun = runs[0] ?? null;
  const dashboardStats = useMemo(
    () => [
      { label: "Recent runs", value: String(runs.length) },
      { label: "Latest ROM", value: latestRun ? formatBytes(latestRun.romBytes) : "-" },
      { label: "Latest RAM", value: latestRun ? formatBytes(latestRun.ramBytes) : "-" },
      { label: "Warnings", value: latestRun ? String(latestRun.warningCount) : "-" },
    ],
    [latestRun, runs.length],
  );

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
      setRequest((current) => ({
        ...current,
        ruleFilePath: current.ruleFilePath ?? saved.defaultRuleFilePath,
        gitRepoPath: current.gitRepoPath ?? saved.defaultGitRepoPath,
      }));
      setNote("Settings saved.");
    } catch (saveError) {
      setError(String(saveError));
    } finally {
      setSavingSettings(false);
    }
  }

  async function handleCancelJob() {
    if (!job) {
      return;
    }
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

  return (
    <div className="app-shell">
      <Navbar maxWidth="full" className="topbar">
        <NavbarBrand>
          <div>
            <div className="brand-title">fwmap desktop</div>
            <div className="brand-subtitle">Tauri D1 shell for local ELF/map analysis</div>
          </div>
        </NavbarBrand>
        <div className="topbar-meta">
          <Chip variant="flat">CLI {appInfo?.cliVersion ?? "-"}</Chip>
          <Chip variant="flat">App {appInfo?.appVersion ?? "-"}</Chip>
        </div>
      </Navbar>

      <div className="app-grid">
        <aside className="sidebar">
          <Card className="sidebar-card">
            <CardHeader className="section-header">Start Analysis</CardHeader>
            <CardBody className="panel-stack">
              <Input
                label="ELF path"
                value={request.elfPath ?? ""}
                onValueChange={(value) => setRequest((current) => ({ ...current, elfPath: value || null }))}
              />
              <Button variant="flat" onPress={() => void chooseFile("elfPath")}>Choose ELF</Button>
              <Input
                label="Map path"
                value={request.mapPath ?? ""}
                onValueChange={(value) => setRequest((current) => ({ ...current, mapPath: value || null }))}
              />
              <Button variant="flat" onPress={() => void chooseFile("mapPath")}>Choose map</Button>
              <Input
                label="Rule file"
                value={request.ruleFilePath ?? ""}
                onValueChange={(value) => setRequest((current) => ({ ...current, ruleFilePath: value || null }))}
              />
              <Button variant="flat" onPress={() => void chooseFile("ruleFilePath")}>Choose rule file</Button>
              <Input
                label="Git repo"
                value={request.gitRepoPath ?? ""}
                onValueChange={(value) => setRequest((current) => ({ ...current, gitRepoPath: value || null }))}
              />
              <Button variant="flat" onPress={() => void chooseFile("gitRepoPath", true)}>Choose repo</Button>
              <Input
                label="Label"
                value={request.label ?? ""}
                onValueChange={(value) => setRequest((current) => ({ ...current, label: value || null }))}
              />
              <Button color="primary" isLoading={starting} onPress={() => void handleStartAnalysis()}>
                Start analysis
              </Button>
              <Button variant="bordered" isDisabled={!job} onPress={() => void handleCancelJob()}>
                Cancel job
              </Button>
            </CardBody>
          </Card>

          <Card className="sidebar-card muted-card">
            <CardHeader className="section-header">Environment</CardHeader>
            <CardBody className="panel-stack compact-text">
              <div>History DB</div>
              <code>{settings.historyDbPath || appInfo?.historyDbPath || "-"}</code>
              <div>App DB</div>
              <code>{appInfo?.appDbPath ?? "-"}</code>
            </CardBody>
          </Card>
        </aside>

        <main className="content">
          <Tabs
            selectedKey={screen}
            onSelectionChange={(key) => setScreen(key as ScreenKey)}
            variant="underlined"
            className="main-tabs"
          >
            <Tab key="dashboard" title="Dashboard">
              {busy ? (
                <div className="loading-state"><Spinner label="Loading dashboard" /></div>
              ) : (
                <div className="page-stack">
                  <section className="stats-grid">
                    {dashboardStats.map((item) => (
                      <Card key={item.label} className="stat-card">
                        <CardBody>
                          <div className="stat-label">{item.label}</div>
                          <div className="stat-value">{item.value}</div>
                        </CardBody>
                      </Card>
                    ))}
                  </section>

                  <section className="two-column">
                    <Card>
                      <CardHeader className="section-header">Current Job</CardHeader>
                      <CardBody className="panel-stack compact-text">
                        <div>Status: <strong>{job?.status ?? "idle"}</strong></div>
                        <div>Message: {job?.progressMessage ?? "No active job"}</div>
                        <div>Started: {formatTime(job?.createdAt)}</div>
                        <div>Updated: {formatTime(job?.updatedAt)}</div>
                        <div>Run ID: {job?.runId ?? "-"}</div>
                      </CardBody>
                    </Card>

                    <Card>
                      <CardHeader className="section-header">Latest Run</CardHeader>
                      <CardBody className="panel-stack compact-text">
                        <div>When: {formatTime(latestRun?.createdAt)}</div>
                        <div>Git: {latestRun?.gitRevision ?? "-"}</div>
                        <div>Profile / Target: {joinParts([latestRun?.profile, latestRun?.target]) || "-"}</div>
                        <div>ROM / RAM: {latestRun ? `${formatBytes(latestRun.romBytes)} / ${formatBytes(latestRun.ramBytes)}` : "-"}</div>
                        <div>Warnings: {latestRun?.warningCount ?? "-"}</div>
                      </CardBody>
                    </Card>
                  </section>
                </div>
              )}
            </Tab>

            <Tab key="runs" title="Runs">
              <div className="runs-layout">
                <Card>
                  <CardHeader className="section-header">Recent Runs</CardHeader>
                  <CardBody className="list-stack">
                    {runs.map((run) => (
                      <button
                        key={run.runId}
                        className={`run-row ${selectedRunId === run.runId ? "selected" : ""}`}
                        onClick={() => setSelectedRunId(run.runId)}
                        type="button"
                      >
                        <div className="run-row-top">
                          <strong>#{run.runId}</strong>
                          <Chip size="sm" variant="flat">{run.status}</Chip>
                        </div>
                        <div>{run.label || run.gitRevision || "Unnamed run"}</div>
                        <div className="run-meta">
                          {formatTime(run.createdAt)}
                          <span>{formatBytes(run.romBytes)} ROM</span>
                          <span>{formatBytes(run.ramBytes)} RAM</span>
                        </div>
                      </button>
                    ))}
                    {runs.length === 0 ? <div className="empty-state">No runs recorded yet.</div> : null}
                  </CardBody>
                </Card>

                <Card>
                  <CardHeader className="section-header">Run Detail</CardHeader>
                  <CardBody className="page-stack compact-text">
                    {runDetail ? (
                      <>
                        <div className="detail-grid">
                          <div><strong>ELF</strong><br />{runDetail.elfPath || "-"}</div>
                          <div><strong>Arch</strong><br />{runDetail.arch || "-"}</div>
                          <div><strong>Linker</strong><br />{joinParts([runDetail.linkerFamily, runDetail.mapFormat]) || "-"}</div>
                          <div><strong>Git</strong><br />{joinParts([runDetail.run.gitRevision, runDetail.gitBranch, runDetail.gitDescribe]) || "-"}</div>
                        </div>
                        <div>
                          <strong>Top sections</strong>
                          <ul className="metric-list">
                            {runDetail.topSections.slice(0, 5).map(([name, size]) => (
                              <li key={name}><span>{name}</span><span>{formatBytes(size)}</span></li>
                            ))}
                          </ul>
                        </div>
                        <div>
                          <strong>Top symbols</strong>
                          <ul className="metric-list">
                            {runDetail.topSymbols.slice(0, 5).map(([name, size]) => (
                              <li key={name}><span>{name}</span><span>{formatBytes(size)}</span></li>
                            ))}
                          </ul>
                        </div>
                        <div>
                          <strong>Rule warnings</strong>
                          <ul className="warning-list">
                            {runDetail.warnings.length === 0 ? <li>No rule warnings recorded.</li> : null}
                            {runDetail.warnings.slice(0, 8).map(([code, level, related], index) => (
                              <li key={`${code}-${index}`}>{level} / {code}{related ? ` / ${related}` : ""}</li>
                            ))}
                          </ul>
                        </div>
                      </>
                    ) : (
                      <div className="empty-state">Select a run to inspect its summary.</div>
                    )}
                  </CardBody>
                </Card>
              </div>
            </Tab>

            <Tab key="settings" title="Settings">
              <Card>
                <CardHeader className="section-header">Desktop Settings</CardHeader>
                <CardBody className="panel-stack">
                  <Input
                    label="History DB path"
                    value={draftSettings.historyDbPath}
                    onValueChange={(value) => setDraftSettings((current) => ({ ...current, historyDbPath: value }))}
                  />
                  <Button variant="flat" onPress={() => void chooseSettingsPath("historyDbPath")}>Choose history DB</Button>
                  <Input
                    label="Default rule file"
                    value={draftSettings.defaultRuleFilePath ?? ""}
                    onValueChange={(value) => setDraftSettings((current) => ({ ...current, defaultRuleFilePath: value || null }))}
                  />
                  <Button variant="flat" onPress={() => void chooseSettingsPath("defaultRuleFilePath")}>Choose rule file</Button>
                  <Input
                    label="Default Git repo"
                    value={draftSettings.defaultGitRepoPath ?? ""}
                    onValueChange={(value) => setDraftSettings((current) => ({ ...current, defaultGitRepoPath: value || null }))}
                  />
                  <Button variant="flat" onPress={() => void chooseSettingsPath("defaultGitRepoPath", true)}>Choose repo</Button>
                  <Textarea
                    label="Notes"
                    value="Phase D1 persists desktop settings locally and keeps analysis history in the existing fwmap history DB."
                    readOnly
                  />
                  <Button color="primary" isLoading={savingSettings} onPress={() => void handleSaveSettings()}>
                    Save settings
                  </Button>
                </CardBody>
              </Card>
            </Tab>
          </Tabs>

          <section className="message-strip">
            {note ? <Card className="message-card success"><CardBody>{note}</CardBody></Card> : null}
            {error ? <Card className="message-card error"><CardBody>{error}</CardBody></Card> : null}
          </section>
        </main>
      </div>
    </div>
  );
}
