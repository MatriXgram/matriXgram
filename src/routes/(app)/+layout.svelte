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
         conversation on the right, however narrow the window gets. The window
         itself has a minimum width, so the columns never collapse into a
         stack. -->
    <div
      class="grid h-full min-h-0 overflow-hidden grid-cols-[minmax(240px,300px)_minmax(0,1fr)]"
    >
      <div class="flex min-h-0 min-w-0 flex-col gap-2 border-r border-hairline bg-panel p-2">
        <AppHeader userId={$shellCurrentUserId} />

        <ChatList
          rooms={$shellChats}
          selectedRoomId={$shellSelectedRoomId}
          onSelectRoom={selectRoom}
        />

        <a class="btn preset-tonal w-full justify-center" href="/settings">Settings</a>
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
