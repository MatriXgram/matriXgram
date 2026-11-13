<script lang="ts">
  import { matrixGetReplyEvent } from "$lib/chats/api";
  import { replyFallback, replyTextPreview as deriveReplyTextPreview } from "../shared";
  import type { JumpToMessageHandler, TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    roomId: string;
    repliedMessage?: TimelineMessage;
    onJumpToMessage?: JumpToMessageHandler;
  }

  let { message, roomId, repliedMessage, onJumpToMessage }: Props = $props();

  let fetchedMessage = $state<TimelineMessage | null>(null);
  let requestedEventId: string | null = null;
  let fetchedEventId: string | null = null;
  let loadingReply = $state(false);
  let replyFetchError = $state("");
  const fallback = $derived(replyFallback(message));
  const eventId = $derived(message.inReplyToEventId ?? null);
  const resolvedMessage = $derived(
    repliedMessage?.eventId === eventId ? repliedMessage : fetchedMessage?.eventId === eventId ? fetchedMessage : null,
  );
  const replySenderLabel = $derived(
    resolvedMessage?.senderDisplayName ?? resolvedMessage?.sender ?? fallback.sender ?? "message",
  );
  const replyTextPreview = $derived(
    resolvedMessage ? deriveReplyTextPreview(resolvedMessage) : fallback.text,
  );
  const replyFetchErrorLabel = $derived(
    /(?:404|M_NOT_FOUND)/i.test(replyFetchError)
      ? "Message deleted or unavailable"
      : "Could not load quoted message",
  );

  $effect(() => {
    const targetEventId = eventId;
    if (!targetEventId || repliedMessage?.eventId === targetEventId || fetchedEventId === targetEventId || requestedEventId === targetEventId) return;
    requestedEventId = targetEventId;
    loadingReply = true;
    replyFetchError = "";
    let cancelled = false;
    void matrixGetReplyEvent(roomId, targetEventId).then((message) => {
      if (!cancelled) {
        fetchedEventId = message.eventId ?? targetEventId;
        fetchedMessage = message;
      }
    }).catch((error) => {
      if (!cancelled) replyFetchError = error instanceof Error ? error.message : String(error);
    }).finally(() => {
      if (!cancelled) loadingReply = false;
    });
    return () => { cancelled = true; };
  });

  function jumpToReply() {
    if (!message.inReplyToEventId) {
      return;
    }

    onJumpToMessage?.(message.inReplyToEventId);
  }
</script>

{#if message.inReplyToEventId}
  <button
    type="button"
    class="reply-quote mb-1 flex w-full cursor-pointer items-stretch gap-2 pr-2.5 py-1 pl-0 text-left"
    onclick={jumpToReply}
  >
    <span class="reply-quote-bar -my-1 w-[3px] shrink-0 bg-accent"></span>
    {#if resolvedMessage?.messageType === "m.image" && resolvedMessage.imageUrl}
      <img
        src={resolvedMessage.imageUrl}
        alt={resolvedMessage.body || "Reply image"}
        loading="lazy"
        class="size-[34px] shrink-0 self-center rounded object-cover"
      />
    {:else if !resolvedMessage && /^image$/i.test(fallback.text)}
      <span class="grid size-[34px] shrink-0 self-center place-items-center rounded bg-panel-alt text-muted" aria-label="Image preview">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" class="size-5" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><path d="m21 15-5-5L5 21"/></svg>
      </span>
    {/if}
    <span class="min-w-0 flex-1">
      <span class="block truncate text-[12px] font-semibold leading-4 text-accent">{replySenderLabel}</span>
      {#if replyTextPreview}
        <span class="block truncate text-[12px] leading-4 text-inherit">{replyTextPreview}</span>
      {:else if loadingReply}
        <span class="block truncate text-[12px] leading-4 opacity-70" role="status">Loading reply…</span>
      {:else if replyFetchError}
        <span class="block truncate text-[12px] leading-4 opacity-70">{replyFetchErrorLabel}</span>
      {:else}
        <span class="block truncate text-[12px] leading-4 opacity-70">Message unavailable</span>
      {/if}
    </span>
  </button>
{/if}

<style>
  .reply-quote {
    background: var(--tg-reply-surface, color-mix(in srgb, var(--tg-accent) 12%, transparent));
  }
  .reply-quote:hover {
    background: color-mix(in srgb, var(--tg-reply-surface, color-mix(in srgb, var(--tg-accent) 12%, transparent)) 90%, var(--tg-accent));
  }
  .reply-quote-bar {
    align-self: stretch;
  }
</style>
