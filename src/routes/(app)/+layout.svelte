<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { get } from "svelte/store";
  import { onMount } from "svelte";
  import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";

  import { matrixSessionStatus } from "$lib/auth/api";
  import { matrixGetProfile, matrixRoomLeave } from "$lib/settings/api";
  import {
    matrixGetChatNavigation,
    matrixGetChats,
    matrixGetPickerAssets,
    matrixSetRootSpaceOrder,
    matrixTriggerRoomUpdate,
    matrixJoinRoom,
  } from "$lib/chats/api";
  import { mergeInitialChatSnapshot } from "$lib/chats/snapshot";
  import { subscribeToRoomUpdates } from "$lib/chats/realtime";
  import {
    logout as logoutSession,
    refreshRecoveryState,
    shellChats,
    shellChatsLoading,
    shellSigningOut,
    shellCurrentUserId,
    shellErrorMessage,
    shellRecoveryState,
    shellRefreshing,
    shellPickerCustomEmoji,
    shellRootScopedRooms,
    shellRootSpaces,
    shellSelectedRootSpaceId,
    shellSelectedRoomId,
  } from "$lib/chats/shell";
  import type {
    MatrixChatSummary,
    MatrixGetChatNavigationRequest,
    MatrixRoomRemovedEvent,
    MatrixSelectedRoomMessagesEvent,
  } from "$lib/chats/types";
  import { AppHeader, ChatList, RoomSettingsDialog } from "$lib/components/navigation";
  import { invalidateSenderAvatar, roomImageCache, senderAvatarCache } from "$lib/components/navigation/shared";

  import UserProfileDialog from "$lib/components/messaging/message/UserProfileDialog.svelte";
  import IncomingVerificationDialog from "$lib/components/verification/IncomingVerificationDialog.svelte";

  let { children } = $props();

  let checkingAuth = $state(true);
  let shellDisposed = false;
  let chatSyncTimeout: ReturnType<typeof setTimeout> | undefined;
  const shellActive = () => !shellDisposed && !get(shellSigningOut);
  let loggingOut = $state(false);
  let currentUserAvatar = $state<string | null>(null);
  let spaceSettingsDialogRoomId = $state<string | null>(null);
  let notificationsOpen = $state(false);
  let selectedInvite = $state<MatrixChatSummary | null>(null);
  let acceptingInvite = $state(false);
  let readInviteIds = $state<string[]>([]);
  let dismissedInviteIds = $state<string[]>([]);
  const inviteNotifications = $derived($shellChats.filter((chat) => chat.isInvited && !dismissedInviteIds.includes(chat.roomId)));
  const unreadInviteCount = $derived(inviteNotifications.filter((invite) => !readInviteIds.includes(invite.roomId)).length);

  function markInviteRead(roomId: string) {
    if (readInviteIds.includes(roomId)) return;
    readInviteIds = [...readInviteIds, roomId];
    localStorage.setItem("matrixgram.read-invites", JSON.stringify(readInviteIds));
  }

  function openNotifications() {
    notificationsOpen = !notificationsOpen;
    if (notificationsOpen) {
      readInviteIds = [...new Set([...readInviteIds, ...inviteNotifications.map((invite) => invite.roomId)])];
      localStorage.setItem("matrixgram.read-invites", JSON.stringify(readInviteIds));
    }
  }

  function handleNotificationOutsidePointer(event: PointerEvent) {
    if (!notificationsOpen) return;
    const target = event.target;
    if (target instanceof Element && target.closest("[data-notification-trigger], [data-notification-popover]")) return;
    notificationsOpen = false;
  }

  function clearReadNotifications() {
    const readIds = inviteNotifications.filter((invite) => readInviteIds.includes(invite.roomId)).map((invite) => invite.roomId);
    dismissedInviteIds = [...new Set([...dismissedInviteIds, ...readIds])];
    localStorage.setItem("matrixgram.dismissed-invites", JSON.stringify(dismissedInviteIds));
  }

  async function declineInvite(invite: MatrixChatSummary) {
    if (acceptingInvite) return;
    acceptingInvite = true;
    try {
      await matrixRoomLeave({ roomId: invite.roomId, forget: true });
      selectedInvite = null;
      dismissedInviteIds = [...new Set([...dismissedInviteIds, invite.roomId])];
      localStorage.setItem("matrixgram.dismissed-invites", JSON.stringify(dismissedInviteIds));
      shellChats.update((chats) => chats.filter((chat) => chat.roomId !== invite.roomId));
      await requestRefresh();
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to decline invitation");
    } finally {
      acceptingInvite = false;
    }
  }
  const isCommunityExplorer = $derived(page.url.searchParams.get("view") === "communities");

  async function openInvite(invite: MatrixChatSummary) {
    markInviteRead(invite.roomId);
    if (invite.isDirect) {
      await acceptInvite(invite);
    } else {
      selectedInvite = invite;
      notificationsOpen = false;
    }
  }

  async function acceptInvite(invite: MatrixChatSummary) {
    if (acceptingInvite) return;
    acceptingInvite = true;
    try {
      const { roomId } = await matrixJoinRoom(invite.roomId, invite.inviteViaServers);
      selectedInvite = null;
      notificationsOpen = false;
      shellSelectedRoomId.set(roomId);
      const params = new URLSearchParams(page.url.searchParams);
      params.set("roomId", roomId);
      await goto(`/chats?${params.toString()}`);
      await requestRefresh();
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to accept invitation");
    } finally {
      acceptingInvite = false;
    }
  }

  // The chat list can be resized by dragging the divider next to it.
  const SIDEBAR_WIDTH_KEY = "matrixgram.sidebarWidth";
  const LEGACY_SIDEBAR_WIDTH_KEY = "matrix-telegram.sidebarWidth";
  const SIDEBAR_WIDTH_DEFAULT = 300;
  const SIDEBAR_WIDTH_MIN = 240;
  const SIDEBAR_WIDTH_MAX = 440;

  function loadSidebarWidth(): number {
    try {
      const stored = Number(localStorage.getItem(SIDEBAR_WIDTH_KEY) ?? localStorage.getItem(LEGACY_SIDEBAR_WIDTH_KEY));
      if (
        Number.isFinite(stored) &&
        stored >= SIDEBAR_WIDTH_MIN &&
        stored <= SIDEBAR_WIDTH_MAX
      ) {
        localStorage.setItem(SIDEBAR_WIDTH_KEY, String(stored));
        return Math.round(stored);
      }
    } catch {
      // A webview with a blocked storage partition keeps the default width.
    }

    return SIDEBAR_WIDTH_DEFAULT;
  }

  let sidebarWidth = $state(loadSidebarWidth());
  let isResizingSidebar = $state(false);

  function persistSidebarWidth() {
    try {
      localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth));
    } catch {
      // The width is a convenience; failing to store it is not worth a warning.
    }
  }

  function startSidebarResize(event: PointerEvent) {
    event.preventDefault();
    isResizingSidebar = true;

    const startX = event.clientX;
    const startWidth = sidebarWidth;

    const handleMove = (moveEvent: PointerEvent) => {
      const next = startWidth + (moveEvent.clientX - startX);
      sidebarWidth = Math.min(
        SIDEBAR_WIDTH_MAX,
        Math.max(SIDEBAR_WIDTH_MIN, Math.round(next)),
      );
    };

    const handleUp = () => {
      isResizingSidebar = false;
      window.removeEventListener("pointermove", handleMove);
      window.removeEventListener("pointerup", handleUp);
      persistSidebarWidth();
    };

    window.addEventListener("pointermove", handleMove);
    window.addEventListener("pointerup", handleUp);
  }

  function resetSidebarWidth() {
    sidebarWidth = SIDEBAR_WIDTH_DEFAULT;
    persistSidebarWidth();
  }

  onMount(() => {
    try {
      const savedReadInvites = JSON.parse(localStorage.getItem("matrixgram.read-invites") ?? "[]");
      if (Array.isArray(savedReadInvites)) readInviteIds = savedReadInvites.filter((value): value is string => typeof value === "string");
      const savedDismissedInvites = JSON.parse(localStorage.getItem("matrixgram.dismissed-invites") ?? "[]");
      if (Array.isArray(savedDismissedInvites)) dismissedInviteIds = savedDismissedInvites.filter((value): value is string => typeof value === "string");
    } catch { /* ignore invalid local read-state */ }
    let unlisten = () => {};
    let unlistenDeepLink = () => {};

    void (async () => {
      unlisten = await subscribeToRoomUpdates({
        onRoomSyncStatus: (payload) => {
          if (!shellActive() || payload.userId !== get(shellCurrentUserId)) return;
          if (payload.error?.startsWith("Sliding Sync failed:") && get(shellChats).length > 0) {
            // Keep cached conversations visible and avoid flashing the fatal
            // page-level error for a background sync error that is retried.
            console.warn("Background Sliding Sync failed; retrying with cached chats available:", payload.error);
            return;
          }
          shellChatsLoading.set(false);
          shellErrorMessage.set(payload.error ?? "");
        },
        onRoomAdded: applyRoomUpsert,
        onRoomUpdated: applyRoomUpsert,
        onRoomRemoved: applyRoomRemoval,
        onSelectedRoomMessages: (_payload: MatrixSelectedRoomMessagesEvent) => {},
        onChatMessagesStream: () => {},
        onVerificationStateChanged: (payload) => {
          if (!payload.verified) return;
          const selectedRoomId = get(shellSelectedRoomId);
          if (selectedRoomId) {
            // Re-read the visible timeline after SAS so per-message sender trust
            // indicators are recomputed from the updated device verification state.
            void matrixTriggerRoomUpdate({ selectedRoomId, includeSelectedMessages: true });
          }
        },
        onProfileAvatarUpdated: (payload) => {
          // Matrix pushed an avatar change for this user; drop every cached
          // avatar so message headers re-resolve the new image.
          invalidateSenderAvatar(payload.userId);
          if (payload.userId === get(shellCurrentUserId)) {
            void matrixGetProfile().then((profile) => {
              currentUserAvatar = profile.avatarUrl ?? null;
            }).catch(() => {
              currentUserAvatar = null;
            });
          }
          void requestRefresh();
        },
      });

      if (!shellActive()) { unlisten(); return; }
      // Read the local session and room cache while deep links initialize.
      const shellLoad = loadShell();
      try {
        const currentUrls = await getCurrent();
        void handleDeepLinks(currentUrls);

        unlistenDeepLink = await onOpenUrl((urls) => {
          void handleDeepLinks(urls);
        });
      } catch (error) {
        console.error("Failed to initialize deep-link listener", error);
      }

      if (!shellActive()) { unlisten(); unlistenDeepLink(); return; }
      await shellLoad;
      if (!shellActive()) return;
      await requestRefresh();
      checkingAuth = false;
    })();

    return () => {
      shellDisposed = true;
      clearTimeout(chatSyncTimeout);
      unlisten();
      unlistenDeepLink();
    };
  });

  async function handleDeepLinks(urls: string[] | null) {
    if (!urls) return;

    for (const urlString of urls) {
      try {
        const parsed = new URL(urlString);
        
        if (parsed.protocol === "matrix:") {
          // matrix://join/#example:matrix.org
          if (parsed.hostname === "join" || parsed.hostname === "room" || parsed.hostname === "space") {
            const targetOrAlias = parsed.hash.length > 0 ? parsed.hash : parsed.pathname.slice(1);
            if (!targetOrAlias) continue;
            
            // Support `?via=example.com&via=another.com`
            const serverNames = parsed.searchParams.getAll("via");
            
            try {
              const res = await matrixJoinRoom(targetOrAlias, serverNames);
              await syncRoute("", res.roomId);
            } catch (error) {
              shellErrorMessage.set(error instanceof Error ? error.message : `Failed to join from link: ${targetOrAlias}`);
            }
          }
        }
      } catch {
        continue;
      }
    }
  }

  function applyRoomUpsert(chat: MatrixChatSummary) {
    if (!shellActive()) return;
    shellChatsLoading.set(false);
    shellChats.update((currentChats) => {
      const index = currentChats.findIndex((candidate) => candidate.roomId === chat.roomId);
      if (index >= 0) {
        return currentChats.map((candidate, candidateIndex) =>
          candidateIndex === index ? chat : candidate,
        );
      }

      return [...currentChats, chat];
    });

    void refreshChatNavigationAndRoute();
  }

  function applyRoomRemoval(payload: MatrixRoomRemovedEvent) {
    if (!shellActive()) return;
    shellChats.update((currentChats) =>
      currentChats.filter((chat) => chat.roomId !== payload.roomId),
    );

    if (get(shellSelectedRoomId) === payload.roomId) {
      shellSelectedRoomId.set("");
    }

    void refreshChatNavigationAndRoute();
  }

  async function loadShell() {
    shellErrorMessage.set("");
    if (!get(shellChats).length) {
      shellChatsLoading.set(true);
      clearTimeout(chatSyncTimeout);
      chatSyncTimeout = setTimeout(() => {
        if (!shellActive() || !get(shellChatsLoading) || get(shellChats).length) return;
        shellChatsLoading.set(false);
        shellErrorMessage.set("Room sync is taking longer than expected. Retry sync.");
      }, 45000);
    }

    try {
      const session = await matrixSessionStatus();
      if (!shellActive()) return;
      if (!session.authenticated) {
        await goto("/");
        return;
      }

      shellCurrentUserId.set(session.userId ?? "");

      const rooms = await matrixGetChats();
      if (!shellActive()) return;
      shellChats.update((current) => mergeInitialChatSnapshot(current, rooms));
      if (get(shellChats).length) shellChatsLoading.set(false);

      const queryRootSpaceId = page.url.searchParams.get("rootSpaceId") ?? "";
      const queryRoomId = page.url.searchParams.get("roomId") ?? "";
      const selectedRoomId = chooseInitialRoom(rooms, queryRoomId);
      shellSelectedRoomId.set(selectedRoomId);

      const selectedRootSpaceId = await refreshChatNavigation({
        rootSpaceId: queryRootSpaceId,
        selectedRoomId,
      });

      if (!page.url.pathname.startsWith("/settings")) {
        await syncRoute(selectedRootSpaceId, selectedRoomId);
      }

      void loadShellMetadata();
    } catch (error) {
      if (!shellActive()) return;
      shellChatsLoading.set(false);
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to load chats");
    }
  }

  async function loadShellMetadata() {
    const profileRequest = matrixGetProfile().then((profile) => {
      currentUserAvatar = profile.avatarUrl ?? null;
    }).catch(() => {
      currentUserAvatar = null;
    });
    const emojiRequest = matrixGetPickerAssets().then(({ customEmoji: pickerCustomEmoji }) => {
      shellPickerCustomEmoji.set(pickerCustomEmoji);
    }).catch(() => {
      shellPickerCustomEmoji.set([]);
    });
    await Promise.all([refreshRecoveryState(), profileRequest, emojiRequest]);
  }

  function chooseInitialRoom(rooms: MatrixChatSummary[], roomIdFromQuery: string): string {
    const hasQueryRoom = rooms.some(
      (room) => room.roomId === roomIdFromQuery && room.kind === "room",
    );
    if (hasQueryRoom) {
      return roomIdFromQuery;
    }

    return "";
  }

  async function requestRefresh() {
    if (!shellActive() || get(shellRefreshing)) {
      return;
    }

    shellRefreshing.set(true);
    shellErrorMessage.set("");

    try {
      await matrixTriggerRoomUpdate();
    } catch (error) {
      shellErrorMessage.set(
        error instanceof Error ? error.message : "Failed to trigger room refresh",
      );
    } finally {
      shellRefreshing.set(false);
    }
  }

  async function syncRoute(rootSpaceId: string, roomId: string, preserveView = true) {
    if (!shellActive()) return;
    const params = new URLSearchParams(page.url.searchParams);
    if (!preserveView) params.delete("view");
    if (rootSpaceId) {
      params.set("rootSpaceId", rootSpaceId);
    } else {
      params.delete("rootSpaceId");
    }

    if (roomId) {
      params.set("roomId", roomId);
    } else {
      params.delete("roomId");
    }

    const search = params.toString();
    const path = search ? `/chats?${search}` : "/chats";
    await goto(path, { replaceState: true, noScroll: true, keepFocus: true });
  }

  async function refreshChatNavigation(input?: MatrixGetChatNavigationRequest) {
    const response = await matrixGetChatNavigation(input);
    if (!shellActive()) return get(shellSelectedRootSpaceId);
    const nextRootSpaceId = response.selectedRootSpaceId ?? "";

    shellRootSpaces.set(response.rootSpaces);
    shellRootScopedRooms.set(response.rootScopedRooms);
    shellSelectedRootSpaceId.set(nextRootSpaceId);

    return nextRootSpaceId;
  }

  async function refreshChatNavigationAndRoute() {
    const roomId = get(shellSelectedRoomId);
    const rootSpaceId = await refreshChatNavigation({
      rootSpaceId: get(shellSelectedRootSpaceId),
      selectedRoomId: roomId,
    });

    if (!page.url.pathname.startsWith("/settings")) {
      await syncRoute(rootSpaceId, roomId);
    }
  }

  async function selectRootSpace(rootSpaceId: string) {
    shellSelectedRoomId.set("");
    const nextRootSpaceId = await refreshChatNavigation({ rootSpaceId });
    await syncRoute(nextRootSpaceId, "");
  }

  async function reorderRootSpaces(rootSpaceIds: string[]) {
    try {
      await matrixSetRootSpaceOrder({ rootSpaceIds });
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to save root space order");
      throw error;
    }

    try {
      await refreshChatNavigationAndRoute();
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to refresh root space order");
    }
  }

  async function selectRoom(roomId: string) {
    const chats = get(shellChats);
    const room = chats.find((candidate) => candidate.roomId === roomId);
    if (!room || room.kind !== "room") {
      return;
    }

    shellSelectedRoomId.set(roomId);
    const nextRootSpaceId = await refreshChatNavigation({
      rootSpaceId: get(shellSelectedRootSpaceId),
      selectedRoomId: roomId,
    });

    await syncRoute(nextRootSpaceId, roomId, false);
  }

  async function logout() {
    loggingOut = true;
    shellErrorMessage.set("");

    try {
      const error = await logoutSession();
      if (error) {
        shellErrorMessage.set(error);
      } else {
        senderAvatarCache.clear();
        roomImageCache.clear();
      }
    } finally {
      loggingOut = false;
    }
  }
