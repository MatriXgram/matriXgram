import { matrixGetRoomMembers } from "./api";
import type { MatrixRoomMember } from "./types";

const roomMembersRequests = new Map<string, Promise<MatrixRoomMember[]>>();

export function getRoomMembersCached(roomId: string): Promise<MatrixRoomMember[]> {
  const existing = roomMembersRequests.get(roomId);
  if (existing) return existing;

  const request = matrixGetRoomMembers(roomId).catch((error) => {
    roomMembersRequests.delete(roomId);
    throw error;
  });
  roomMembersRequests.set(roomId, request);
  return request;
}

export async function getDirectRoomPeer(roomId: string, currentUserId: string | null | undefined) {
  if (!currentUserId) return null;
  const members = await getRoomMembersCached(roomId);
  return members.find((member) => member.userId !== currentUserId) ?? null;
}
