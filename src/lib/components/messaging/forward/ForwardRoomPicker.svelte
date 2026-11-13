<script lang="ts">
  import type { MatrixChatSummary } from "$lib/chats/types";
  import type { TimelineMessage } from "../shared";

  interface Props {
    open: boolean;
    message: TimelineMessage | null;
    rooms: MatrixChatSummary[];
    error: string;
    onClose: () => void;
    onForward: (roomId: string) => void;
  }

  let { open, message, rooms, error, onClose, onForward }: Props = $props();

  let search = $state("");

  const visibleRooms = $derived.by(() => {
    const query = search.trim().toLowerCase();

    return rooms
      .filter((room) => room.kind === "room" && room.joined)
      .filter((room) => {
        if (!query) {
          return true;
        }

        return room.displayName.toLowerCase().includes(query);
      })
      .sort((left, right) => left.displayName.localeCompare(right.displayName));
  });

  $effect(() => {
    if (!open) {
      search = "";
    }
  });

  function handleKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      onClose();
    }
  }
</script>

{#if open && message}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 grid place-items-center bg-black/50 p-4"
    role="presentation"
    onclick={onClose}
    onkeydown={handleKeyDown}
  >
    <div
      class="flex max-h-[26rem] w-full max-w-sm flex-col overflow-hidden rounded-2xl border border-hairline bg-panel shadow-2xl"
      role="dialog"
      tabindex="-1"
      aria-modal="true"
      aria-label="Forward message"
      onclick={(event) => event.stopPropagation()}
      onkeydown={(event) => event.stopPropagation()}
    >
      <header class="border-b border-hairline px-4 py-3">
        <h2 class="text-sm font-semibold">Forward to…</h2>
        <p class="mt-0.5 truncate text-[11px] text-secondary">{message.body}</p>
      </header>

      <div class="border-b border-hairline p-2">
        <input
          type="search"
          bind:value={search}
          placeholder="Search chats"
          class="w-full rounded-lg border border-hairline bg-panel-alt px-3 py-1.5 text-[13px] outline-none placeholder:text-muted focus:border-accent"
        />
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto py-1">
        {#if visibleRooms.length === 0}
          <p class="px-4 py-6 text-center text-[13px] text-secondary">No chats found.</p>
        {:else}
          <ul>
            {#each visibleRooms as room (room.roomId)}
              <li>
                <button
                  type="button"
                  class="flex w-full items-center gap-2 px-4 py-1.5 text-left hover:bg-hover"
                  onclick={() => onForward(room.roomId)}
                >
                  <span
                    class="grid size-7 shrink-0 place-items-center rounded-full bg-hover text-[11px] font-semibold"
                  >
                    {room.displayName.charAt(0).toUpperCase()}
                  </span>
                  <span class="truncate text-[13px]">{room.displayName}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>

      {#if error}
        <p class="border-t border-hairline px-4 py-2 text-[11px] text-danger">{error}</p>
      {/if}

      <footer class="border-t border-hairline p-2">
        <button type="button" class="btn btn-sm w-full justify-center" onclick={onClose}>Cancel</button>
      </footer>
    </div>
  </div>
{/if}
