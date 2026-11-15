<script lang="ts">
  import { matrixGetRoomImage, matrixJoinRoom, matrixTriggerRoomUpdate } from "$lib/chats/api";
  import { getDirectRoomPeer } from "$lib/chats/room-members-cache";
  import type { MatrixChatSummary } from "$lib/chats/types";
  import { shellCurrentUserId, shellErrorMessage } from "$lib/chats/shell";
  import { toChatListTime } from "$lib/components/messaging/shared";
  import { RoomAvatar, RoomSettingsDialog, isVirtualRoomId, roomImageCache, roomImageChangeEpoch } from "../shared";

  interface Props {
    room: MatrixChatSummary;
    depth: number;
    isSelected: boolean;
    hasChildren: boolean;
    isExpanded: boolean;
    onSelect?: (roomId: string) => void;
    onToggleSpace?: (roomId: string) => void;
    compact?: boolean;
  }

  let {
    room,
    depth,
    isSelected,
    hasChildren,
    isExpanded,
    onSelect,
    onToggleSpace,
    compact = false,
  }: Props = $props();

  const isSpace = $derived(room.kind === "space");
  let lazyImageUrl = $state<string | null | undefined>(null);
  let directPeerId = $state<string | null>(null);
  let settingsDialogOpen = $state(false);
  let isContextMenuOpen = $state(false);
  let contextMenuPosition = $state<{ x: number; y: number } | null>(null);
  let contextMenuElement: HTMLDivElement | undefined = $state();
  let isJoining = $state(false);

  const contextMenuAvailable = $derived(!isVirtualRoomId(room.roomId));
  const settingsAvailable = $derived(!isVirtualRoomId(room.roomId) && room.joined);
  const previewSenderSeparator = $derived(room.lastMessagePreview?.indexOf(": ") ?? -1);

  const indentation = $derived(`${Math.max(0, depth) * 0.9}rem`);
  const avatarIdentity = $derived(room.isDirect ? directPeerId ?? room.roomId : room.roomId);

  $effect(() => {
    const roomId = room.roomId;
    directPeerId = null;
    if (!room.isDirect) return;
    let cancelled = false;
    void getDirectRoomPeer(roomId, $shellCurrentUserId).then((peer) => {
      if (!cancelled && room.roomId === roomId) directPeerId = peer?.userId ?? null;
    }).catch(() => {});
    return () => { cancelled = true; };
  });

  $effect(() => {
    const cacheEpoch = roomImageChangeEpoch.current;
    lazyImageUrl = cacheEpoch === 0 ? room.imageUrl : undefined;

    if (room.imageUrl && cacheEpoch === 0) {
      roomImageCache.prime(room.roomId, room.imageUrl);
    }
  });

  $effect(() => {
    void roomImageChangeEpoch.current;
    if (lazyImageUrl || isVirtualRoomId(room.roomId)) {
      return;
    }

    const cachedImage = roomImageCache.getCached(room.roomId);
    if (cachedImage !== undefined) {
      lazyImageUrl = cachedImage;
      return;
    }

    const roomId = room.roomId;
    void roomImageCache
      .getOrLoad(room.roomId, () => matrixGetRoomImage(room.roomId))
      .then((imageUrl) => {
        if (room.roomId === roomId) {
          lazyImageUrl = imageUrl;
        }
      });
  });

  async function handleJoin(event?: MouseEvent) {
    if (event) {
      event.stopPropagation();
    }
    if (isJoining || room.joined) {
      return;
    }

    isJoining = true;
    shellErrorMessage.set("");

    try {
      const res = await matrixJoinRoom(room.roomId);
      await matrixTriggerRoomUpdate();
      onSelect?.(res.roomId || room.roomId);
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to join room");
    } finally {
      isJoining = false;
    }
  }

  function handleClick() {
    if (isSpace) {
      if (hasChildren) {
        onToggleSpace?.(room.roomId);
      }
      return;
    }

    if (!room.joined) {
      void handleJoin();
      return;
    }

    onSelect?.(room.roomId);
  }

  $effect(() => {
    if (!isContextMenuOpen) {
      return;
    }

    const onWindowPointerDown = (event: PointerEvent) => {
      if (!(event.target instanceof Node)) {
        return;
      }

      if (contextMenuElement?.contains(event.target)) {
        return;
      }

      closeContextMenu();
    };

    const onWindowKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }

      closeContextMenu();
    };

    window.addEventListener("pointerdown", onWindowPointerDown, true);
    window.addEventListener("keydown", onWindowKeyDown);

    return () => {
      window.removeEventListener("pointerdown", onWindowPointerDown, true);
      window.removeEventListener("keydown", onWindowKeyDown);
    };
  });

  function openContextMenu(event: MouseEvent) {
    if (!contextMenuAvailable) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    contextMenuPosition = { x: event.clientX, y: event.clientY };
    isContextMenuOpen = true;
  }

  function closeContextMenu() {
    isContextMenuOpen = false;
    contextMenuPosition = null;
  }

  function openSettingsFromContextMenu() {
    closeContextMenu();
    settingsDialogOpen = true;
  }
</script>

