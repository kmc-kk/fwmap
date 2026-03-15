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

export type HistoryQuery = {
  repoPath?: string | null;
  branch?: string | null;
  profile?: string | null;
  toolchain?: string | null;
  target?: string | null;
  limit?: number | null;
  order?: "ancestry" | "timestamp" | null;
};

export type DashboardQuery = HistoryQuery;

export type HistoryItem = {
  buildId: number;
  createdAt: string;
  elfPath: string;
  arch: string;
  linkerFamily: string;
  mapFormat: string;
  romBytes: number;
  ramBytes: number;
  warningCount: number;
  errorCount: number;
  gitRevision: string | null;
  gitBranch: string | null;
  gitSubject: string | null;
  gitDescribe: string | null;
  profile: string | null;
  target: string | null;
  toolchainId: string | null;
  label: string | null;
};

export type DeltaEntry = {
  name: string;
  delta: number;
};

export type TimelineEntry = {
  commit: string;
  shortCommit: string;
  commitTime: string;
  authorName: string;
  subject: string;
  branchNames: string[];
  tagNames: string[];
  describe: string | null;
  buildProfile: string | null;
  toolchainId: string | null;
  targetId: string | null;
  romTotal: number;
  ramTotal: number;
  romDeltaVsPrevious: number | null;
  ramDeltaVsPrevious: number | null;
  ruleViolationsCount: number;
  topSections: DeltaEntry[];
  topObjects: DeltaEntry[];
  topSourceFiles: DeltaEntry[];
  topSymbols: DeltaEntry[];
};

export type TimelineResult = {
  repoId: string;
  order: string;
  branch: string | null;
  profile: string | null;
  toolchain: string | null;
  target: string | null;
  rows: TimelineEntry[];
};

export type RunCompareRequest = {
  leftRunId: number;
  rightRunId: number;
};

export type MetricSummary = {
  romDelta: number;
  ramDelta: number;
  warningDelta: number;
};

export type RunCompareResult = {
  leftRun: RunSummary;
  rightRun: RunSummary;
  summary: MetricSummary;
  regionDeltas: DeltaEntry[];
  sectionDeltas: DeltaEntry[];
  objectDeltas: DeltaEntry[];
  sourceFileDeltas: DeltaEntry[];
  symbolDeltas: DeltaEntry[];
  rustDependencyDeltas: DeltaEntry[];
  rustFamilyDeltas: DeltaEntry[];
};

export type GitRef = {
  name: string;
  kind: string;
};

export type ChangedFilesSummary = {
  gitChangedFiles: string[];
  changedSourceFilesInAnalysis: string[];
  intersectionFiles: string[];
  gitOnlyFilesCount: number;
  analysisOnlyFilesCount: number;
  intersectionCount: number;
};

export type WorstCommitSummary = {
  commit: string;
  delta: number;
  subject: string;
  date: string;
};

export type FirstRuleViolationSummary = {
  commit: string;
  ruleIds: string[];
  subject: string;
};

export type RangeDiffQuery = {
  repoPath?: string | null;
  spec: string;
  includeChangedFiles?: boolean | null;
  order?: "ancestry" | "timestamp" | null;
  profile?: string | null;
  toolchain?: string | null;
  target?: string | null;
};

export type RangeDiffResult = {
  repoId: string;
  inputRangeSpec: string;
  comparisonMode: string;
  resolvedBase: string;
  resolvedHead: string;
  resolvedMergeBase: string | null;
  order: string;
  totalCommitsInGitRange: number;
  analyzedCommitsCount: number;
  missingAnalysisCommitsCount: number;
  cumulativeRomDelta: number;
  cumulativeRamDelta: number;
  worstCommitByRom: WorstCommitSummary | null;
  worstCommitByRam: WorstCommitSummary | null;
  firstRuleViolation: FirstRuleViolationSummary | null;
  topChangedSections: DeltaEntry[];
  topChangedObjects: DeltaEntry[];
  topChangedSourceFiles: DeltaEntry[];
  topChangedSymbols: DeltaEntry[];
  topChangedRustDependencies: DeltaEntry[];
  topChangedRustFamilies: DeltaEntry[];
  changedFilesSummary: ChangedFilesSummary | null;
  timelineRows: TimelineEntry[];
};

