import { get, writable } from "svelte/store";

import { goto } from "$app/navigation";
import { roomImageCache, senderAvatarCache } from "$lib/components/navigation/shared/image-cache.svelte";
import { matrixLogout, matrixRecoveryStatus, matrixSessionStatus } from "../auth/api";
import { clearReactionNotices } from "./reaction-notifications";
import { clearIncomingVerificationRequests } from "$lib/verification/incoming-requests";
import type { MatrixRecoveryState } from "../auth/types";
import type {
	MatrixChatSummary,
	MatrixPickerCustomEmoji,
} from "./types";

export const shellChatsLoading = writable(true);
export const shellSigningOut = writable(false);
export const shellRefreshing = writable(false);
export const shellErrorMessage = writable("");
export const shellCurrentUserId = writable("");
export const shellRecoveryState = writable<MatrixRecoveryState | null>(null);

export const shellChats = writable<MatrixChatSummary[]>([]);
export const shellRootSpaces = writable<MatrixChatSummary[]>([]);
export const shellRootScopedRooms = writable<MatrixChatSummary[]>([]);
export const shellSelectedRootSpaceId = writable("");
export const shellSelectedRoomId = writable("");
export const shellPickerCustomEmoji = writable<MatrixPickerCustomEmoji[]>([]);

/// Reset all shell state to its signed-out default.
export function resetShellState() {
	shellChatsLoading.set(true);
	shellRefreshing.set(false);
	shellErrorMessage.set("");
	roomImageCache.clear();
	senderAvatarCache.clear();
	shellChats.set([]);
	shellRootSpaces.set([]);
	shellRootScopedRooms.set([]);
	shellSelectedRootSpaceId.set("");
	shellSelectedRoomId.set("");
	shellCurrentUserId.set("");
	shellRecoveryState.set(null);
	shellPickerCustomEmoji.set([]);
 clearReactionNotices();
 clearIncomingVerificationRequests();
}

/// Refresh the recovery status into `shellRecoveryState`, clearing it on error.
export async function refreshRecoveryState() {
	try {
		const recovery = await matrixRecoveryStatus();
		shellRecoveryState.set(recovery.state);
	} catch {
		shellRecoveryState.set(null);
	}
}

/// Sign out: clear the server session, reset shell state, and navigate to the
/// signed-out landing page. Returns the error message on failure, or null.
export async function logout(): Promise<string | null> {
  if (get(shellSigningOut)) return "Sign-out is already in progress";
  shellSigningOut.set(true);
  try {
    try {
      // Preserve the actual server before auth is cleared, not just matrix.org.
      try { await matrixSessionStatus(); } catch {}
      await matrixLogout();
    }
    catch (error) {
      // Cleanup errors must not strand an already signed-out user in chats.
      const status = await matrixSessionStatus();
      if (status.authenticated) throw error;
    }
    resetShellState();
    await goto("/", { replaceState: true });
    return null;
  } catch (error) {
    return error instanceof Error ? error.message : "Failed to log out";
  } finally { shellSigningOut.set(false); }
}
