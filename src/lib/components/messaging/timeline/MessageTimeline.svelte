<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";
  import type {
    ReplyToMessageHandler,
    RetryMessageHandler,
    TimelineMessage,
    ToggleReactionHandler,
  } from "../shared";
  import {
    TimelineContent,
    TimelineEmptySelection,
    TimelineHeader,
  } from ".";
  import { ErrorBanner } from "../shared";

  interface Props {
    messages: TimelineMessage[];
    unreadBoundaryId?: string | null;
    unreadCount?: number;
    showJumpToLatest?: boolean;
    onJumpToLatest?: () => void;
    roomId: string;
    selectedRoomId: string | null;
    currentUserId: string | null;
    pickerCustomEmoji: PickerCustomEmoji[];
    roomEncrypted: boolean;
    roomName: string;
    loadingMessages: boolean;
    activeLoadKind: string | null;
    streamMessageCount: number;
    error: string;
    isSending: boolean;
    /// Show the sender name above incoming bubbles (group chats).
    showSenderNames?: boolean;
    onTimelineElementChange?: (element: HTMLElement | null) => void;
    onProgrammaticScroll?: () => void;
    onScroll?: (event: Event) => void;
    onOpenRoomSettings?: (roomId: string) => void;
    onRetryMessage?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    onReplyToMessage?: ReplyToMessageHandler;
    onForwardMessage?: (message: TimelineMessage) => void;
    onLoadJumpContext?: (eventId: string) => Promise<boolean>;
  }

  let { 
    messages, 
    unreadBoundaryId = null, unreadCount = 0, showJumpToLatest = false, onJumpToLatest,
    roomId, 
    selectedRoomId, 
    currentUserId,
    pickerCustomEmoji,
    roomEncrypted, 
    roomName, 
    loadingMessages, 
    activeLoadKind, 
    streamMessageCount, 
    error,
    isSending,
    showSenderNames = false,
    onTimelineElementChange,
    onProgrammaticScroll,
    onScroll,
    onOpenRoomSettings,
    onRetryMessage,
    onToggleReaction,
    onReplyToMessage,
    onForwardMessage,
    onLoadJumpContext,
  }: Props = $props();

</script>

<section class="relative flex min-h-0 min-w-0 flex-grow flex-col bg-panel-alt">
  <ErrorBanner {error} />

  {#if !selectedRoomId}
    <TimelineEmptySelection />
  {:else}
    <TimelineHeader
      {roomName}
      {roomEncrypted}
      {loadingMessages}
      {activeLoadKind}
      {streamMessageCount}
      roomId={roomId}
      onOpenSettings={onOpenRoomSettings}
    />

    <TimelineContent
      {messages}
      {unreadBoundaryId}
      {roomId}
      {currentUserId}
      {pickerCustomEmoji}
      {isSending}
      {showSenderNames}
      {onScroll}
      {onProgrammaticScroll}
      {onRetryMessage}
      {onToggleReaction}
      {onReplyToMessage}
      onForward={onForwardMessage}
      {onTimelineElementChange}
      {onLoadJumpContext}
    />
  {/if}
  {#if selectedRoomId && showJumpToLatest}
    <button type="button" class="absolute bottom-4 right-5 z-20 grid size-11 cursor-pointer place-items-center rounded-full border border-hairline bg-panel text-accent shadow-lg transition hover:bg-accent-soft hover:shadow-xl focus-visible:ring-2 focus-visible:ring-accent"
      aria-label={unreadCount > 0 ? `Jump to latest • ${unreadCount} unread messages` : "Jump to latest"} onclick={onJumpToLatest}>
      {#if unreadCount > 0}<span class="absolute -top-2 min-w-5 rounded-full bg-accent px-1.5 py-0.5 text-[11px] font-semibold text-white tabular-nums">{unreadCount > 999 ? "999+" : unreadCount}</span>{/if}
      <svg viewBox="0 0 24 24" class="size-5" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
    </button>
  {/if}
</section>