export type RegressionQuery = {
  repoPath?: string | null;
  spec: string;
  detectorType: "metric" | "rule" | "entity";
  key: string;
  mode: "first-crossing" | "first-jump" | "first-presence" | "first-violation";
  threshold?: number | null;
  thresholdPercent?: number | null;
  jumpThreshold?: number | null;
  order?: "ancestry" | "timestamp" | null;
  includeEvidence?: boolean | null;
  includeChangedFiles?: boolean | null;
  bisectLike?: boolean | null;
  maxSteps?: number | null;
  limitCommits?: number | null;
  profile?: string | null;
  toolchain?: string | null;
  target?: string | null;
};

export type RegressionOriginPoint = {
  commit: string;
  shortCommit: string;
  subject: string;
  value: number | null;
};

export type RegressionWindowRow = {
  commit: string;
  shortCommit: string;
  subject: string;
  status: string;
  value: number | null;
};

export type RegressionResult = {
  repoId: string;
  detectorType: string;
  key: string;
  mode: string;
  confidence: string;
  reasoning: string;
  searchedCommitCount: number;
  analyzedCommitCount: number;
  missingAnalysisCount: number;
  mixedConfiguration: boolean;
  lastGood: RegressionOriginPoint | null;
  firstObservedBad: RegressionOriginPoint | null;
  firstBadCandidate: RegressionOriginPoint | null;
  transitionWindow: RegressionWindowRow[];
  topGrowthSections: DeltaEntry[];
  topGrowthObjects: DeltaEntry[];
  topGrowthSourceFiles: DeltaEntry[];
  topGrowthSymbols: DeltaEntry[];
  changedFilesSummary: ChangedFilesSummary | null;
  relatedRuleHits: string[];
  narrowedCommits: string[];
};

export type OverviewCard = {
  key: string;
  title: string;
  value: string;
  subtitle: string | null;
  tone: string;
};

export type TrendPoint = {
  label: string;
  value: number;
  secondaryValue: number | null;
};

export type TrendSeries = {
  key: string;
  label: string;
  unit: string;
  points: TrendPoint[];
};

export type TopGrowthEntry = {
  scope: string;
  name: string;
  delta: number;
  detail: string | null;
};

export type RecentRegression = {
  detectorType: string;
  key: string;
  confidence: string;
  commit: string;
  subject: string;
  reasoning: string;
};

export type RegionUsage = {
  regionName: string;
  usedBytes: number;
  freeBytes: number;
  usageRatio: number;
};

export type DashboardSummary = {
  overviewCards: OverviewCard[];
  latestRun: RunSummary | null;
  latestHistoryItem: HistoryItem | null;
  recentTrends: TrendSeries[];
  recentRegressions: RecentRegression[];
  topGrowthSources: TopGrowthEntry[];
  regionUsage: RegionUsage[];
};


export type ProjectSummary = {
  projectId: number;
  name: string;
  rootPath: string;
  gitRepoPath: string | null;
  defaultRuleFilePath: string | null;
  defaultTarget: string | null;
  defaultProfile: string | null;
  lastRunAt: string | null;
  lastExportAt: string | null;
};

export type ProjectDetail = {
  projectId: number;
  name: string;
  rootPath: string;
  gitRepoPath: string | null;
  defaultElfPath: string | null;
  defaultMapPath: string | null;
  defaultDebugPath: string | null;
  defaultRuleFilePath: string | null;
  defaultTarget: string | null;
  defaultProfile: string | null;
  defaultExportDir: string | null;
  pinnedReportPath: string | null;
  lastOpenedScreen: string | null;
  lastOpenedFiltersJson: string | null;
  createdAt: string;
  updatedAt: string;
};

