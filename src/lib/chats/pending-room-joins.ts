export const PENDING_ROOM_JOINS_KEY = "matrixgram.pending-room-joins";

export interface PendingRoomJoin {
  roomId: string;
  name: string;
  alias: string;
  server: string;
  since: number;
}

function isPendingRoomJoin(item: unknown): item is PendingRoomJoin {
  if (item === null || typeof item !== "object") return false;
  const candidate = item as Record<string, unknown>;
  return typeof candidate.roomId === "string" &&
    typeof candidate.name === "string" &&
    typeof candidate.alias === "string" &&
    typeof candidate.server === "string" &&
    typeof candidate.since === "number";
}

export function loadPendingRoomJoins(): PendingRoomJoin[] {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(PENDING_ROOM_JOINS_KEY) ?? "[]");
    if (!Array.isArray(saved)) return [];
    return saved.filter(isPendingRoomJoin);
  } catch {
    return [];
  }
}

export function savePendingRoomJoins(rooms: PendingRoomJoin[]) {
  try {
    localStorage.setItem(PENDING_ROOM_JOINS_KEY, JSON.stringify(rooms));
  } catch {
    // Keep the join flow usable when browser storage is unavailable.
  }
}

export function addPendingRoomJoin(room: PendingRoomJoin): PendingRoomJoin[] {
  const rooms = [
    ...loadPendingRoomJoins().filter((item) => item.roomId !== room.roomId),
    room,
  ];
  savePendingRoomJoins(rooms);
  return rooms;
}

export function removePendingRoomJoin(roomId: string): PendingRoomJoin[] {
  const rooms = loadPendingRoomJoins().filter((item) => item.roomId !== roomId);
  savePendingRoomJoins(rooms);
  return rooms;
}
