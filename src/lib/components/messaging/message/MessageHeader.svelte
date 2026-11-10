<script lang="ts">
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    /// Whether the sender name is shown above the bubble. Telegram hides it
    /// for direct chats and for grouped messages of the same sender.
    show: boolean;
  }

  let { message, show }: Props = $props();

  const senderLabel = $derived(message.senderDisplayName ?? message.sender);
</script>

{#if show}
  <span class="mb-0.5 ml-3 text-[13px] font-semibold leading-tight text-accent">
    {senderLabel}
  </span>
{/if}
