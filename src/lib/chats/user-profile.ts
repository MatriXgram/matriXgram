import { writable } from "svelte/store";
import { matrixGetUserProfile } from "./api";
import type { MatrixUserProfileResponse } from "./types";

export type ProfileTarget = { roomId: string; userId: string; displayName?: string | null; avatarUrl?: string | null };
export const profileTarget = writable<ProfileTarget | null>(null);
export function openUserProfile(target: ProfileTarget) { profileTarget.set(target); }

// Deduplicate hover/profile requests without permanently caching stale profiles.
const cache = new Map<string, { value: MatrixUserProfileResponse; expires: number }>();
const requests = new Map<string, Promise<MatrixUserProfileResponse>>();
export function loadUserProfile(roomId: string, userId: string) {
  const key = `${roomId}\0${userId}`;
  const cached = cache.get(key);
  if (cached && cached.expires > Date.now()) return Promise.resolve(cached.value);
  const existing = requests.get(key);
  if (existing) return existing;
  const request = matrixGetUserProfile(roomId, userId).then((value) => {
    cache.set(key, { value, expires: Date.now() + 60000 });
    return value;
  }).finally(() => requests.delete(key));
  requests.set(key, request);
  return request;
}