export type CreateProjectRequest = {
  name: string;
  rootPath: string;
  gitRepoPath: string | null;
  defaultElfPath: string | null;
  defaultMapPath: string | null;
  defaultDebugPath: string | null;
  defaultRuleFilePath: string | null;
  defaultTarget: string | null;
  defaultProfile: string | null;
  defaultExportDir: string | null;
};

export type UpdateProjectRequest = Partial<CreateProjectRequest> & {
  pinnedReportPath?: string | null;
  lastOpenedScreen?: string | null;
  lastOpenedFiltersJson?: string | null;
};

export type ActiveProjectState = {
  activeProjectId: number | null;
  activeProject: ProjectDetail | null;
};

export type PolicyDocument = {
  path: string | null;
  format: string;
  content: string;
  projectId: number | null;
};

export type PolicyValidationIssue = {
  level: string;
  message: string;
  line: number | null;
};

export type PolicyValidationResult = {
  ok: boolean;
  issues: PolicyValidationIssue[];
};

export type ExportRequest = {
  projectId?: number | null;
  exportTarget: "run" | "diff" | "history" | "regression" | "dashboard";
  format: "html" | "json" | "print-html";
  destinationPath: string;
  runId?: number | null;
  compare?: RunCompareRequest | null;
  historyQuery?: HistoryQuery | null;
  rangeQuery?: RangeDiffQuery | null;
  regressionQuery?: RegressionQuery | null;
  dashboardQuery?: DashboardQuery | null;
  title?: string | null;
};

export type ExportResult = {
  destinationPath: string;
  exportTarget: string;
  format: string;
  createdAt: string;
};

export type RecentExport = {
  exportId: number;
  projectId: number | null;
  createdAt: string;
  exportTarget: string;
  format: string;
  destinationPath: string;
  title: string;
};


export type InspectorQuery = {
  runId?: number | null;
  buildId?: number | null;
  leftRunId?: number | null;
  rightRunId?: number | null;
  viewMode: "region-section" | "source-file" | "function-symbol" | "crate-dependency";
  groupBy: "region" | "section" | "file" | "directory" | "function" | "symbol" | "crate" | "dependency";
  metric: "size" | "delta";
  search?: string | null;
  topN?: number | null;
  thresholdMin?: number | null;
  onlyIncreased?: boolean | null;
  onlyDecreased?: boolean | null;
  debugInfoOnly?: boolean | null;
};

export type InspectorSelection = {
  stableId: string;
  kind: string;
};

export type InspectorSummary = {
  contextLabel: string;
  sourceKind: string;
  entityCount: number;
  totalSizeBytes: number;
  totalDeltaBytes: number;
  debugInfoAvailable: boolean;
  availableViews: string[];
  availableVisualizations: string[];
};

export type InspectorItem = {
  stableId: string;
  displayLabel: string;
  rawLabel: string;
  kind: string;
  sizeBytes: number;
  deltaBytes: number;
  percentage: number;
  parentId: string | null;
  hasChildren: boolean;
  sourceAvailable: boolean;
  metadata: Record<string, string>;
};

export type InspectorBreakdown = {
  query: InspectorQuery;
  items: InspectorItem[];
};

export type InspectorHierarchyNode = {
  stableId: string;
  label: string;
  kind: string;
  sizeBytes: number;
  deltaBytes: number;
  percentage: number;
  sourceAvailable: boolean;
  children: InspectorHierarchyNode[];
};

export type InspectorDetail = {
  stableId: string;
  label: string;
  kind: string;
  sizeBytes: number;
  deltaBytes: number;
  parentLabel: string | null;
  sourceAvailable: boolean;
  metadata: Record<string, string>;
  relatedRuleViolations: string[];
  relatedRegressionEvidence: string[];
};

export type SourceContext = {
  path: string | null;
  functionName: string | null;
  lineStart: number | null;
  lineEnd: number | null;
  excerpt: string | null;
  compileUnit: string | null;
  crateName: string | null;
  relatedSections: string[];
  relatedRegions: string[];
  availabilityReason: string | null;
};
