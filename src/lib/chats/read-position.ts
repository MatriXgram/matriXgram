import type { MatrixChatMessage } from "./types";
// A real receipt wins over a count estimate (state/reaction events also affect
// server counts). Keep the divider fixed for the lifetime of an open room.
export function isUnreadMessage(message: MatrixChatMessage): boolean {
  // Membership changes are rendered as timeline rows, but Matrix unread
  // counts and read receipts are about room messages, not join/leave notices.
  return message.messageType !== "m.room.member";
}

/**
 * Advance a local read cursor only forwards through the currently ordered
 * timeline. Matrix event IDs are opaque, so their timeline position (rather
 * than arithmetic on the ID) is the high-water mark.
 */
export function advanceReadWatermark<T extends { eventId?: string | null }>(
  messages: T[],
  currentEventId: string | null,
  candidateIndex: number,
): string | null {
  const candidateId = messages[candidateIndex]?.eventId;
  if (!candidateId) return currentEventId;

  const currentIndex = currentEventId
    ? messages.findIndex((message) => message.eventId === currentEventId)
    : -1;
  if (currentIndex >= 0 && candidateIndex <= currentIndex) return currentEventId;
  return candidateId;
}

/** Count unread rows available in the loaded timeline, not hidden sync events. */
export function countVisibleUnreadMessages(
  messages: MatrixChatMessage[],
  marker: string | null,
  serverUnreadCount: number,
  userId: string,
  consumedEventIds: Set<string> = new Set(),
  newUnreadEventIds: Set<string> = new Set(),
): number {
  const markerIndex = marker ? messages.findIndex((message) => message.eventId === marker) : -1;
  let startIndex: number;
  if (markerIndex >= 0) {
    startIndex = markerIndex + 1;
  } else if (serverUnreadCount > 0) {
    startIndex = firstUnreadIndex(messages, marker, serverUnreadCount, userId);
    // The loaded window may be smaller than the server's aggregate count.
    if (startIndex < 0) startIndex = 0;
  } else {
    return messages.filter((message) =>
      Boolean(message.eventId && newUnreadEventIds.has(message.eventId)) &&
      !consumedEventIds.has(message.eventId!) && message.sender !== userId && isUnreadMessage(message),
    ).length;
  }

  return messages.slice(startIndex).filter((message) =>
    Boolean(message.eventId) && message.sender !== userId && isUnreadMessage(message) &&
    !consumedEventIds.has(message.eventId!),
  ).length;
}

export function firstUnreadIndex(messages: MatrixChatMessage[], marker: string | null, count: number, userId: string): number {
  if (count <= 0) return -1;
  const readIndex = marker ? messages.findIndex(m => m.eventId === marker) : -1;
  if (readIndex >= 0) return messages.findIndex((m, i) => i > readIndex && m.sender !== userId && isUnreadMessage(m));
  let remaining = count;
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].sender !== userId && isUnreadMessage(messages[i]) && --remaining <= 0) return i;
  }
  // The server unread count can exceed the currently loaded timeline. In that
  // case the oldest loaded message is not a reliable unread boundary; let the
  // caller open at the latest message instead of jumping to the start of a day.
  return -1;
}
