import { useEffect, useMemo, useState } from "react";
import { Button, Card, CardBody, CardHeader, Chip, Input, Spinner } from "@heroui/react";

import { getInspectorBreakdown, getInspectorDetail, getInspectorHierarchy, getInspectorSummary, getSourceContext } from "../lib/api";
import { formatBytes } from "../lib/format";
import type { InspectorDetail, InspectorHierarchyNode, InspectorQuery, InspectorSelection, InspectorSummary, SourceContext } from "../lib/types";

type InspectorPanelProps = {
  query: InspectorQuery;
  onQueryChange: (query: InspectorQuery) => void;
  selection: InspectorSelection | null;
  onSelectionChange: (selection: InspectorSelection | null) => void;
};

type VisualMode = "treemap" | "icicle" | "table";

export function InspectorPanel({ query, onQueryChange, selection, onSelectionChange }: InspectorPanelProps) {
  const [summary, setSummary] = useState<InspectorSummary | null>(null);
  const [items, setItems] = useState<InspectorDetailRow[]>([]);
  const [hierarchy, setHierarchy] = useState<InspectorHierarchyNode[]>([]);
  const [detail, setDetail] = useState<InspectorDetail | null>(null);
  const [source, setSource] = useState<SourceContext | null>(null);
  const [loading, setLoading] = useState(false);
  const [visualMode, setVisualMode] = useState<VisualMode>("treemap");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let disposed = false;
    async function load() {
      setLoading(true);
      setError(null);
      try {
        const [nextSummary, breakdown, nextHierarchy] = await Promise.all([
          getInspectorSummary(query),
          getInspectorBreakdown(query),
          getInspectorHierarchy(query),
        ]);
        if (disposed) return;
        setSummary(nextSummary);
        setItems(breakdown.items);
        setHierarchy(nextHierarchy);
        const first = selection ?? breakdown.items[0] ? { stableId: (selection ?? breakdown.items[0]).stableId, kind: (selection ?? breakdown.items[0]).kind } : null;
        if (first) {
          onSelectionChange(first);
        } else {
          setDetail(null);
          setSource(null);
        }
      } catch (loadError) {
        if (!disposed) setError(String(loadError));
      } finally {
        if (!disposed) setLoading(false);
      }
    }
    void load();
    return () => {
      disposed = true;
    };
  }, [onSelectionChange, query]);

  useEffect(() => {
    if (!selection) {
      setDetail(null);
      setSource(null);
      return;
    }
    let disposed = false;
    async function loadSelection() {
      const currentSelection = selection;
      if (!currentSelection) return;
      try {
        const [nextDetail, nextSource] = await Promise.all([
          getInspectorDetail(query, currentSelection),
          getSourceContext(query, currentSelection),
        ]);
        if (disposed) return;
        setDetail(nextDetail);
        setSource(nextSource);
      } catch (loadError) {
        if (!disposed) setError(String(loadError));
      }
    }
    void loadSelection();
    return () => {
      disposed = true;
    };
  }, [query, selection]);

  const itemMap = useMemo(() => new Map(items.map((item) => [item.stableId, item])), [items]);

  return (
    <div className="page-stack inspector-page">
      <Card className="feature-card">
        <CardHeader className="feature-header">
          <div>
            <div className="section-header">Inspector Controls</div>
            <div className="section-subtitle">Pivot the same build context by region, file, function, symbol, or Rust ownership.</div>
          </div>
          <div className="button-row compact-wrap">
            <Button size="sm" variant={visualMode === "treemap" ? "solid" : "flat"} onPress={() => setVisualMode("treemap")}>Treemap</Button>
            <Button size="sm" variant={visualMode === "icicle" ? "solid" : "flat"} onPress={() => setVisualMode("icicle")}>Icicle</Button>
            <Button size="sm" variant={visualMode === "table" ? "solid" : "flat"} onPress={() => setVisualMode("table")}>Table</Button>
          </div>
        </CardHeader>
        <CardBody className="inspector-toolbar-grid compact-text">
          <div>
            <label>View mode</label>
            <select className="native-select" value={query.viewMode} onChange={(event) => onQueryChange({ ...query, viewMode: event.target.value as InspectorQuery["viewMode"] })}>
              <option value="region-section">Region / Section</option>
              <option value="source-file">Source File</option>
              <option value="function-symbol">Function / Symbol</option>
              <option value="crate-dependency">Crate / Dependency</option>
            </select>
          </div>
          <div>
            <label>Group by</label>
            <select className="native-select" value={query.groupBy} onChange={(event) => onQueryChange({ ...query, groupBy: event.target.value as InspectorQuery["groupBy"] })}>
              <option value="section">Section</option>
              <option value="region">Region</option>
              <option value="file">Source file</option>
              <option value="directory">Directory</option>
              <option value="function">Function</option>
              <option value="symbol">Symbol</option>
              <option value="crate">Crate</option>
              <option value="dependency">Dependency</option>
            </select>
          </div>
          <div>
            <label>Metric</label>
            <select className="native-select" value={query.metric} onChange={(event) => onQueryChange({ ...query, metric: event.target.value as InspectorQuery["metric"] })}>
              <option value="size">Absolute size</option>
              <option value="delta">Delta</option>
            </select>
          </div>
          <Input label="Search" value={query.search ?? ""} onValueChange={(value) => onQueryChange({ ...query, search: value || null })} />
          <Input label="Top N" type="number" value={String(query.topN ?? 24)} onValueChange={(value) => onQueryChange({ ...query, topN: value ? Number(value) : 24 })} />
          <div className="button-row compact-wrap inspector-toggles">
            <Button size="sm" variant={query.onlyIncreased ? "solid" : "flat"} onPress={() => onQueryChange({ ...query, onlyIncreased: !query.onlyIncreased, onlyDecreased: false })}>Only increased</Button>
            <Button size="sm" variant={query.onlyDecreased ? "solid" : "flat"} onPress={() => onQueryChange({ ...query, onlyDecreased: !query.onlyDecreased, onlyIncreased: false })}>Only decreased</Button>
            <Button size="sm" variant={query.debugInfoOnly ? "solid" : "flat"} onPress={() => onQueryChange({ ...query, debugInfoOnly: !query.debugInfoOnly })}>Debug info only</Button>
          </div>
        </CardBody>
      </Card>

      <section className="three-column inspector-summary-strip">
        <Card className="stat-card"><CardBody><div className="stat-card-content"><div className="stat-label">Context</div><div className="stat-value">{summary?.contextLabel ?? "-"}</div><div className="stat-subtitle">{summary?.sourceKind ?? "loading"}</div></div></CardBody></Card>
        <Card className="stat-card"><CardBody><div className="stat-card-content"><div className="stat-label">Entities</div><div className="stat-value">{summary?.entityCount ?? 0}</div><div className="stat-subtitle">{summary ? formatBytes(summary.totalSizeBytes) : "-"}</div></div></CardBody></Card>
        <Card className="stat-card"><CardBody><div className="stat-card-content"><div className="stat-label">Debug info</div><div className="stat-value">{summary?.debugInfoAvailable ? "Available" : "Partial"}</div><div className="stat-subtitle">Delta {summary ? signed(summary.totalDeltaBytes) : "-"}</div></div></CardBody></Card>
      </section>

      {error ? <Card className="message-card error"><CardBody>{error}</CardBody></Card> : null}

      <div className="inspector-layout">
        <Card className="feature-card">
          <CardHeader className="feature-header">
            <div>
              <div className="section-header">Visualization</div>
              <div className="section-subtitle">Click a block or row to open detail and source context.</div>
            </div>
            {loading ? <Spinner size="sm" /> : null}
          </CardHeader>
          <CardBody>
            {loading ? <div className="loading-state"><Spinner label="Loading inspector" /></div> : visualMode === "treemap" ? <TreemapView nodes={hierarchy} onSelect={onSelectionChange} /> : visualMode === "icicle" ? <IcicleView nodes={hierarchy} onSelect={onSelectionChange} /> : <InspectorTable items={items} selection={selection} onSelect={onSelectionChange} />}
          </CardBody>
        </Card>

        <Card className="feature-card">
          <CardHeader className="feature-header"><div><div className="section-header">Selection detail</div><div className="section-subtitle">Metadata, rule hits, and source context for the selected node.</div></div></CardHeader>
          <CardBody className="panel-stack compact-text inspector-detail-stack">
            {detail ? <>
              <div><strong>{detail.label}</strong> <Chip size="sm" variant="flat">{detail.kind}</Chip></div>
              <div>Size: {formatBytes(detail.sizeBytes)}</div>
              <div>Delta: <span className={detail.deltaBytes === 0 ? "delta-pill delta-pill-neutral" : detail.deltaBytes > 0 ? "delta-pill delta-pill-up" : "delta-pill delta-pill-down"}>{signed(detail.deltaBytes)}</span></div>
              {detail.parentLabel ? <div>Parent: {detail.parentLabel}</div> : null}
              <div><strong>Metadata</strong><ul className="warning-list inspector-meta-list">{Object.entries(detail.metadata).length === 0 ? <li>No metadata</li> : Object.entries(detail.metadata).map(([key, value]) => <li key={key}>{key}: {value}</li>)}</ul></div>
              <div><strong>Related rule hits</strong><ul className="warning-list">{detail.relatedRuleViolations.length === 0 ? <li>No related rule hits</li> : detail.relatedRuleViolations.map((item) => <li key={item}>{item}</li>)}</ul></div>
              <div><strong>Context</strong><ul className="warning-list">{detail.relatedRegressionEvidence.map((item) => <li key={item}>{item}</li>)}</ul></div>
            </> : <div className="empty-state compact-empty">Select an inspector item.</div>}
            <SourceContextPanel source={source} />
          </CardBody>
        </Card>
      </div>
    </div>
  );
}

