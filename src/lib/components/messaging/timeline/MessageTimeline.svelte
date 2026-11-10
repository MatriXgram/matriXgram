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
    nextFrom: string | null;
    isSending: boolean;
    /// Show the sender name above incoming bubbles (group chats).
    showSenderNames?: boolean;
    onTimelineElementChange?: (element: HTMLElement | null) => void;
    onProgrammaticScroll?: () => void;
    onScroll?: (event: Event) => void;
    onLoadOlder?: () => void;
    onOpenRoomSettings?: (roomId: string) => void;
    onRetryMessage?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    onReplyToMessage?: ReplyToMessageHandler;
    onForwardMessage?: (message: TimelineMessage) => void;
  }

  let { 
    messages, 
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
    nextFrom,
    isSending,
    showSenderNames = false,
    onTimelineElementChange,
    onProgrammaticScroll,
    onScroll,
    onLoadOlder,
    onOpenRoomSettings,
    onRetryMessage,
    onToggleReaction,
    onReplyToMessage,
    onForwardMessage,
  }: Props = $props();

</script>

<section class="flex min-h-0 min-w-0 flex-grow flex-col bg-panel-alt">
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
      {nextFrom}
      roomId={roomId}
      onOpenSettings={onOpenRoomSettings}
      onLoadOlder={onLoadOlder}
    />

    <TimelineContent
      {messages}
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
    />
  {/if}
</section>
