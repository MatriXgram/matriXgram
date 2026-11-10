<script lang="ts">
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { get } from "svelte/store";
  import { onMount } from "svelte";
  import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";

  import { matrixSessionStatus } from "$lib/auth/api";
  import {
    matrixGetChatNavigation,
    matrixGetChats,
    matrixGetPickerAssets,
    matrixSetRootSpaceOrder,
    matrixTriggerRoomUpdate,
    matrixJoinRoom,
  } from "$lib/chats/api";
  import { subscribeToRoomUpdates } from "$lib/chats/realtime";
  import {
    logout as logoutSession,
    refreshRecoveryState,
    shellChats,
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

  let { children } = $props();

  let checkingAuth = $state(true);
  let loggingOut = $state(false);
  let spaceSettingsDialogRoomId = $state<string | null>(null);

  // The chat list can be resized by dragging the divider next to it.
  const SIDEBAR_WIDTH_KEY = "matrix-telegram.sidebarWidth";
  const SIDEBAR_WIDTH_DEFAULT = 260;
  const SIDEBAR_WIDTH_MIN = 200;
  const SIDEBAR_WIDTH_MAX = 440;

  function loadSidebarWidth(): number {
    try {
      const stored = Number(localStorage.getItem(SIDEBAR_WIDTH_KEY));
      if (
        Number.isFinite(stored) &&
        stored >= SIDEBAR_WIDTH_MIN &&
        stored <= SIDEBAR_WIDTH_MAX
      ) {
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
    let unlisten = () => {};
    let unlistenDeepLink = () => {};

    void (async () => {
      unlisten = await subscribeToRoomUpdates({
        onRoomAdded: applyRoomUpsert,
        onRoomUpdated: applyRoomUpsert,
        onRoomRemoved: applyRoomRemoval,
        onSelectedRoomMessages: (_payload: MatrixSelectedRoomMessagesEvent) => {},
        onChatMessagesStream: () => {},
        onProfileAvatarUpdated: (payload) => {
          // Matrix pushed an avatar change for this user; drop every cached
          // avatar so message headers re-resolve the new image.
          invalidateSenderAvatar(payload.userId);
          void requestRefresh();
        },
      });

      try {
        const currentUrls = await getCurrent();
        void handleDeepLinks(currentUrls);

        unlistenDeepLink = await onOpenUrl((urls) => {
          void handleDeepLinks(urls);
        });
      } catch (error) {
        console.error("Failed to initialize deep-link listener", error);
      }

      await loadShell();
      await requestRefresh();
      checkingAuth = false;
    })();

    return () => {
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

    try {
      const session = await matrixSessionStatus();
      if (!session.authenticated) {
        await goto("/");
        return;
      }

      shellCurrentUserId.set(session.userId ?? "");

      const rooms = await matrixGetChats();
      shellChats.set(rooms);

      const queryRootSpaceId = page.url.searchParams.get("rootSpaceId") ?? "";
      const queryRoomId = page.url.searchParams.get("roomId") ?? "";
      const selectedRoomId = chooseInitialRoom(rooms, queryRoomId);
      shellSelectedRoomId.set(selectedRoomId);

      const selectedRootSpaceId = await refreshChatNavigation({
        rootSpaceId: queryRootSpaceId,
        selectedRoomId,
      });

      await syncRoute(selectedRootSpaceId, selectedRoomId);

      void loadShellMetadata();
    } catch (error) {
      shellErrorMessage.set(error instanceof Error ? error.message : "Failed to load chats");
    }
  }

  async function loadShellMetadata() {
    await refreshRecoveryState();

    try {
      const { customEmoji: pickerCustomEmoji } = await matrixGetPickerAssets();
      shellPickerCustomEmoji.set(pickerCustomEmoji);
    } catch {
      shellPickerCustomEmoji.set([]);
    }
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
    if (get(shellRefreshing)) {
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

  async function syncRoute(rootSpaceId: string, roomId: string) {
    const params = new URLSearchParams(page.url.searchParams);
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

    await syncRoute(rootSpaceId, roomId);
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

    await syncRoute(nextRootSpaceId, roomId);
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
        await goto("/");
      }
    } finally {
      loggingOut = false;
    }
  }
</script>

<main class="h-screen overflow-hidden">
    {#if $shellErrorMessage}
      <p class="card p-3 text-sm preset-filled-error-500 mx-4 md:mx-6 mt-4">{$shellErrorMessage}</p>
    {/if}

    <!-- Two columns, always: the chat list stays on the left and the
         conversation on the right, however narrow the window gets. The
         divider between them can be dragged to resize the chat list. -->
    <div
      class="grid h-full min-h-0 overflow-hidden"
      style={`grid-template-columns: ${sidebarWidth}px minmax(0, 1fr);`}
    >
      <div class="relative flex min-h-0 min-w-0 flex-col gap-2 bg-panel p-2">
        <AppHeader userId={$shellCurrentUserId} />

        <ChatList
          rooms={$shellChats}
          selectedRoomId={$shellSelectedRoomId}
          onSelectRoom={selectRoom}
        />

        <a class="btn preset-tonal w-full justify-center" href="/settings">Settings</a>

        <!-- Resize handle: drag to widen or narrow the chat list, double
             click to go back to the default width. -->
        <div
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
        </div>
      </div>

      <section class="min-h-0 min-w-0 overflow-hidden">
        {@render children()}
      </section>
    </div>
  </main>

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
