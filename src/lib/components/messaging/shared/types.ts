import type { MatrixChatMessage } from "$lib/chats/types";

export interface TimelineMessage extends MatrixChatMessage {
  localId?: string;
  sendState?: "sending" | "failed";
  /// Set by the timeline when the message has been read by someone else.
  isRead?: boolean;
}

export type RetryMessageHandler = (message: TimelineMessage) => void;

export type ToggleReactionHandler = (message: TimelineMessage, key: string) => void;

export type JumpToMessageHandler = (eventId: string) => void;

export type ReplyToMessageHandler = (message: TimelineMessage) => void;
