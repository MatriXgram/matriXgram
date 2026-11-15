import { listen } from "@tauri-apps/api/event";

import {
  EVENT_CHAT_MESSAGES_STREAM,
  EVENT_ROOM_ADDED,
  EVENT_ROOM_SYNC_STATUS,
  EVENT_ROOM_REMOVED,
  EVENT_ROOM_UPDATED,
  EVENT_PROFILE_AVATAR_UPDATED,
  EVENT_SELECTED_ROOM_MESSAGES,
  EVENT_VERIFICATION_STATE_CHANGED,
  EVENT_REACTION_RECEIVED,
  EVENT_REACTION_REDACTED,
} from "$lib/events";import {
  normalizeChatSummaryImageUrl,
  normalizeChatMessageStreamEvent,
  normalizeSelectedRoomMessagesEvent,
} from "./media";
import type {
  MatrixChatSummary,
  MatrixRoomSyncStatusEvent,
  MatrixChatMessageStreamEvent,
  MatrixProfileAvatarUpdatedEvent,
  MatrixRoomRemovedEvent,
  MatrixSelectedRoomMessagesEvent,
  MatrixVerificationStateChangedEvent,
} from "./types";
import type { ReactionNotice } from "./reaction-notifications";

export interface RoomUpdateHandlers {
  onRoomAdded: (room: MatrixChatSummary) => void;
  onRoomUpdated: (room: MatrixChatSummary) => void;
  onRoomRemoved: (payload: MatrixRoomRemovedEvent) => void;
  onSelectedRoomMessages: (payload: MatrixSelectedRoomMessagesEvent) => void;
  onChatMessagesStream: (payload: MatrixChatMessageStreamEvent) => void;
  onVerificationStateChanged?: (payload: MatrixVerificationStateChangedEvent) => void;
  onRoomSyncStatus?: (payload: MatrixRoomSyncStatusEvent) => void;
  onProfileAvatarUpdated?: (payload: MatrixProfileAvatarUpdatedEvent) => void;
  onReactionReceived?: (payload: ReactionNotice) => void;
  onReactionRedacted?: (payload: { userId: string; roomId: string; eventId: string }) => void;
}

export async function subscribeToRoomUpdates(handlers: RoomUpdateHandlers): Promise<() => void> {
  const unlisteners = await Promise.all([
    listen<MatrixRoomSyncStatusEvent>(EVENT_ROOM_SYNC_STATUS, (event) => handlers.onRoomSyncStatus?.(event.payload)),
    listen<ReactionNotice>(EVENT_REACTION_RECEIVED, (event) => handlers.onReactionReceived?.(event.payload)),
    listen<{ userId: string; roomId: string; eventId: string }>(EVENT_REACTION_REDACTED, (event) => handlers.onReactionRedacted?.(event.payload)),
    listen<MatrixChatSummary>(EVENT_ROOM_ADDED, (event) =>
      handlers.onRoomAdded(normalizeChatSummaryImageUrl(event.payload)),
    ),
    listen<MatrixChatSummary>(EVENT_ROOM_UPDATED, (event) =>
      handlers.onRoomUpdated(normalizeChatSummaryImageUrl(event.payload)),
    ),
    listen<MatrixRoomRemovedEvent>(EVENT_ROOM_REMOVED, (event) => handlers.onRoomRemoved(event.payload)),
    listen<MatrixSelectedRoomMessagesEvent>(EVENT_SELECTED_ROOM_MESSAGES, (event) =>
      handlers.onSelectedRoomMessages(normalizeSelectedRoomMessagesEvent(event.payload)),
    ),
    listen<MatrixChatMessageStreamEvent>(EVENT_CHAT_MESSAGES_STREAM, (event) =>
      handlers.onChatMessagesStream(normalizeChatMessageStreamEvent(event.payload)),
    ),
    listen<MatrixVerificationStateChangedEvent>(EVENT_VERIFICATION_STATE_CHANGED, (event) => {
      handlers.onVerificationStateChanged?.(event.payload);
    }),
    listen<MatrixProfileAvatarUpdatedEvent>(EVENT_PROFILE_AVATAR_UPDATED, (event) => {
      handlers.onProfileAvatarUpdated?.(event.payload);
    }),
  ]);

  return () => {
    for (const unlisten of unlisteners) {
      unlisten();
    }
  };
}
