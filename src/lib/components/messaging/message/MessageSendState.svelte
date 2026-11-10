<script lang="ts">
  import type { RetryMessageHandler, TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    isSending?: boolean;
    onRetry?: RetryMessageHandler;
  }

  let {
    message,
    isSending = false,
    onRetry,
  }: Props = $props();
</script>

{#if message.sendState === "failed"}
  <div class="mt-1 flex items-center gap-2 text-[11px] text-danger">
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      class="size-3.5"
      aria-hidden="true"
    >
      <circle cx="12" cy="12" r="10" />
      <line x1="12" y1="8" x2="12" y2="12" />
      <line x1="12" y1="16" x2="12.01" y2="16" />
    </svg>
    <span>Failed to send</span>
    <button
      type="button"
      class="btn btn-xs"
      onclick={() => onRetry?.(message)}
      disabled={isSending}
    >
      Retry
    </button>
  </div>
{/if}
