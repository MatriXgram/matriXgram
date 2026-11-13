<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";
  import RoomListItem from "./RoomListItem.svelte";
  import ExploreCommunities from "./ExploreCommunities.svelte";

  interface Props {
    rooms: MatrixChatSummary[];
    loading?: boolean;
    error?: string;
    onRetry?: () => void;
    compact?: boolean;
    selectedRoomId: string | null;
    onSelectRoom?: (roomId: string) => void;
  }

  let { rooms, selectedRoomId, onSelectRoom, loading = false, error = "", onRetry, compact = false }: Props = $props();

  let searchQuery = $state("");

  const visibleRooms = $derived.by(() => {
    const query = searchQuery.trim().toLowerCase();

    const filtered = rooms
      .filter((room) => room.kind === "room")
      .filter((room) => {
        if (!query) {
          return true;
        }

        return (
          room.displayName.toLowerCase().includes(query) ||
          (room.lastMessagePreview ?? "").toLowerCase().includes(query)
        );
      });

    // Pinned chats first, then the most recent conversation, then the name.
    return filtered.sort((left, right) => {
      if (left.isPinned !== right.isPinned) {
        return left.isPinned ? -1 : 1;
      }

      const leftTimestamp = left.lastMessageTs ?? 0;
      const rightTimestamp = right.lastMessageTs ?? 0;
      if (leftTimestamp !== rightTimestamp) {
        return rightTimestamp - leftTimestamp;
      }

      return left.displayName.localeCompare(right.displayName);
    });
  });

  const directRooms = $derived(visibleRooms.filter((room) => room.isDirect));
  const groupRooms = $derived(visibleRooms.filter((room) => !room.isDirect));
</script>

<aside class="flex min-h-0 min-w-0 flex-1 flex-col bg-panel">
  {#if compact}
    <div class="flex min-h-0 flex-1 flex-col items-center overflow-y-auto py-2">
      <ul class="w-full">
        {#each visibleRooms as room (room.roomId)}
          <RoomListItem
            {room}
            depth={0}
            hasChildren={false}
            isExpanded={false}
            isSelected={false}
            onSelect={onSelectRoom}
            compact
          />
        {/each}
      </ul>
      <div class="my-1 h-px w-8 shrink-0 bg-hairline"></div>
      <ExploreCommunities compact />
    </div>
  {:else}
  <div class="chat-search">
    <label class="relative block">
      <span class="sr-only">Search chats</span>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" class="pointer-events-none absolute left-3 top-[9px] size-[18px] text-muted" aria-hidden="true"><circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 4 4"/></svg>
      <input
        type="search"
        bind:value={searchQuery}
        placeholder="Search conversations"
        class="w-full rounded-lg border border-hairline bg-panel-alt px-3 py-2 text-sm outline-none placeholder:text-muted focus:border-accent"
      />
    </label>
  </div>

  <div class="chat-list-scroll min-h-0 flex-1 overflow-y-auto">
    {#if visibleRooms.length === 0}
      <div class="px-4 py-6 text-center text-sm text-secondary">
        {#if searchQuery}<p>No chats match your search.</p>
        {:else if loading}<p role="status">Syncing conversations…</p>
        {:else if error}<p>Couldn't load conversations.</p>
        {:else}<p>No chats yet.</p>{/if}
        {#if !searchQuery && onRetry}
          <button type="button" class="btn btn-sm mt-3" onclick={onRetry} disabled={loading}>Retry sync</button>
        {/if}
      </div>
    {:else}
      {#if directRooms.length > 0}
        <p class="chat-section-label font-semibold uppercase text-muted">
          Direct messages
        </p>
        <ul>
          {#each directRooms as room (room.roomId)}
            <RoomListItem
              {room}
              depth={0}
              hasChildren={false}
              isExpanded={false}
              isSelected={room.roomId === selectedRoomId}
              onSelect={onSelectRoom}
            />
          {/each}
        </ul>
      {/if}

      {#if groupRooms.length > 0}
        <p class="chat-section-label font-semibold uppercase text-muted">
          Groups
        </p>
        <ul>
          {#each groupRooms as room (room.roomId)}
            <RoomListItem
              {room}
              depth={0}
              hasChildren={false}
              isExpanded={false}
              isSelected={room.roomId === selectedRoomId}
              onSelect={onSelectRoom}
            />
          {/each}
        </ul>
      {/if}
    {/if}
    <ExploreCommunities />
  </div>
  {/if}
</aside>