<li class={compact ? "flex justify-center py-0.5" : undefined} oncontextmenu={openContextMenu}>
  {#if compact}
    <button type="button" class={`relative grid size-11 cursor-pointer place-items-center rounded-full transition-all duration-150 hover:scale-105 hover:bg-accent-soft hover:ring-1 hover:ring-accent/40 ${isSelected ? "bg-primary-100-900 ring-1 ring-accent/40" : ""}`} title={`${room.displayName}${room.unreadCount ? ` · ${room.unreadCount} unread` : ""}`} aria-label={`Open ${room.displayName}`} aria-current={isSelected ? "true" : undefined} onclick={handleClick}>
      <RoomAvatar imageUrl={lazyImageUrl} displayName={room.displayName} identity={avatarIdentity} />
      {#if room.unreadCount > 0}<span class="absolute -right-0.5 -top-0.5 min-w-4 rounded-full bg-badge px-1 text-center text-[9px] font-semibold leading-4 text-badge-fg">{room.unreadCount > 99 ? "99+" : room.unreadCount}</span>{/if}
    </button>
  {:else}
  <div
    class="chat-list-row flex items-center justify-between w-full transition-colors"
    class:group-room-row={!room.isDirect}
    aria-current={isSelected ? "true" : undefined}
    class:hover:bg-surface-200-800={!isSpace || hasChildren}
    class:opacity-90={isSpace}
    class:bg-primary-100-900={isSelected}
      style={`padding-left: calc(0.625rem + ${indentation});`}
  >
    <button
      type="button"
      class="flex-1 text-left min-w-0"
      class:cursor-pointer={!isSpace || hasChildren}
      class:cursor-default={isSpace && !hasChildren}
      onclick={handleClick}
    >
      <div class="flex items-center gap-3 min-w-0">
        <RoomAvatar imageUrl={lazyImageUrl} displayName={room.displayName} identity={avatarIdentity} />

        <div class="min-w-0 flex-1">
          <div class="flex items-baseline justify-between gap-2">
            <p class="min-w-0 flex-1 truncate text-[13px] font-semibold" class:text-accent={room.isPinned}>
              {#if isSpace}
                {isExpanded ? "▼" : "▶"}
              {/if}
              {room.displayName}
            </p>

            <span class="shrink-0 text-[11px] text-secondary">
              {toChatListTime(room.lastMessageTs)}
            </span>
          </div>

          <div class="flex items-center justify-between gap-2">
            <p class="min-w-0 flex-1 truncate text-xs text-secondary">
              {#if room.lastMessagePreview}
                {#if previewSenderSeparator >= 0}
                  <span class="text-primary">{room.lastMessagePreview.slice(0, previewSenderSeparator + 1)}</span>
                  <span>{room.lastMessagePreview.slice(previewSenderSeparator + 2)}</span>
                {:else}
                  {room.lastMessagePreview}
                {/if}
              {:else if room.kind === "space"}
                {#if !room.joined}
                  Not joined
                {:else}
                  {hasChildren ? "Contains rooms" : "No child rooms"}
                {/if}
              {:else if !room.joined}
                Not joined
              {:else}
                {room.encrypted ? "Encrypted" : "Unencrypted"} • {room.joinedMembers} members
              {/if}
            </p>

            <div class="flex shrink-0 items-center gap-1">
              {#if room.isMuted}
                <span class="text-secondary" title="Muted" aria-label="Muted">🔕</span>
              {/if}
              {#if room.unreadCount > 0}
                <span
                  class="flex h-5 min-w-5 items-center justify-center rounded-full bg-badge px-1.5 text-[11px] font-semibold text-badge-fg"
                >
                  {room.unreadCount > 99 ? "99+" : room.unreadCount}
                </span>
              {/if}
            </div>
          </div>
        </div>
      </div>
    </button>

    <div class="flex items-center gap-1.5 shrink-0 ml-2">
      {#if room.kind === "space"}
        <span class="text-[10px] uppercase tracking-wide text-surface-700-300">Space</span>
      {/if}
      {#if !room.joined}
        <button
          type="button"
          class="btn preset-filled-primary-500 text-xs px-2.5 py-1"
          disabled={isJoining}
          onclick={handleJoin}
        >
          {isJoining ? "Joining..." : "Join"}
        </button>
      {/if}
    </div>
  </div>
  {/if}

  {#if isContextMenuOpen && contextMenuPosition}
    <div
      class="fixed z-30 min-w-28 overflow-hidden rounded border border-surface-300-700 bg-surface-50-950 shadow"
      style={`left: ${contextMenuPosition.x}px; top: ${contextMenuPosition.y}px;`}
      role="menu"
      bind:this={contextMenuElement}
    >
      {#if !room.joined}
        <button
          type="button"
          class="block w-full px-3 py-2 text-left text-sm hover:bg-surface-200-800 text-primary-500 font-medium"
          role="menuitem"
          disabled={isJoining}
          onclick={() => {
            closeContextMenu();
            void handleJoin();
          }}
        >
          {isJoining ? "Joining..." : "Join"}
        </button>
      {:else if settingsAvailable}
        <button
          type="button"
          class="block w-full px-3 py-2 text-left text-sm hover:bg-surface-200-800"
          role="menuitem"
          onclick={openSettingsFromContextMenu}
        >
          Settings
        </button>
      {/if}
    </div>
  {/if}

  {#if settingsAvailable}
    <RoomSettingsDialog
      open={settingsDialogOpen}
      roomId={room.roomId}
      roomName={room.displayName}
      isSpace={isSpace}
      isDirect={room.isDirect}
      onClose={() => (settingsDialogOpen = false)}
    />
  {/if}
</li>
