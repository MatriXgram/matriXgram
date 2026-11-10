<script lang="ts">
  interface Props {
    isSending: boolean;
    isDisabled: boolean;
    draft: string;
    onChooseAttachment?: () => void;
  }

  let {
    isSending,
    isDisabled,
    draft,
    onChooseAttachment,
  }: Props = $props();
</script>

<div class="mt-1 flex items-center justify-between gap-2">
  <button
    type="button"
    class="btn btn-sm"
    disabled={isDisabled || isSending}
    title="Attach a file"
    aria-label="Attach a file"
    onclick={() => onChooseAttachment?.()}
  >
    📎
  </button>

  <button
    type="submit"
    class="btn btn-sm btn-primary"
    disabled={isDisabled || isSending || draft.trim().length === 0}
    title="Send"
    aria-label="Send message"
  >
    {#if isSending}…{:else}➤{/if}
  </button>
</div>
