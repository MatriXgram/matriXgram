<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";
  import RoomListItem from "./RoomListItem.svelte";
  import ExploreCommunities from "./ExploreCommunities.svelte";
  import { matrixOpenDirectChat } from "$lib/chats/api";

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
  let activeChatTab = $state<"groups" | "direct">("groups");
  let lastTabSyncedRoomId: string | null = null;
  let createChatOpen = $state(false);
  let createChatUserId = $state("");
  let createChatEncrypted = $state(true);
  let creatingChat = $state(false);
  let createChatError = $state("");

  const visibleRooms = $derived.by(() => {
    const query = searchQuery.trim().toLowerCase();

    const filtered = rooms
      .filter((room) => room.kind === "room")
      .filter((room) => room.joined)
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
  const groupUnreadCount = $derived(groupRooms.reduce((total, room) => total + room.unreadCount, 0));
  const directUnreadCount = $derived(directRooms.reduce((total, room) => total + room.unreadCount, 0));
  const activeRooms = $derived(activeChatTab === "groups" ? groupRooms : directRooms);

  $effect(() => {
    // Room summaries change during sync, but that must not undo a tab the user
    // chose manually. Switch tabs only when the selected room actually changes.
    if (!selectedRoomId) {
      lastTabSyncedRoomId = null;
      return;
    }
    if (selectedRoomId === lastTabSyncedRoomId) return;
    const selected = rooms.find((room) => room.roomId === selectedRoomId && room.joined);
    if (selected) {
      activeChatTab = selected.isDirect ? "direct" : "groups";
      lastTabSyncedRoomId = selectedRoomId;
    }
  });

  async function handleCreateChat(event: SubmitEvent) {
    event.preventDefault();
    const userId = createChatUserId.trim();
    if (!userId || creatingChat) return;
    creatingChat = true;
    createChatError = "";
    try {
      const { roomId } = await matrixOpenDirectChat(userId, createChatEncrypted);
      createChatOpen = false;
      createChatUserId = "";
      onSelectRoom?.(roomId);
    } catch (error) {
      createChatError = error instanceof Error ? error.message : String(error);
    } finally {
      creatingChat = false;
    }
  }
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

  <div class="grid shrink-0 grid-cols-2 border-b border-hairline" role="tablist" aria-label="Conversation type">
    <button type="button" role="tab" aria-selected={activeChatTab === "groups"} class={`cursor-pointer border-b-2 px-3 py-2 text-sm font-semibold transition-colors ${activeChatTab === "groups" ? "border-accent text-primary" : "border-transparent text-secondary hover:text-primary"}`} onclick={() => activeChatTab = "groups"}>Groups{#if groupUnreadCount}<span class="ml-1 text-xs opacity-70">{groupUnreadCount}</span>{/if}</button>
    <button type="button" role="tab" aria-selected={activeChatTab === "direct"} class={`cursor-pointer border-b-2 px-3 py-2 text-sm font-semibold transition-colors ${activeChatTab === "direct" ? "border-accent text-primary" : "border-transparent text-secondary hover:text-primary"}`} onclick={() => activeChatTab = "direct"}>Direct messages{#if directUnreadCount}<span class="ml-1 text-xs opacity-70">{directUnreadCount}</span>{/if}</button>
  </div>

  <div class="chat-list-scroll min-h-0 flex-1 overflow-y-auto">
    {#if activeRooms.length === 0}
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
      <ul>
        {#each activeRooms as room (room.roomId)}
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
    {#if activeChatTab === "groups"}
      <ExploreCommunities />
    {:else}
      <button type="button" class="flex w-full cursor-pointer items-center gap-3 border-t border-hairline px-4 py-3 text-left text-sm text-secondary transition hover:bg-hover hover:text-primary" onclick={() => { createChatError = ""; createChatOpen = true; }}>
        <svg viewBox="0 0 24 24" class="size-5 shrink-0" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
        <span>Create Chat</span>
      </button>
    {/if}
  </div>
  {/if}
</aside>

{#if createChatOpen}
  <dialog open class="fixed inset-0 z-[100] m-0 grid h-dvh w-screen max-w-none place-items-center border-0 bg-black/55 p-4" aria-modal="true" aria-labelledby="create-chat-title">
    <section class="w-full max-w-md border border-hairline bg-panel shadow-2xl">
      <header class="flex items-start justify-between gap-4 border-b border-hairline px-5 py-4">
        <div>
          <h2 id="create-chat-title" class="text-lg font-semibold text-primary">Create Chat</h2>
          <p class="mt-1 text-sm text-secondary">Start a private chat by entering a user ID.</p>
        </div>
        <button type="button" class="cursor-pointer px-1 text-xl leading-none text-secondary hover:text-primary" aria-label="Close" onclick={() => createChatOpen = false}>×</button>
      </header>

      <form class="space-y-5 p-5" onsubmit={handleCreateChat}>
        <label class="block space-y-1.5">
          <span class="text-sm font-medium text-primary">User ID</span>
          <input type="text" bind:value={createChatUserId} placeholder="@user:server.org" autocomplete="off" spellcheck="false" class="w-full border border-hairline bg-panel-alt px-3 py-2.5 text-sm text-primary outline-none placeholder:text-muted focus:border-accent" required disabled={creatingChat} />
        </label>

        <fieldset class="space-y-3 border-t border-hairline pt-4">
          <legend class="px-0 text-sm font-semibold text-primary">Options</legend>
          <label class="flex cursor-pointer items-start gap-3">
            <input type="checkbox" bind:checked={createChatEncrypted} class="mt-0.5 accent-[var(--tg-accent)]" disabled={creatingChat} />
            <span>
              <span class="block text-sm font-medium text-primary">End-to-End Encryption</span>
              <span class="mt-1 block text-xs leading-relaxed text-secondary">Once this feature is enabled, it can't be disabled after the room is created.</span>
            </span>
          </label>
        </fieldset>

        {#if createChatError}<p class="text-sm text-danger" role="alert">{createChatError}</p>{/if}
        <div class="flex justify-end gap-2 border-t border-hairline pt-4">
          <button type="button" class="btn" disabled={creatingChat} onclick={() => createChatOpen = false}>Cancel</button>
          <button type="submit" class="btn btn-primary" disabled={creatingChat || !createChatUserId.trim()}>{creatingChat ? "Creating…" : "Create"}</button>
        </div>
      </form>
    </section>
  </dialog>
{/if}
