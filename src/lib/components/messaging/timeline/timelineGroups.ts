import type { TimelineMessage } from "../shared";

export type MessageGroup = {
  kind: "group";
  key: string;
  sender: string;
  messages: { key: string; message: TimelineMessage }[];
};
export type TimelineGroupRow = MessageGroup | { kind: "separator"; key: string; timestamp: number };
const GROUPING_WINDOW_MS = 5 * 60 * 1000;

// Reconcile by overlapping messages so prepending history or appending a new
// message does not remount an existing group and disturb its scroll anchor.
export function createTimelineGrouper() {
  let previous: TimelineGroupRow[] = [];
  return (messages: TimelineMessage[]): TimelineGroupRow[] => {
    const previousKeys = new Map<string, string>();
    for (const row of previous) {
      if (row.kind === "group") for (const item of row.messages) previousKeys.set(item.key, row.key);
    }
    const rows: TimelineGroupRow[] = [];
    let lastMessage: TimelineMessage | undefined;
    let lastDay: string | null = null;
    let group: MessageGroup | undefined;
    for (const [index, message] of messages.entries()) {
      const key = message.localId ?? message.eventId ?? `${index}-${message.timestamp ?? 0}`;
      const day = message.timestamp != null ? new Date(message.timestamp).toDateString() : null;
      if (day && day !== lastDay) rows.push({ kind: "separator", key: `separator-${key}`, timestamp: message.timestamp! });
      const gap = lastMessage?.timestamp != null && message.timestamp != null
        ? message.timestamp - lastMessage.timestamp : null;
      const continues = group && lastMessage?.sender === message.sender && day && day === lastDay &&
        gap !== null && gap >= 0 && gap < GROUPING_WINDOW_MS;
      if (!continues) {
        group = { kind: "group", key: `group-${key}`, sender: message.sender, messages: [] };
        rows.push(group);
      }
      group!.messages.push({ key, message });
      lastMessage = message;
      lastDay = day;
    }
    const used = new Set<string>();
    for (const row of rows) {
      if (row.kind !== "group") continue;
      const existing = row.messages.map((item) => previousKeys.get(item.key)).find((key) => key && !used.has(key));
      if (existing) row.key = existing;
      used.add(row.key);
    }
    previous = rows;
    return rows;
  };
}
