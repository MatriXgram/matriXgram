<script lang="ts">
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { getCachedPublicRooms, matrixJoinRoom, matrixSearchPublicRooms, matrixTriggerRoomUpdate } from "$lib/chats/api";
  import { shellChats, shellCurrentUserId } from "$lib/chats/shell";
  import type { MatrixPublicRoom } from "$lib/chats/types";

  const STORAGE_KEY = "matrixgram.community-directories";
  const LEGACY_STORAGE_KEY = "matrix-telegram.community-directories";
  const PENDING_JOINS_KEY = "matrixgram.pending-room-joins";
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
  let directoryTab = $state<"servers" | "pending">("servers");
  let joining = $state("");
  let joined = $state<string[]>([]);
  let pendingJoins = $state<{ roomId: string; name: string; alias: string; server: string; since: number }[]>([]);
  let failedAvatars = $state<string[]>([]);
  let selectedRoom = $state<MatrixPublicRoom | null>(null);
  let linkContextMenu = $state<{ url: string; x: number; y: number } | null>(null);
  let searchSequence = 0;
  const roomCache = new Map<string, { rooms: MatrixPublicRoom[]; nextBatch: string | null }>();

  function splitLinks(text: string) {
    const parts: { text: string; url?: string }[] = [];
    const pattern = /https?:\/\/[^\s<>"']+/gi;
    let cursor = 0;
    for (const match of text.matchAll(pattern)) {
      const start = match.index ?? 0;
      const raw = match[0];
      const trailing = raw.match(/[),.!?;:]+$/)?.[0] ?? "";
      const url = raw.slice(0, raw.length - trailing.length);
      if (start > cursor) parts.push({ text: text.slice(cursor, start) });
      if (url) parts.push({ text: url, url });
      if (trailing) parts.push({ text: trailing });
      cursor = start + raw.length;
    }
    if (cursor < text.length) parts.push({ text: text.slice(cursor) });
    return parts;
  }

  function showLinkMenu(event: MouseEvent, url: string) {
    event.preventDefault();
    event.stopPropagation();
    linkContextMenu = { url, x: event.clientX, y: event.clientY };
  }

  async function copyContextLink() {
    if (!linkContextMenu) return;
    try { await navigator.clipboard.writeText(linkContextMenu.url); }
    finally { linkContextMenu = null; }
  }

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
      const saved = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? localStorage.getItem(LEGACY_STORAGE_KEY) ?? "[]");
      if (Array.isArray(saved)) directories = [...new Set([...defaults, ...saved.filter((item) => typeof item === "string")])];
      if (!localStorage.getItem(STORAGE_KEY) && localStorage.getItem(LEGACY_STORAGE_KEY)) localStorage.setItem(STORAGE_KEY, JSON.stringify(saved));
    } catch { /* use built-in directories */ }
    try {
      const savedPending = JSON.parse(localStorage.getItem(PENDING_JOINS_KEY) ?? "[]");
      if (Array.isArray(savedPending)) pendingJoins = savedPending;
    } catch { /* ignore invalid pending-join cache */ }
    addOwnServer($shellCurrentUserId);
    void loadRooms();
  });

  $effect(() => addOwnServer($shellCurrentUserId));

  $effect(() => {
    const stillPending = pendingJoins.filter((room) => !isJoined(room.roomId));
    if (stillPending.length !== pendingJoins.length) {
      pendingJoins = stillPending;
      localStorage.setItem(PENDING_JOINS_KEY, JSON.stringify(stillPending));
    }
  });

  async function loadRooms(reset = true) {
    if (!selectedServer || (loading && !reset)) return;
    const requestedCursor = reset ? undefined : nextBatch ?? undefined;
    if (!reset && !requestedCursor) return;
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
        since: requestedCursor,
        limit: 20,
      });
      if (sequence !== searchSequence) return;
      if (reset) {
        rooms = result.rooms;
      } else {
        // Public directories may repeat rooms between pages. Keep the list
        // stable and keyed uniquely so a repeated page does not look like a
        // failed load (or cause duplicate-key rendering issues).
        const knownRoomIds = new Set(rooms.map((room) => room.roomId));
        rooms = [...rooms, ...result.rooms.filter((room) => !knownRoomIds.has(room.roomId))];
      }
      // A broken directory can return its current pagination token again.
      // Stop offering the same page forever in that case.
      nextBatch = result.nextBatch && result.nextBatch !== requestedCursor ? result.nextBatch : null;
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
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : String(cause);
      if (message.startsWith("JOIN_PENDING:")) {
        pendingJoins = [
          ...pendingJoins.filter((item) => item.roomId !== room.roomId),
          { roomId: room.roomId, name: room.name || room.canonicalAlias || room.roomId, alias: room.canonicalAlias || room.roomId, server: room.directoryServer, since: Date.now() },
        ];
        localStorage.setItem(PENDING_JOINS_KEY, JSON.stringify(pendingJoins));
      } else error = message;
    }
    finally { joining = ""; }
  }
</script>

