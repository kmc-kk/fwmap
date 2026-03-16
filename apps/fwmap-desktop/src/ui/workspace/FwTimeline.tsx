import { Chip } from "@heroui/react";
import type { InvestigationTimelineEvent } from "../../lib/types";
import { formatTime } from "../../lib/format";

export function FwTimeline({ events }: { events: InvestigationTimelineEvent[] }) {
  return (
    <div className="fw-timeline">
      {events.map((eventItem) => (
        <article key={eventItem.eventId} className="fw-timeline-item">
          <div className="fw-timeline-marker" />
          <div className="fw-timeline-copy">
            <div className="fw-timeline-top">
              <strong>{eventItem.eventType}</strong>
              <Chip size="sm" variant="flat">{formatTime(eventItem.createdAt)}</Chip>
            </div>
            <code>{JSON.stringify(eventItem.payload)}</code>
          </div>
        </article>
      ))}
    </div>
  );
}
