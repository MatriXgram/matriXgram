import { writable } from "svelte/store";

export type ReactionNotice = {
  userId: string;
  roomId: string;
  eventId: string;
  targetEventId: string;
  sender: string;
  senderName: string;
  key: string;
  timestamp: number;
};

export const reactionNotices = writable<ReactionNotice[]>([]);
let activeUser = "";
let cutoff = 0;
let startedAt = 0;
let seen = new Set<string>();
let pending: ReactionNotice[] = [];

function storageKey(userId: string) { return `matrixgram.reactions.${userId}`; }

function persist() {
  if (!activeUser) return;
  try {
    localStorage.setItem(storageKey(activeUser), JSON.stringify({
      lastStarted: startedAt, seen: [...seen].slice(-5000), pending: pending.slice(-1000),
    }));
  } catch { /* Notifications continue to work in this session without storage. */ }
}

export function initializeReactionNotices(userId: string) {
  if (!userId || activeUser === userId) return;
  activeUser = userId;
  startedAt = Date.now();
  cutoff = startedAt;
  seen = new Set();
  pending = [];
  try {
    const saved = JSON.parse(localStorage.getItem(storageKey(userId)) ?? "null");
    if (saved && typeof saved.lastStarted === "number") cutoff = saved.lastStarted;
    if (Array.isArray(saved?.seen)) seen = new Set(saved.seen.filter((id: unknown): id is string => typeof id === "string"));
    if (Array.isArray(saved?.pending)) pending = saved.pending.filter((notice: ReactionNotice) =>
      notice && notice.userId === userId && typeof notice.eventId === "string" && typeof notice.roomId === "string" &&
      typeof notice.targetEventId === "string" && typeof notice.timestamp === "number" &&
      typeof notice.sender === "string" && typeof notice.senderName === "string" && typeof notice.key === "string");
  } catch { /* Ignore invalid stored data. */ }
  reactionNotices.set(pending);
  persist();
}

export function recordReactionNotice(notice: ReactionNotice) {
  if (notice.userId !== activeUser || !notice.eventId || notice.sender === activeUser || seen.has(notice.eventId)) return;
  seen.add(notice.eventId);
  if (notice.timestamp >= cutoff) {
    pending = [...pending, notice].sort((a, b) => a.timestamp - b.timestamp).slice(-1000);
    reactionNotices.set(pending);
  }
  persist();
}

export function removeReactionNotice(userId: string, eventId: string) {
  if (userId !== activeUser) return;
  // Remember redactions that arrive before the target lookup completes.
  seen.add(eventId);
  pending = pending.filter((notice) => notice.eventId !== eventId);
  reactionNotices.set(pending);
  persist();
}

export function consumeReactionTarget(roomId: string, targetEventId: string) {
  pending = pending.filter((notice) => notice.roomId !== roomId || notice.targetEventId !== targetEventId);
  reactionNotices.set(pending);
  persist();
}

export function clearReactionNotices() {
  activeUser = "";
  seen = new Set();
  pending = [];
  reactionNotices.set([]);
}
