import { Input } from "@heroui/react";
import { useEffect, useState } from "react";

export function FwSearchField({ label, placeholder, value, onValueChange, debounceMs = 120 }: { label?: string; placeholder?: string; value: string; onValueChange: (value: string) => void; debounceMs?: number }) {
  const [draft, setDraft] = useState(value);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  useEffect(() => {
    const handle = window.setTimeout(() => {
      if (draft !== value) {
        onValueChange(draft);
      }
    }, debounceMs);
    return () => window.clearTimeout(handle);
  }, [debounceMs, draft, onValueChange, value]);

  return (
    <Input
      aria-label={label ?? placeholder ?? "Search"}
      label={label}
      placeholder={placeholder}
      value={draft}
      onValueChange={setDraft}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          setDraft("");
          onValueChange("");
        }
      }}
    />
  );
}
