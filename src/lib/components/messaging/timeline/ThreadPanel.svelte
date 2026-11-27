<script lang="ts">
  import { MessageComposer, type ComposerMention } from "$lib/components/messaging/composer";
  import type { PickerCustomEmoji } from "$lib/emoji/picker";
  import MessageItem from "../message/MessageItem.svelte";
  import { toClockTime } from "../shared";
  import type { TimelineMessage } from "../shared";

  interface Props {
    root: TimelineMessage;
    replies: TimelineMessage[];
    roomId: string;
    currentUserId: string | null;
    pickerCustomEmoji?: PickerCustomEmoji[];
    isSending?: boolean;
    loading: boolean;
    error: string;
    draft: string;
    sending: boolean;
    onClose: () => void;
    onDraftChange: (draft: string) => void;
    onSend: (draft: string, mentions?: ComposerMention[], replyToMessage?: TimelineMessage | null) => void;
    onRetry: () => void;
    onToggleReaction?: (message: TimelineMessage, key: string) => void;
    onForward?: (message: TimelineMessage) => void;
    onDeleteMessage?: (message: TimelineMessage) => void;
    onOpenThread?: (message: TimelineMessage) => void;
    onJumpToMessage?: (eventId: string) => void;
  }

  let {
    root, replies, roomId, currentUserId, pickerCustomEmoji = [], isSending = false, loading, error, draft, sending,
    onClose, onDraftChange, onSend, onRetry, onToggleReaction, onForward, onDeleteMessage, onOpenThread, onJumpToMessage,
  }: Props = $props();

  let replyTarget = $state<TimelineMessage | null>(null);
  const imageMessages = $derived([root, ...replies].filter((message) => message.messageType === "m.image" && message.imageUrl));

  function sendReply(messageDraft: string, mentions?: ComposerMention[]) {
    onSend(messageDraft, mentions, replyTarget);
    replyTarget = null;
  }

</script>

<aside class="thread-panel flex h-full min-h-0 w-full min-w-0 flex-1 flex-col bg-panel" aria-label="Message thread">
  <header class="flex min-h-14 shrink-0 items-center border-b border-hairline px-3 py-2">
    <button type="button" class="grid size-10 shrink-0 place-items-center rounded-lg text-primary transition hover:bg-hover" aria-label="Back to chat" onclick={onClose}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="size-5 shrink-0" aria-hidden="true"><path d="m15 18-6-6 6-6"/></svg>
    </button>
    <div class="ml-2 min-w-0">
      <h2 class="text-base font-semibold text-primary">Discussion</h2>
      <p class="text-xs text-secondary">{Math.max(root.threadReplyCount, replies.length)} {Math.max(root.threadReplyCount, replies.length) === 1 ? "reply" : "replies"}</p>
    </div>
  </header>

  <div class="flex min-h-0 flex-1 flex-col">
    {#if error}
      <div class="m-3 rounded-lg border border-red-500/30 bg-red-500/10 p-3 text-xs text-red-400">
        <p>{error}</p>
        <button type="button" class="mt-2 font-semibold underline" onclick={onRetry}>Try again</button>
      </div>
    {/if}
    <ul class="min-h-0 flex-1 space-y-3 overflow-y-auto px-3 py-4" aria-live="polite">
      <MessageItem
        message={root}
        {imageMessages}
        {roomId}
        {currentUserId}
        {pickerCustomEmoji}
        showAvatar
        showSenderName
        onToggleReaction={onToggleReaction}
        onReplyToMessage={(message) => replyTarget = message}
        {onForward}
        {onDeleteMessage}
        onOpenThread={onOpenThread}
        {onJumpToMessage}
        isSending={isSending || sending}
      />
      <li class="flex items-center gap-3 py-1 text-[11px] text-secondary" aria-label="Discussion started">
        <span class="h-px flex-1 bg-hairline" aria-hidden="true"></span>
        <span>Discussion started{root.timestamp ? ` · ${toClockTime(root.timestamp)}` : ""}</span>
        <span class="h-px flex-1 bg-hairline" aria-hidden="true"></span>
      </li>
      {#if loading && replies.length === 0}
        <li class="py-6 text-center text-xs text-secondary">Loading thread…</li>
      {:else if replies.length === 0 && !error}
        <li class="py-6 text-center text-xs text-secondary">No replies yet. Start the conversation.</li>
      {/if}
      {#each replies as reply (reply.eventId ?? reply.localId)}
        <MessageItem
          message={reply}
          {imageMessages}
          {roomId}
          {currentUserId}
          {pickerCustomEmoji}
          showAvatar
          showSenderName
          repliedMessage={reply.inReplyToEventId === root.eventId ? root : replies.find((candidate) => candidate.eventId === reply.inReplyToEventId)}
          onToggleReaction={onToggleReaction}
          onReplyToMessage={(message) => replyTarget = message}
          {onForward}
          {onDeleteMessage}
          onOpenThread={onOpenThread}
          {onJumpToMessage}
          isSending={isSending || sending}
        />
      {/each}
    </ul>
  </div>

  <div class="composer-dock shrink-0">
      <MessageComposer
        {draft}
        {roomId}
        error=""
        isSending={sending}
        isDisabled={!roomId}
        placeholder="Write a message..."
        {pickerCustomEmoji}
        replyToMessage={replyTarget}
        onClearReply={() => replyTarget = null}
        onDraftChange={onDraftChange}
        onSubmit={sendReply}
      />
  </div>
</aside>
