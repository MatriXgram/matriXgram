<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";
  import RoomListItem from "./RoomListItem.svelte";

  interface Props {
    rooms: MatrixChatSummary[];
    selectedRoomId: string | null;
    onSelectRoom?: (roomId: string) => void;
  }

  let { rooms, selectedRoomId, onSelectRoom }: Props = $props();

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
  <div class="border-b border-hairline p-2">
    <label class="relative block">
      <span class="sr-only">Search chats</span>
      <input
        type="search"
        bind:value={searchQuery}
        placeholder="Search"
        class="w-full rounded-lg border border-hairline bg-panel-alt px-3 py-2 text-sm outline-none placeholder:text-muted focus:border-accent"
      />
    </label>
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto py-1">
    {#if visibleRooms.length === 0}
      <p class="px-4 py-6 text-center text-sm text-secondary">
        {searchQuery ? "No chats match your search." : "No chats yet."}
      </p>
    {:else}
      {#if directRooms.length > 0}
        <p class="px-4 pb-1 pt-2 text-[11px] font-semibold uppercase tracking-wide text-muted">
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
        <p class="px-4 pb-1 pt-3 text-[11px] font-semibold uppercase tracking-wide text-muted">
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
  </div>
</aside>
