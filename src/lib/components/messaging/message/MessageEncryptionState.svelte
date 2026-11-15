<script lang="ts">
  import { decryptionLabel, verificationLabel } from "../shared";
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
  }

  let { message }: Props = $props();
</script>

{#if message.encrypted && message.decryptionStatus !== "plaintext"}
  <span class="inline-flex shrink-0 items-center gap-1" aria-label="Encryption details">
    {#if message.decryptionStatus === "decrypted"}
      <span class="inline-flex text-success-600-400" role="img" aria-label={decryptionLabel(message.decryptionStatus)} title={decryptionLabel(message.decryptionStatus)}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="size-3.5" aria-hidden="true"><rect x="4" y="10" width="16" height="11" rx="2"/><path d="M8 10V7a4 4 0 0 1 7.6-1.7M9 15l2 2 4-4"/></svg>
      </span>
    {:else if message.decryptionStatus === "unableToDecrypt"}
      <span class="inline-flex text-warning-600-400" role="img" aria-label={decryptionLabel(message.decryptionStatus)} title={decryptionLabel(message.decryptionStatus)}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="size-3.5" aria-hidden="true"><path d="M12 3 2.8 19h18.4L12 3Z"/><path d="M12 9v4m0 3h.01"/></svg>
      </span>
    {/if}
    {#if message.verificationStatus === "unverified"}
      <span class="inline-flex text-warning-600-400" role="img" aria-label={verificationLabel(message.verificationStatus)} title={verificationLabel(message.verificationStatus)}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="size-3.5" aria-hidden="true"><path d="M12 3 2.8 19h18.4L12 3Z"/><path d="M12 9v4m0 3h.01"/></svg>
      </span>
    {:else if message.verificationStatus === "verified"}
      <span class="inline-flex text-success-600-400" role="img" aria-label={verificationLabel(message.verificationStatus)} title={verificationLabel(message.verificationStatus)}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="size-3.5" aria-hidden="true"><path d="m5 12 4 4L19 6"/></svg>
      </span>
    {/if}
  </span>
{/if}
