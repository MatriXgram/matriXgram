<script lang="ts">
  import { streamStatusLabel } from "../shared";

  interface Props {
    roomName: string;
    roomEncrypted: boolean;
    loadingMessages: boolean;
    activeLoadKind: string | null;
    streamMessageCount: number;
    nextFrom: string | null;
    onLoadOlder?: () => void;
    roomId?: string | null;
    onOpenSettings?: (roomId: string) => void;
  }

  let {
    roomName,
    roomEncrypted,
    loadingMessages,
    activeLoadKind,
    streamMessageCount,
    nextFrom,
    onLoadOlder,
    roomId,
    onOpenSettings,
  }: Props = $props();
</script>

<header class="flex items-center justify-between gap-2 border-b border-hairline bg-panel px-4 py-2.5">
  <div class="min-w-0">
    <h2 class="truncate text-[15px] font-semibold leading-tight">{roomName}</h2>
    <p class="truncate text-[11px] text-secondary">
      {roomEncrypted ? "🔒 Encrypted" : "Not encrypted"}
      {#if loadingMessages && activeLoadKind}
        <span class="text-accent">· {streamStatusLabel(loadingMessages, activeLoadKind, streamMessageCount)}</span>
      {/if}
    </p>
  </div>

  <div class="flex shrink-0 items-center gap-2">
    {#if roomId && onOpenSettings}
      <button
        type="button"
        class="btn btn-sm"
        title="Room settings"
        aria-label="Open room settings"
        onclick={() => onOpenSettings?.(roomId)}
      >
        ⚙
      </button>
    {/if}

    <button
      type="button"
      class="btn btn-sm"
      onclick={onLoadOlder}
      disabled={!nextFrom || loadingMessages}
    >
      {#if loadingMessages}Loading...{:else}Load older{/if}
    </button>
  </div>
</header>
