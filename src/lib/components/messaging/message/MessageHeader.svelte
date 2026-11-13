<script lang="ts">
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    /// Whether the sender name is shown. Telegram hides it for direct chats
    /// and for grouped messages of the same sender.
    show: boolean;
    onOpenProfile?: () => void;
  }

  let { message, show, onOpenProfile }: Props = $props();

  const senderLabel = $derived(message.senderDisplayName?.trim() || message.sender.replace(/^@/, "").split(":")[0]);
</script>

{#if show}
  <button type="button" onclick={onOpenProfile} title={message.sender} class="cursor-pointer text-left hover:underline focus-visible:underline mb-0.5 block max-w-full truncate text-[14px] font-semibold leading-tight text-accent">
    {senderLabel}
  </button>
{/if}
