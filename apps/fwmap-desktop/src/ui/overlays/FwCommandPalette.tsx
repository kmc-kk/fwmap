import { Button, Input } from "@heroui/react";
import { useEffect, useMemo, useState } from "react";

type CommandItem = { id: string; title: string; subtitle: string; onSelect: () => void; group: string };

export function FwCommandPalette({ open, items, onClose }: { open: boolean; items: CommandItem[]; onClose: () => void }) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return normalized
      ? items.filter((item) => `${item.title} ${item.subtitle} ${item.group}`.toLowerCase().includes(normalized))
      : items;
  }, [items, query]);

  useEffect(() => {
    if (!open) {
      setQuery("");
    }
  }, [open]);

  useEffect(() => {
    if (!open) return;
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        onClose();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose, open]);

  if (!open) return null;

  const grouped = new Map<string, CommandItem[]>();
  for (const item of filtered) {
    const list = grouped.get(item.group) ?? [];
    list.push(item);
    grouped.set(item.group, list);
  }

  return (
    <div className="fw-command-palette-backdrop" role="presentation" onClick={onClose}>
      <section className="fw-command-palette" role="dialog" aria-modal="true" aria-label="Command palette" onClick={(event) => event.stopPropagation()}>
        <Input autoFocus label="Search" placeholder="Jump to screens, investigations, or packages" value={query} onValueChange={setQuery} />
        <div className="fw-command-results">
          {grouped.size === 0 ? <div className="fw-empty-state"><strong>No results</strong><p>Try a broader query or close the palette with Esc.</p></div> : null}
          {Array.from(grouped.entries()).map(([group, entries]) => (
            <div key={group} className="fw-command-group">
              <div className="fw-command-group-label">{group}</div>
              {entries.map((entry) => (
                <button key={entry.id} type="button" className="fw-command-item" onClick={() => { entry.onSelect(); onClose(); }}>
                  <strong>{entry.title}</strong>
                  <span>{entry.subtitle}</span>
                </button>
              ))}
            </div>
          ))}
        </div>
        <div className="button-row compact-wrap"><Button variant="flat" onPress={onClose}>Close</Button></div>
      </section>
    </div>
  );
}
