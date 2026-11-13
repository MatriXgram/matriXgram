<script lang="ts">
  import { onMount } from "svelte";
  import { getCachedPublicRooms, matrixJoinRoom, matrixSearchPublicRooms, matrixTriggerRoomUpdate } from "$lib/chats/api";
  import { shellChats, shellCurrentUserId } from "$lib/chats/shell";
  import type { MatrixPublicRoom } from "$lib/chats/types";

  const STORAGE_KEY = "matrix-telegram.community-directories";
  const defaults = [
    "matrix.org",
    "matrixrooms.info",
    "mozilla.org",
    "unredacted.org",
    "fedoraproject.org",
    "ubuntu.com",
  ];
  let directories = $state<string[]>(defaults);
  let selectedServer = $state("matrix.org");
  let query = $state("");
  let rooms = $state<MatrixPublicRoom[]>([]);
  let nextBatch = $state<string | null>(null);
  let loading = $state(false);
  let error = $state("");
  let customServer = $state("");
  let joining = $state("");
  let joined = $state<string[]>([]);
  let failedAvatars = $state<string[]>([]);
  let searchSequence = 0;
  const roomCache = new Map<string, { rooms: MatrixPublicRoom[]; nextBatch: string | null }>();

  function userServer(userId: string) {
    const colon = userId.lastIndexOf(":");
    return colon >= 0 ? userId.slice(colon + 1) : "";
  }

  function isJoined(roomId: string) {
    return joined.includes(roomId) || $shellChats.some((chat) => chat.roomId === roomId && chat.joined);
  }

  function addOwnServer(userId: string) {
    const own = userServer(userId);
    if (own && !directories.some((server) => server.toLowerCase() === own.toLowerCase())) {
      directories = [...directories, own];
    }
  }

  onMount(() => {
    try {
      const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
      if (Array.isArray(saved)) directories = [...new Set([...defaults, ...saved.filter((item) => typeof item === "string")])];
    } catch { /* use built-in directories */ }
    addOwnServer($shellCurrentUserId);
    void loadRooms();
  });

  $effect(() => addOwnServer($shellCurrentUserId));

  async function loadRooms(reset = true) {
    if (!selectedServer || (loading && !reset)) return;
    const sequence = ++searchSequence;
    const cacheKey = `${selectedServer.toLowerCase()}\n${query.trim().toLowerCase()}`;
    loading = true;
    error = "";
    if (reset) {
      const cached = roomCache.get(cacheKey) ?? (() => {
        const result = getCachedPublicRooms(selectedServer, query.trim());
        return result ? { rooms: result.rooms, nextBatch: result.nextBatch ?? null } : null;
      })();
      rooms = cached?.rooms ?? [];
      nextBatch = cached?.nextBatch ?? null;
    }
    try {
      const result = await matrixSearchPublicRooms({
        serverName: selectedServer,
        searchTerm: query.trim() || undefined,
        since: reset ? undefined : nextBatch ?? undefined,
        limit: 20,
      });
      if (sequence !== searchSequence) return;
      rooms = reset ? result.rooms : [...rooms, ...result.rooms];
      nextBatch = result.nextBatch ?? null;
      roomCache.set(cacheKey, { rooms, nextBatch });
    } catch (cause) {
      if (sequence !== searchSequence) return;
      error = cause instanceof Error ? cause.message : String(cause);
    } finally {
      if (sequence === searchSequence) loading = false;
    }
  }

  function addServer() {
    const server = customServer.trim().replace(/^https?:\/\//i, "").replace(/\/$/, "");
    if (!server || server.includes("/") || directories.some((item) => item.toLowerCase() === server.toLowerCase())) return;
    directories = [...directories, server];
    localStorage.setItem(STORAGE_KEY, JSON.stringify(directories.filter((item) => !defaults.includes(item))));
    selectedServer = server;
    customServer = "";
    void loadRooms();
  }

  async function joinRoom(room: MatrixPublicRoom) {
    joining = room.roomId;
    error = "";
    try {
      await matrixJoinRoom(room.canonicalAlias || room.roomId, [room.directoryServer]);
      joined = [...joined, room.roomId];
      await matrixTriggerRoomUpdate({ includeSelectedMessages: false });
    } catch (cause) { error = cause instanceof Error ? cause.message : String(cause); }
    finally { joining = ""; }
  }
</script>

<main class="flex h-full min-h-0 flex-col bg-panel">
  <header class="flex shrink-0 items-center gap-3 border-b border-hairline px-5 py-4">
    <a href="/chats" class="grid size-9 place-items-center rounded-lg text-secondary transition hover:bg-panel-alt hover:text-primary" aria-label="Back to chats" title="Back to chats">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="size-5" aria-hidden="true"><path d="m15 18-6-6 6-6M9 12h12"/></svg>
    </a>
    <div><h1 class="text-lg font-semibold">Explore communities</h1><p class="text-xs text-muted">Find public rooms across Matrix</p></div>
  </header>

  <div class="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[240px_minmax(0,1fr)]">
    <aside class="flex min-h-0 flex-col border-b border-hairline bg-panel-alt/40 md:border-b-0 md:border-r">
      <div class="px-3 pb-1 pt-3"><h2 class="text-xs font-semibold uppercase tracking-wide text-muted">Directory servers</h2></div>
      <nav class="min-h-0 flex-1 overflow-y-auto px-0 pb-1" aria-label="Directory servers">
        {#each directories as server (server)}
          <button type="button" class={`flex w-full cursor-pointer items-center gap-2 border-l-2 px-2 py-1.5 text-left text-sm transition-colors ${selectedServer === server ? "border-accent bg-accent/5 text-primary" : "border-transparent text-secondary hover:text-primary"}`} aria-current={selectedServer === server ? "true" : undefined} onclick={() => { selectedServer = server; void loadRooms(); }}>
            <span class="grid size-6 shrink-0 place-items-center rounded-full bg-panel text-[10px] font-semibold uppercase text-muted">{server.slice(0, 1)}</span>
            <span class="min-w-0 flex-1 truncate">{server}</span>
            {#if server === userServer($shellCurrentUserId)}<span class="shrink-0 text-[10px] text-muted">You</span>{/if}
            {#if selectedServer === server}<span class="size-1.5 rounded-full bg-accent"></span>{/if}
          </button>
        {/each}
      </nav>
      <form class="flex shrink-0 gap-1 border-t border-hairline p-2" onsubmit={(event) => { event.preventDefault(); addServer(); }}>
        <input bind:value={customServer} class="min-w-0 flex-1 rounded-md border border-hairline bg-panel px-2 py-2 text-xs outline-none focus:border-accent" placeholder="Add server name" aria-label="Add directory server" />
        <button type="submit" class="btn btn-sm" disabled={!customServer.trim()} aria-label="Add directory server" title="Add directory server">Add</button>
      </form>
    </aside>

    <section class="flex min-h-0 min-w-0 flex-col">
      <div class="flex shrink-0 flex-wrap items-center justify-between gap-3 border-b border-hairline px-5 py-4">
        <div class="min-w-0"><h2 class="truncate text-base font-semibold">{selectedServer}</h2><p class="text-xs text-muted">Public rooms directory</p></div>
        <form class="flex w-full gap-2 sm:w-auto" onsubmit={(event) => { event.preventDefault(); void loadRooms(); }}>
          <input bind:value={query} class="min-w-0 flex-1 rounded-lg border border-hairline bg-panel-alt px-3 py-2 text-sm outline-none focus:border-accent sm:w-64" placeholder="Search rooms" aria-label="Search public rooms" />
          <button type="submit" class="btn" disabled={loading}>Search</button>
        </form>
      </div>
      {#if error}<p class="mx-5 mt-4 rounded-lg border border-danger/30 bg-danger/5 p-3 text-sm text-danger">{error}</p>{/if}
      <div class="min-h-0 flex-1 overflow-y-auto p-4 sm:p-5">
        {#if loading && rooms.length === 0}
          <div class="grid h-full place-items-center text-sm text-muted" role="status">Loading public rooms…</div>
        {:else if rooms.length === 0 && !error}
          <div class="grid h-full place-items-center text-sm text-muted">No public rooms found.</div>
        {:else}
          <ul class="mx-auto grid max-w-5xl grid-cols-1 gap-3 md:grid-cols-2">
            {#each rooms as room (room.roomId)}
              <li class="flex min-w-0 gap-3 rounded-xl border border-hairline bg-panel p-4 transition hover:border-accent/40">
                {#if room.avatarUrl && !failedAvatars.includes(room.roomId)}<img src={room.avatarUrl} alt="" loading="lazy" class="size-12 shrink-0 rounded-xl object-cover" onerror={() => { failedAvatars = [...failedAvatars, room.roomId]; }} />{:else}<div class="grid size-12 shrink-0 place-items-center rounded-xl bg-panel-alt text-lg font-semibold text-muted">{(room.name || room.canonicalAlias || "#").slice(0, 1).toUpperCase()}</div>{/if}
                <div class="min-w-0 flex-1">
                  <h3 class="truncate text-sm font-semibold">{room.name || room.canonicalAlias || room.roomId}</h3>
                  {#if room.canonicalAlias}<p class="truncate text-xs text-muted">{room.canonicalAlias}</p>{/if}
                  <p class="mt-2 line-clamp-3 text-sm text-secondary" title={room.topic || "No description provided."}>{room.topic || "No description provided."}</p>
                  <div class="mt-3 flex items-center justify-between gap-3">
                    <span class="text-xs text-muted">{room.joinedMembers.toLocaleString()} members</span>
                    <button type="button" class="btn btn-sm" disabled={joining === room.roomId || isJoined(room.roomId)} onclick={() => void joinRoom(room)}>{isJoined(room.roomId) ? "Joined" : joining === room.roomId ? "Joining…" : "Join"}</button>
                  </div>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
        {#if nextBatch}<div class="mx-auto mt-4 max-w-5xl"><button type="button" class="btn w-full" disabled={loading} onclick={() => void loadRooms(false)}>{loading ? "Loading…" : "Load more rooms"}</button></div>{/if}
      </div>
    </section>
  </div>
</main>
