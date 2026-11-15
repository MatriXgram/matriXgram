import type { MatrixChatMessage } from "./types";
// A real receipt wins over a count estimate (state/reaction events also affect
// server counts). Keep the divider fixed for the lifetime of an open room.
export function isUnreadMessage(message: MatrixChatMessage): boolean {
  // Membership changes are rendered as timeline rows, but Matrix unread
  // counts and read receipts are about room messages, not join/leave notices.
  return message.messageType !== "m.room.member";
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
