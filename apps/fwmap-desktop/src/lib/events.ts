import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { JobEvent } from "./types";

export async function listenToJobEvents(handlers: {
  onCreated?: (event: JobEvent) => void;
  onProgress?: (event: JobEvent) => void;
  onFinished?: (event: JobEvent) => void;
  onFailed?: (event: JobEvent) => void;
}): Promise<UnlistenFn[]> {
  const listeners: Promise<UnlistenFn>[] = [];

  if (handlers.onCreated) {
    listeners.push(listen<JobEvent>("job-created", (event) => handlers.onCreated?.(event.payload)));
  }
  if (handlers.onProgress) {
    listeners.push(listen<JobEvent>("job-progress", (event) => handlers.onProgress?.(event.payload)));
  }
  if (handlers.onFinished) {
    listeners.push(listen<JobEvent>("job-finished", (event) => handlers.onFinished?.(event.payload)));
  }
  if (handlers.onFailed) {
    listeners.push(listen<JobEvent>("job-failed", (event) => handlers.onFailed?.(event.payload)));
  }

  return Promise.all(listeners);
}