</script>

<svelte:window onpointerdown={handleNotificationOutsidePointer} onkeydown={(event) => { if (event.key === "Escape") notificationsOpen = false; }} />

<main class="h-full overflow-hidden">
    {#if $shellErrorMessage}
      <p class="card p-3 text-sm preset-filled-error-500 mx-4 md:mx-6 mt-4">{$shellErrorMessage}</p>
    {/if}

    <!-- Two columns, always: the chat list stays on the left and the
         conversation on the right, however narrow the window gets. The
         divider between them can be dragged to resize the chat list. -->
    <div
      class="grid h-full min-h-0 grid-rows-[minmax(0,1fr)] overflow-hidden transition-[grid-template-columns] duration-200 ease-out"
      style={`grid-template-columns: ${isCommunityExplorer ? 76 : sidebarWidth}px minmax(0, 1fr);`}
    >
      <div class="app-sidebar relative flex min-h-0 min-w-0 flex-col bg-panel p-2">
        {#if isCommunityExplorer}
          <div class="mx-auto my-2 grid size-10 shrink-0 place-items-center overflow-hidden rounded-full border border-hairline bg-accent-soft text-sm font-semibold text-accent" title={$shellCurrentUserId} aria-label={$shellCurrentUserId}>
            {#if currentUserAvatar}
              <img src={currentUserAvatar} alt="" class="h-full w-full object-cover" onerror={() => (currentUserAvatar = null)} />
            {:else}
              {$shellCurrentUserId.replace(/^@/, "").charAt(0).toUpperCase() || "•"}
            {/if}
          </div>
        {:else}
          <AppHeader userId={$shellCurrentUserId} avatarUrl={currentUserAvatar} />
        {/if}

        <ChatList
          rooms={$shellChats}
          loading={$shellChatsLoading}
          error={$shellErrorMessage}
          onRetry={() => { shellChatsLoading.set(true); void loadShell().then(() => requestRefresh()); }}
          selectedRoomId={$shellSelectedRoomId}
          onSelectRoom={selectRoom}
          compact={isCommunityExplorer}
        />

        <div class="sidebar-footer relative flex gap-1">
          <button type="button" data-notification-trigger aria-label="Notifications" title="Notifications" class={`btn relative min-w-0 flex-1 justify-center px-0 ${notificationsOpen ? "bg-hover text-primary" : ""}`} aria-expanded={notificationsOpen} onclick={openNotifications}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" class="size-[18px]" aria-hidden="true"><path d="M18 9a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9"/><path d="M10 21h4"/></svg>
            {#if unreadInviteCount}<span class="absolute -right-1 -top-1 grid min-w-4 place-items-center rounded-full bg-accent px-1 text-[10px] leading-4 text-white">{unreadInviteCount}</span>{/if}
          </button>
          <a class="btn min-w-0 flex-1 justify-center px-0" aria-label="Settings" title={page.url.pathname.startsWith("/settings") ? "Back to chat" : "Settings"} href={page.url.pathname.startsWith("/settings") ? `/chats?${new URLSearchParams({ roomId: $shellSelectedRoomId, rootSpaceId: $shellSelectedRootSpaceId })}` : "/settings"}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" class="size-[18px]" aria-hidden="true"><path d="M9 3h6l1 3 3 1 2 5-2 5-3 1-1 3H9l-1-3-3-1-2-5 2-5 3-1z"/><circle cx="12" cy="12" r="3"/></svg>
          </a>
          {#if notificationsOpen}
            <section data-notification-popover class="absolute bottom-full left-0 z-50 mb-2 max-h-[min(70vh,520px)] w-[min(360px,calc(100vw-24px))] overflow-y-auto border border-hairline bg-panel shadow-xl" aria-label="Invitations">
              <div class="sticky top-0 flex items-center justify-between border-b border-hairline bg-panel px-3 py-2"><h2 class="text-sm font-semibold">Notifications</h2><div class="flex items-center gap-2">{#if inviteNotifications.some((invite) => readInviteIds.includes(invite.roomId))}<button type="button" class="cursor-pointer text-xs text-secondary hover:text-primary" onclick={clearReadNotifications}>Clear read</button>{/if}<button type="button" class="cursor-pointer px-1 text-secondary hover:text-primary" aria-label="Close notifications" onclick={() => notificationsOpen = false}>×</button></div></div>
              {#if inviteNotifications.length === 0}<p class="px-3 py-5 text-center text-sm text-secondary">No new invitations.</p>{:else}
                {#each inviteNotifications as invite (invite.roomId)}
                  <div class="flex items-center gap-1 border-b border-hairline pr-2"><button type="button" class="flex min-w-0 flex-1 cursor-pointer items-center gap-3 px-3 py-3 text-left hover:bg-panel-alt" onclick={() => void openInvite(invite)}><div class="grid size-9 shrink-0 place-items-center rounded-full bg-accent-soft text-sm font-semibold text-accent">{invite.displayName.slice(0, 1).toUpperCase()}</div><div class="min-w-0 flex-1"><p class="truncate text-sm font-medium text-primary">{invite.displayName}</p><p class="text-xs text-secondary">{invite.isDirect ? "Private chat invite · Open conversation" : "Room invite · View room details"}</p></div><span class="text-xs text-accent">Open</span></button><button type="button" class="btn btn-sm" title="Decline invitation" aria-label={`Decline ${invite.displayName}`} onclick={() => void declineInvite(invite)}>Decline</button></div>
                {/each}
              {/if}
            </section>
          {/if}
        </div>

        <!-- Resize handle: drag to widen or narrow the chat list, double
             click to go back to the default width. -->
        {#if !isCommunityExplorer}<div
          class="absolute inset-y-0 -right-1 z-30 w-2 cursor-col-resize"
          role="separator"
          aria-orientation="vertical"
          aria-label="Resize the chat list"
          title="Drag to resize, double click to reset"
          onpointerdown={startSidebarResize}
          ondblclick={resetSidebarWidth}
        >
          <div
            class="mx-auto h-full w-px transition-colors hover:bg-accent/50"
            class:bg-accent={isResizingSidebar}
          ></div>
        </div>{/if}
      </div>

      <section class="min-h-0 min-w-0 overflow-hidden">
        {@render children()}
      </section>
    </div>
  </main>

{#if selectedInvite}
  <div class="fixed inset-0 z-[80] grid place-items-center bg-black/55 p-4" role="presentation" onclick={(event) => { if (event.target === event.currentTarget) selectedInvite = null; }}>
    <section class="w-full max-w-md border border-hairline bg-panel shadow-2xl" role="dialog" aria-modal="true" aria-labelledby="invite-room-title">
      <header class="flex items-start gap-3 border-b border-hairline p-4">
        <div class="grid size-12 shrink-0 place-items-center rounded-full bg-accent-soft text-lg font-semibold text-accent">{selectedInvite.displayName.slice(0, 1).toUpperCase()}</div>
        <div class="min-w-0 flex-1"><p class="text-xs font-semibold uppercase tracking-wide text-accent">Room invitation</p><h2 id="invite-room-title" class="break-words text-lg font-semibold">{selectedInvite.displayName}</h2></div>
        <button type="button" class="cursor-pointer px-1 text-secondary hover:text-primary" aria-label="Close room information" onclick={() => selectedInvite = null}>×</button>
      </header>
      <div class="space-y-3 p-4">
        <p class="text-sm text-secondary">You have been invited to this room.</p>
        <div><h3 class="text-xs font-semibold uppercase tracking-wide text-muted">Room ID</h3><p class="mt-1 break-all font-mono text-xs text-primary">{selectedInvite.roomId}</p></div>
        <p class="text-sm text-secondary">{selectedInvite.joinedMembers.toLocaleString()} members · {selectedInvite.encrypted ? "Encrypted" : "Not encrypted"}</p>
      </div>
      <footer class="flex items-center justify-between gap-2 border-t border-hairline p-3">
        <button type="button" class="btn text-danger hover:bg-danger/10" disabled={acceptingInvite} onclick={() => selectedInvite && void declineInvite(selectedInvite)}>Decline</button>
        <div class="flex gap-2">
          <button type="button" class="btn" onclick={() => selectedInvite = null}>Later</button>
          <button type="button" class="btn btn-primary" disabled={acceptingInvite} onclick={() => selectedInvite && void acceptInvite(selectedInvite)}>{acceptingInvite ? "Joining…" : "Join room"}</button>
        </div>
      </footer>
    </section>
  </div>
{/if}

{#if spaceSettingsDialogRoomId}
  {@const dialogSpace = $shellChats.find((chat) => chat.roomId === spaceSettingsDialogRoomId)}
  {#if dialogSpace}
    <RoomSettingsDialog
      open={true}
      roomId={dialogSpace.roomId}
      roomName={dialogSpace.displayName}
      isSpace={dialogSpace.kind === "space"}
      onClose={() => (spaceSettingsDialogRoomId = null)}
    />
  {/if}
{/if}

<UserProfileDialog />
<IncomingVerificationDialog />
