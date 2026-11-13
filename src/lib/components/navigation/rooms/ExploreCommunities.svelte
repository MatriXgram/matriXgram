<script lang="ts">
  interface Props { compact?: boolean }
  let { compact = false }: Props = $props();

  import { primeMatrixPublicRooms } from "$lib/chats/api";
  import { shellChatsLoading, shellCurrentUserId } from "$lib/chats/shell";

  let primedServersKey = "";
  $effect(() => {
    const userId = $shellCurrentUserId;
    const chatsLoading = $shellChatsLoading;
    const colon = userId.lastIndexOf(":");
    const ownServer = colon >= 0 ? userId.slice(colon + 1) : "";
    if (!userId || chatsLoading) return;

    const servers = [...new Set([
      "matrix.org",
      "matrixrooms.info",
      "mozilla.org",
      "unredacted.org",
      "fedoraproject.org",
      "ubuntu.com",
      ownServer,
    ].filter(Boolean))];
    const key = servers.join("|");
    if (key === primedServersKey) return;
    primedServersKey = key;
    for (const server of servers) primeMatrixPublicRooms(server);
  });
</script>

<a
  href="/chats?view=communities"
  class={compact
    ? "mx-auto my-2 grid size-11 place-items-center rounded-xl text-secondary transition hover:bg-panel-alt hover:text-primary"
    : "chat-list-row group-room-row flex w-full cursor-pointer items-center gap-3 text-left text-[13px] font-medium text-secondary transition-colors hover:bg-surface-200-800"}
  aria-label="Explore communities"
  title="Explore communities"
>
  {#if compact}
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" class="size-5" aria-hidden="true">
      <circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0-18"/>
    </svg>
  {:else}
    <span class="grid size-[46px] shrink-0 place-items-center rounded-full bg-accent-soft text-accent">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" class="size-5" aria-hidden="true">
        <circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0-18"/>
      </svg>
    </span>
    <span class="min-w-0 flex-1">
      <span class="block truncate text-[13px] font-semibold text-primary">Explore communities</span>
      <span class="block truncate text-xs text-secondary">Find public Matrix rooms</span>
    </span>
  {/if}
</a>
