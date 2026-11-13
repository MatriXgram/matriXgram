<script lang="ts">
  import { matrixGetRoomImage } from "$lib/chats/api";
  import { RoomAvatar, roomImageCache } from "$lib/components/navigation/shared";
  import { streamStatusLabel } from "../shared";

  interface Props {
    roomName: string;
    roomEncrypted: boolean;
    loadingMessages: boolean;
    activeLoadKind: string | null;
    streamMessageCount: number;
    roomId?: string | null;
    onOpenSettings?: (roomId: string) => void;
  }

  let {
    roomName,
    roomEncrypted,
    loadingMessages,
    activeLoadKind,
    streamMessageCount,
    roomId,
    onOpenSettings,
  }: Props = $props();
  let roomImage = $state<string | null>(null);
  $effect(() => {
    const id = roomId;
    roomImage = id ? roomImageCache.getCached(id) ?? null : null;
    if (!id) return;
    let cancelled = false;
    void roomImageCache.getOrLoad(id, () => matrixGetRoomImage(id)).then((image) => {
      if (!cancelled) roomImage = image;
    });
    return () => { cancelled = true; };
  });
</script>

<header class="conversation-header flex items-center justify-between border-b border-hairline bg-panel">
  <div class="flex min-w-0 items-center gap-3">
    <RoomAvatar imageUrl={roomImage} displayName={roomName} />
    <div class="min-w-0">
    <h2 class="conversation-title truncate leading-tight">{roomName}</h2>
    <p class="truncate text-[11px] text-secondary">
      <span class="inline-flex items-center gap-1">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="size-3" aria-hidden="true"><rect x="5" y="10" width="14" height="11" rx="3"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></svg>
        {roomEncrypted ? "End-to-end encrypted" : "Not encrypted"}
      </span>
      {#if loadingMessages && activeLoadKind}
        <span class="text-accent">· {streamStatusLabel(loadingMessages, activeLoadKind, streamMessageCount)}</span>
      {/if}
    </p>
    </div>
  </div>

  <div class="flex shrink-0 items-center gap-2">
    {#if roomId && onOpenSettings}
      <button
        type="button"
        class="icon-button"
        title="Room settings"
        aria-label="Open room settings"
        onclick={() => onOpenSettings?.(roomId)}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" class="size-5" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 11v6m0-10v.5"/></svg>
      </button>
    {/if}


  </div>
</header>
