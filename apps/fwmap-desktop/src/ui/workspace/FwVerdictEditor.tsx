import { Button, Input, Textarea } from "@heroui/react";
import type { InvestigationVerdict } from "../../lib/types";

export function FwVerdictEditor({ value, onChange, onSave }: { value: Omit<InvestigationVerdict, "investigationId" | "updatedAt">; onChange: (value: Omit<InvestigationVerdict, "investigationId" | "updatedAt">) => void; onSave: () => void }) {
  return (
    <div className="panel-stack compact-text">
      <div className="form-grid-inline">
        <div>
          <label>Verdict type</label>
          <select className="native-select" value={value.verdictType} onChange={(event) => onChange({ ...value, verdictType: event.target.value })}>
            <option value="code change">code change</option>
            <option value="compiler/codegen change">compiler/codegen change</option>
            <option value="linker layout change">linker layout change</option>
            <option value="dependency update">dependency update</option>
            <option value="build/config change">build/config change</option>
            <option value="mixed">mixed</option>
            <option value="unknown">unknown</option>
          </select>
        </div>
        <Input label="Confidence" type="number" value={String(value.confidence)} onValueChange={(next) => onChange({ ...value, confidence: Number(next) || 0 })} />
      </div>
      <Textarea minRows={3} label="Summary" value={value.summary} onValueChange={(summary) => onChange({ ...value, summary })} />
      <Textarea minRows={3} label="Unresolved questions" value={value.unresolvedQuestions} onValueChange={(unresolvedQuestions) => onChange({ ...value, unresolvedQuestions })} />
      <Textarea minRows={3} label="Next actions" value={value.nextActions} onValueChange={(nextActions) => onChange({ ...value, nextActions })} />
      <Button color="primary" onPress={onSave}>Save verdict</Button>
    </div>
  );
}