<svelte:window onpointerdown={() => linkContextMenu = null} onkeydown={(event) => { if (event.key === "Escape") linkContextMenu = null; }} />

<main class="flex h-full min-h-0 flex-col bg-panel">
  <header class="flex shrink-0 items-center gap-3 border-b border-hairline px-5 py-4">
    <a href="/chats" class="grid size-9 place-items-center rounded-lg text-secondary transition hover:bg-panel-alt hover:text-primary" aria-label="Back to chats" title="Back to chats">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="size-5" aria-hidden="true"><path d="m15 18-6-6 6-6M9 12h12"/></svg>
    </a>
    <div><h1 class="text-lg font-semibold">Explore communities</h1><p class="text-xs text-muted">Find public rooms across Matrix</p></div>
  </header>

  <div class="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[240px_minmax(0,1fr)]">
    <aside class="flex min-h-0 flex-col border-b border-hairline bg-panel-alt/40 md:border-b-0 md:border-r">
      <div class="grid shrink-0 grid-cols-2 border-b border-hairline text-xs font-medium">
        <button type="button" class={`cursor-pointer border-b-2 px-2 py-2 ${directoryTab === "servers" ? "border-accent text-primary" : "border-transparent text-muted hover:text-primary"}`} onclick={() => directoryTab = "servers"}>Servers</button>
        <button type="button" class={`cursor-pointer border-b-2 px-2 py-2 ${directoryTab === "pending" ? "border-accent text-primary" : "border-transparent text-muted hover:text-primary"}`} onclick={() => directoryTab = "pending"}>Pending{#if pendingJoins.length} ({pendingJoins.length}){/if}</button>
      </div>
      {#if directoryTab === "servers"}
        <nav class="min-h-0 flex-1 overflow-y-auto px-0 py-1" aria-label="Directory servers">
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
      {:else}
        <div class="min-h-0 flex-1 overflow-y-auto px-2 py-1" aria-label="Rooms waiting to join">
          {#if pendingJoins.length === 0}
            <p class="px-1 py-3 text-xs text-muted">No rooms waiting to join.</p>
          {:else}
            {#each pendingJoins as pending (pending.roomId)}
              <div class="flex min-w-0 items-start gap-2 border-b border-hairline py-2">
                <span class="mt-1 size-2 shrink-0 animate-pulse rounded-full bg-accent" aria-hidden="true"></span>
                <div class="min-w-0 flex-1">
                  <p class="truncate text-sm text-primary" title={pending.name}>{pending.name}</p>
                  <p class="truncate text-xs text-muted" title={pending.alias}>{pending.alias}</p>
                  <p class="mt-1 text-xs text-secondary">Waiting for {pending.server}</p>
                </div>
                <button type="button" class="cursor-pointer px-1 text-muted hover:text-primary" aria-label={`Remove ${pending.name} from pending list`} title="Dismiss" onclick={() => { pendingJoins = pendingJoins.filter((item) => item.roomId !== pending.roomId); localStorage.setItem(PENDING_JOINS_KEY, JSON.stringify(pendingJoins)); }}>×</button>
              </div>
            {/each}
            <p class="px-1 py-2 text-xs text-muted">This list updates when your homeserver syncs membership.</p>
          {/if}
        </div>
      {/if}
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
      <div class="min-h-0 flex-1 overflow-y-auto p-1 sm:p-2">
        <div class="min-h-full">
        {#if loading && rooms.length === 0}
          <div class="grid h-full place-items-center text-sm text-muted" role="status">Loading public rooms…</div>
        {:else if rooms.length === 0 && !error}
          <div class="grid h-full place-items-center text-sm text-muted">No public rooms found.</div>
        {:else}
          <ul class="mx-auto grid max-w-5xl grid-cols-1 gap-x-0 gap-y-1 md:grid-cols-2 md:divide-x md:divide-hairline">
            {#each rooms as room (room.roomId)}
              <li role="button" tabindex="0" class="flex min-w-0 cursor-pointer gap-2 border-b border-hairline px-1 py-2 transition-colors hover:bg-panel-alt/40 focus-visible:bg-panel-alt/40 md:px-2" onclick={() => selectedRoom = room} onkeydown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); selectedRoom = room; } }}>
                {#if room.avatarUrl && !failedAvatars.includes(room.roomId)}<img src={room.avatarUrl} alt="" loading="lazy" class="size-12 shrink-0 rounded-full object-cover" onerror={() => { failedAvatars = [...failedAvatars, room.roomId]; }} />{:else}<div class="grid size-12 shrink-0 place-items-center rounded-full bg-panel-alt text-lg font-semibold text-muted">{(room.name || room.canonicalAlias || "#").slice(0, 1).toUpperCase()}</div>{/if}
                <div class="min-w-0 flex-1">
                  <h3 class="truncate text-sm font-semibold">{room.name || room.canonicalAlias || room.roomId}</h3>
                  {#if room.canonicalAlias}<p class="truncate text-xs text-muted">{room.canonicalAlias}</p>{/if}
                  <p class="mt-1 line-clamp-3 text-sm text-secondary" title={room.topic || "No description provided."}>{room.topic || "No description provided."}</p>
                  <div class="mt-2 flex items-center justify-between gap-2">
                    <span class="text-xs text-muted">{room.joinedMembers.toLocaleString()} members</span>
                    <button type="button" class="btn btn-sm" disabled={joining === room.roomId || isJoined(room.roomId)} onclick={(event) => { event.stopPropagation(); void joinRoom(room); }}>{isJoined(room.roomId) ? "Joined" : joining === room.roomId ? "Joining…" : "Join"}</button>
                  </div>
                </div>
              </li>
            {/each}
          </ul>
        {/if}
          {#if nextBatch}<div class="mx-auto mt-4 max-w-5xl"><button type="button" class="btn w-full" disabled={loading} onclick={() => void loadRooms(false)}>{loading ? "Loading…" : "Load more rooms"}</button></div>{/if}
        </div>
      </div>
    </section>
  </div>
</main>

{#if selectedRoom}
  <div class="fixed inset-0 z-50 grid place-items-center bg-black/55 p-4" role="presentation" onclick={(event) => { if (event.target === event.currentTarget) selectedRoom = null; }} onkeydown={(event) => { if (event.key === "Escape") selectedRoom = null; }}>
    <section class="w-full max-w-lg border border-hairline bg-panel shadow-2xl" role="dialog" aria-modal="true" aria-labelledby="community-room-title">
      <div class="flex items-start gap-4 border-b border-hairline p-4">
        {#if selectedRoom.avatarUrl && !failedAvatars.includes(selectedRoom.roomId)}<img src={selectedRoom.avatarUrl} alt="" class="size-16 shrink-0 rounded-full object-cover" onerror={() => { failedAvatars = [...failedAvatars, selectedRoom!.roomId]; }} />{:else}<div class="grid size-16 shrink-0 place-items-center rounded-full bg-panel-alt text-xl font-semibold text-muted">{(selectedRoom.name || selectedRoom.canonicalAlias || "#").slice(0, 1).toUpperCase()}</div>{/if}
        <div class="min-w-0 flex-1"><h2 id="community-room-title" class="break-words text-lg font-semibold">{selectedRoom.name || selectedRoom.canonicalAlias || selectedRoom.roomId}</h2><p class="mt-1 text-sm text-muted">{selectedRoom.joinedMembers.toLocaleString()} members · {selectedRoom.directoryServer}</p></div>
        <button type="button" class="grid size-8 shrink-0 cursor-pointer place-items-center text-secondary hover:text-primary" aria-label="Close room details" onclick={() => selectedRoom = null}>×</button>
      </div>
      <div class="max-h-[60vh] space-y-4 overflow-y-auto p-4">
        <div><h3 class="text-xs font-semibold uppercase tracking-wide text-muted">Description</h3><p class="mt-1 whitespace-pre-wrap break-words text-sm text-primary">{#each splitLinks(selectedRoom.topic || "No description provided.") as part, index}{#if part.url}<a href={part.url} class="cursor-pointer text-accent underline decoration-accent/50 underline-offset-2 hover:decoration-accent" onclick={(event) => { event.preventDefault(); void openUrl(part.url!); }} oncontextmenu={(event) => showLinkMenu(event, part.url!)}>{part.text}</a>{:else}{part.text}{/if}{/each}</p></div>
        <div><h3 class="text-xs font-semibold uppercase tracking-wide text-muted">Room ID</h3><p class="mt-1 break-all font-mono text-sm text-primary">{selectedRoom.roomId}</p></div>
        {#if selectedRoom.canonicalAlias}<div><h3 class="text-xs font-semibold uppercase tracking-wide text-muted">Address</h3><p class="mt-1 break-all text-sm text-primary">{selectedRoom.canonicalAlias}</p></div>{/if}
        <div><h3 class="text-xs font-semibold uppercase tracking-wide text-muted">Directory server</h3><p class="mt-1 break-all text-sm text-primary">{selectedRoom.directoryServer}</p></div>
      </div>
      <div class="flex justify-end gap-2 border-t border-hairline p-3">
        <button type="button" class="btn" onclick={() => selectedRoom = null}>Close</button>
        <button type="button" class="btn btn-primary" disabled={joining === selectedRoom.roomId || isJoined(selectedRoom.roomId)} onclick={() => void joinRoom(selectedRoom!)}>{isJoined(selectedRoom.roomId) ? "Joined" : joining === selectedRoom.roomId ? "Joining…" : "Join room"}</button>
      </div>
    </section>
  </div>
{/if}

{#if linkContextMenu}
  <div class="fixed z-[70] min-w-32 border border-hairline bg-panel py-1 shadow-xl" style={`left:${linkContextMenu.x}px;top:${linkContextMenu.y}px`} role="menu" onclick={(event) => event.stopPropagation()} onpointerdown={(event) => event.stopPropagation()}>
    <button type="button" class="w-full cursor-pointer px-3 py-2 text-left text-sm text-primary hover:bg-panel-alt" role="menuitem" onclick={() => void copyContextLink()}>Copy link</button>
  </div>
{/if}
