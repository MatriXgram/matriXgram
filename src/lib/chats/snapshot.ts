import type { MatrixChatSummary } from "./types";

// A first cache read can finish after live room-added/updated events. Keep the
// live records rather than replacing them with a stale or empty cache snapshot.
export function mergeInitialChatSnapshot(current: MatrixChatSummary[], cached: MatrixChatSummary[]) {
  const rooms = new Map(cached.map((room) => [room.roomId, room]));
  for (const room of current) rooms.set(room.roomId, room);
  return [...rooms.values()];
}
