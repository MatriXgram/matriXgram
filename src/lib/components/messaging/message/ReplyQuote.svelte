<script lang="ts">
  import { replyTextPreview as deriveReplyTextPreview } from "../shared";
  import type { JumpToMessageHandler, TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    repliedMessage?: TimelineMessage;
    onJumpToMessage?: JumpToMessageHandler;
  }

  let { message, repliedMessage, onJumpToMessage }: Props = $props();

  const replySenderLabel = $derived(repliedMessage?.senderDisplayName ?? repliedMessage?.sender ?? "message");
  const replyTextPreview = $derived(deriveReplyTextPreview(repliedMessage));

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
    class="reply-quote mb-1 flex w-full items-stretch gap-2 text-left"
    onclick={jumpToReply}
  >
    <span class="reply-quote-bar w-0.5 shrink-0 rounded-full bg-accent"></span>
    <span class="min-w-0 flex-1">
      <span class="block truncate text-[12px] font-semibold text-accent">{replySenderLabel}</span>
      {#if repliedMessage?.messageType === "m.image" && repliedMessage.imageUrl}
        <span class="mt-0.5 flex items-center gap-2">
          <img
            src={repliedMessage.imageUrl}
            alt={repliedMessage.body || "Reply image"}
            loading="lazy"
            class="h-8 w-8 rounded border border-hairline object-cover"
          />
          <span class="truncate text-[12px] opacity-80">Image</span>
        </span>
      {:else if replyTextPreview}
        <span class="block truncate text-[12px] opacity-80">{replyTextPreview}</span>
      {:else}
        <span class="block truncate text-[12px] opacity-70">Message unavailable</span>
      {/if}
    </span>
  </button>
{/if}

<style>
  .reply-quote-bar {
    align-self: stretch;
  }
</style>
