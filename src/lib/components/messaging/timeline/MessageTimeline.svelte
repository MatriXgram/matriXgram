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
    showAttentionIndicator?: boolean;
    attentionCount?: number;
    attentionJumpRequest?: { eventId: string; nonce: number } | null;
    showJumpToLatest?: boolean;
    onJumpToLatest?: () => void;
    onJumpToAttention?: () => void;
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
    onRemoveFailedMessage?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    onReplyToMessage?: ReplyToMessageHandler;
    onForwardMessage?: (message: TimelineMessage) => void;
    onDeleteMessage?: (message: TimelineMessage) => void;
    onLoadJumpContext?: (eventId: string) => Promise<boolean>;
  }

  let { 
    messages, 
    unreadBoundaryId = null, unreadCount = 0, showJumpToLatest = false, showAttentionIndicator = false, attentionCount = 0, attentionJumpRequest = null, onJumpToLatest, onJumpToAttention,
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
    onRemoveFailedMessage,
    onToggleReaction,
    onReplyToMessage,
    onForwardMessage,
    onDeleteMessage,
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
      {onRemoveFailedMessage}
      {onToggleReaction}
      {onReplyToMessage}
      onForward={onForwardMessage}
      {onDeleteMessage}
      {onTimelineElementChange}
      {onLoadJumpContext}
      {attentionJumpRequest}
    />
  {/if}
  {#if selectedRoomId && showJumpToLatest}
    <button type="button" class="absolute bottom-4 right-5 z-20 grid size-11 cursor-pointer place-items-center rounded-full border border-hairline bg-panel text-accent shadow-lg transition hover:bg-accent-soft hover:shadow-xl focus-visible:ring-2 focus-visible:ring-accent"
      aria-label={unreadCount > 0 ? `Jump to latest • ${unreadCount} unread messages` : "Jump to latest"} onclick={onJumpToLatest}>
      {#if unreadCount > 0}<span class="absolute -top-2 min-w-5 rounded-full bg-accent px-1.5 py-0.5 text-[11px] font-semibold text-white tabular-nums">{unreadCount > 999 ? "999+" : unreadCount}</span>{/if}
      <svg viewBox="0 0 24 24" class="size-5" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
    </button>
  {/if}
  {#if selectedRoomId && showAttentionIndicator}
    <button type="button" class="absolute bottom-[4.25rem] right-5 z-20 grid size-11 cursor-pointer place-items-center rounded-full border border-sky-400/60 bg-panel text-sky-400 shadow-lg transition hover:bg-sky-400/10 hover:shadow-xl focus-visible:ring-2 focus-visible:ring-sky-400"
      aria-label={`${attentionCount} unread mentions or replies to you`} title={`${attentionCount} unread mentions or replies to you`} onclick={onJumpToAttention}>
      <span class="text-lg font-semibold leading-none" aria-hidden="true">@</span>
      <span class="absolute -top-2 -right-2 min-w-5 rounded-full bg-sky-500 px-1.5 py-0.5 text-center text-[11px] font-semibold leading-4 text-white tabular-nums">{attentionCount > 999 ? "999+" : attentionCount}</span>
    </button>
  {/if}
</section>
