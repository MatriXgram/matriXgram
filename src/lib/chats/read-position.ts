import type { MatrixChatMessage } from "./types";
// A real receipt wins over a count estimate (state/reaction events also affect
// server counts). Keep the divider fixed for the lifetime of an open room.
export function firstUnreadIndex(messages: MatrixChatMessage[], marker: string | null, count: number, userId: string): number {
  if (count <= 0) return -1;
  const readIndex = marker ? messages.findIndex(m => m.eventId === marker) : -1;
  if (readIndex >= 0) return messages.findIndex((m, i) => i > readIndex && m.sender !== userId);
  let remaining = count;
  for (let i = messages.length - 1; i >= 0; i--) {
    if (messages[i].sender !== userId && --remaining <= 0) return i;
  }
  return messages.findIndex(m => m.sender !== userId);
}
