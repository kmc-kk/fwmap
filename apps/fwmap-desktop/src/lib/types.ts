export type DesktopAppInfo = {
  appName: string;
  appVersion: string;
  cliVersion: string;
  historyDbPath: string;
  appDbPath: string;
};

export type AnalysisRequest = {
  elfPath: string | null;
  mapPath: string | null;
  debugPath: string | null;
  ruleFilePath: string | null;
  gitRepoPath: string | null;
  label: string | null;
};

export type DesktopSettings = {
  historyDbPath: string;
  defaultRuleFilePath: string | null;
  defaultGitRepoPath: string | null;
  lastElfPath: string | null;
  lastMapPath: string | null;
};

export type JobStatus = {
  jobId: string;
  status: string;
  createdAt: string;
  updatedAt: string;
  label: string | null;
  progressMessage: string;
  errorMessage: string | null;
  runId: number | null;
};

export type JobEvent = {
  jobId: string;
  status: string;
  message: string;
  runId: number | null;
  errorMessage: string | null;
};

export type RunSummary = {
  runId: number;
  buildId: number;
  createdAt: string;
  label: string | null;
  status: string;
  gitRevision: string | null;
  profile: string | null;
  target: string | null;
  romBytes: number;
  ramBytes: number;
  warningCount: number;
};

export type RunDetail = {
  run: RunSummary;
  elfPath: string;
  arch: string;
  linkerFamily: string;
  mapFormat: string;
  reportHtmlPath: string | null;
  reportJsonPath: string | null;
  gitBranch: string | null;
  gitDescribe: string | null;
  topSections: Array<[string, number]>;
  topSymbols: Array<[string, number]>;
  warnings: Array<[string, string, string | null]>;
};