type InspectorDetailRow = {
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

function InspectorTable({ items, selection, onSelect }: { items: InspectorDetailRow[]; selection: InspectorSelection | null; onSelect: (selection: InspectorSelection) => void }) {
  return <table className="data-table"><thead><tr><th>Item</th><th>Kind</th><th>Size</th><th>Delta</th></tr></thead><tbody>{items.map((item) => <tr key={item.stableId} className={selection?.stableId === item.stableId ? "inspector-row-active" : undefined} onClick={() => onSelect({ stableId: item.stableId, kind: item.kind })}><td>{item.displayLabel}</td><td>{item.kind}</td><td>{formatBytes(item.sizeBytes)}</td><td><span className={item.deltaBytes === 0 ? "delta-pill delta-pill-neutral" : item.deltaBytes > 0 ? "delta-pill delta-pill-up" : "delta-pill delta-pill-down"}>{signed(item.deltaBytes)}</span></td></tr>)}</tbody></table>;
}

function TreemapView({ nodes, onSelect }: { nodes: InspectorHierarchyNode[]; onSelect: (selection: InspectorSelection) => void }) {
  const total = nodes.reduce((sum, item) => sum + item.sizeBytes, 0) || 1;
  return <div className="inspector-treemap">{nodes.map((node) => <button key={node.stableId} type="button" className="treemap-node" style={{ flexGrow: Math.max(node.sizeBytes, 1), flexBasis: `${Math.max((node.sizeBytes / total) * 100, 12)}%` }} onClick={() => onSelect({ stableId: node.stableId, kind: node.kind })}><span>{node.label}</span><strong>{formatBytes(node.sizeBytes)}</strong>{node.children.length > 0 ? <em>{node.children.length} children</em> : null}</button>)}</div>;
}

function IcicleView({ nodes, onSelect }: { nodes: InspectorHierarchyNode[]; onSelect: (selection: InspectorSelection) => void }) {
  return <div className="inspector-icicle">{nodes.map((node) => <div key={node.stableId} className="icicle-branch"><button type="button" className="icicle-node depth-1" onClick={() => onSelect({ stableId: node.stableId, kind: node.kind })}><span>{node.label}</span><strong>{formatBytes(node.sizeBytes)}</strong></button>{node.children.length > 0 ? <div className="icicle-children">{node.children.map((child) => <button key={child.stableId} type="button" className="icicle-node depth-2" onClick={() => onSelect({ stableId: child.stableId, kind: child.kind })}><span>{child.label}</span><strong>{formatBytes(child.sizeBytes)}</strong></button>)}</div> : null}</div>)}</div>;
}

function SourceContextPanel({ source }: { source: SourceContext | null }) {
  if (!source) {
    return <div className="empty-state compact-empty">Source context appears here.</div>;
  }
  return <div className="source-context-panel"><strong>Source context</strong><ul className="warning-list">{source.path ? <li>Path: {source.path}</li> : null}{source.functionName ? <li>Function: {source.functionName}</li> : null}{source.compileUnit ? <li>Compile unit: {source.compileUnit}</li> : null}{source.crateName ? <li>Crate: {source.crateName}</li> : null}{source.excerpt ? <li>{source.excerpt}</li> : null}{source.availabilityReason ? <li>{source.availabilityReason}</li> : null}{!source.path && !source.functionName && !source.excerpt && !source.availabilityReason ? <li>No source metadata available.</li> : null}</ul></div>;
}

function signed(value: number): string {
  return `${value >= 0 ? "+" : ""}${value}`;
}
