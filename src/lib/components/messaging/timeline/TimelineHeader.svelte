<script lang="ts">
  import { matrixGetRoomImage } from "$lib/chats/api";
  import { getRoomMembersCached } from "$lib/chats/room-members-cache";
  import type { MatrixRoomMember } from "$lib/chats/types";
  import { openUserProfile } from "$lib/chats/user-profile";
  import { RoomAvatar, roomImageCache, roomImageChangeEpoch } from "$lib/components/navigation/shared";
  import MessageAvatar from "../message/MessageAvatar.svelte";
  import { streamStatusLabel } from "../shared";

  interface Props {
    roomName: string;
    roomEncrypted: boolean;
    loadingMessages: boolean;
    activeLoadKind: string | null;
    streamMessageCount: number;
    roomId?: string | null;
    currentUserId?: string | null;
    isDirect?: boolean;
    onOpenSettings?: (roomId: string) => void;
  }

  let {
    roomName,
    roomEncrypted,
    loadingMessages,
    activeLoadKind,
    streamMessageCount,
    roomId,
    currentUserId = null,
    isDirect = false,
    onOpenSettings,
  }: Props = $props();
  let roomImage = $state<string | null>(null);
  let roomMembers = $state<MatrixRoomMember[]>([]);
  let showAllBotMembers = $state(false);
  let botPopupCloseTimer: ReturnType<typeof setTimeout> | undefined;
  const botMembers = $derived.by(() => roomMembers.flatMap((member) => {
    const name = member.displayName.trim();
    const identity = `${name} ${member.userId}`;
    const botMatch = /\(bot\)|\[bot\]|_bot/i.test(identity);
    const crawlerMatch = /\(crawler\)|\[crawler\]|_crawler/i.test(identity);
    if (!botMatch && !crawlerMatch) return [];
    return [{
      label: name || member.userId,
      userId: member.userId,
      kind: crawlerMatch ? "crawler" : "bot",
    }];
  }));
  const latestBotMember = $derived(botMembers.at(-1) ?? null);
  const directMember = $derived(isDirect ? roomMembers.find((member) => member.userId !== currentUserId) ?? null : null);

  function openBotMembersPopup() {
    if (botPopupCloseTimer) clearTimeout(botPopupCloseTimer);
    botPopupCloseTimer = undefined;
    showAllBotMembers = true;
  }

  function scheduleBotMembersPopupClose() {
    if (botPopupCloseTimer) clearTimeout(botPopupCloseTimer);
    botPopupCloseTimer = setTimeout(() => {
      showAllBotMembers = false;
      botPopupCloseTimer = undefined;
    }, 180);
  }

  function showBotProfile(member: (typeof botMembers)[number], avatarUrl?: string | null) {
    if (botPopupCloseTimer) clearTimeout(botPopupCloseTimer);
    botPopupCloseTimer = undefined;
    showAllBotMembers = false;
    openUserProfile({ roomId: roomId ?? "", userId: member.userId, displayName: member.label, avatarUrl });
  }

  $effect(() => {
    void roomImageChangeEpoch.current;
    const id = roomId;
    roomImage = id ? roomImageCache.getCached(id) ?? null : null;
    if (!id) return;
    let cancelled = false;
    void roomImageCache.getOrLoad(id, () => matrixGetRoomImage(id)).then((image) => {
      if (!cancelled) roomImage = image;
    });
    return () => { cancelled = true; };
  });

  $effect(() => {
    const id = roomId;
    roomMembers = [];
    if (botPopupCloseTimer) clearTimeout(botPopupCloseTimer);
    botPopupCloseTimer = undefined;
    showAllBotMembers = false;
    if (!id) return;
    let cancelled = false;
    void getRoomMembersCached(id).then((members) => {
      if (!cancelled) roomMembers = members;
    }).catch(() => {
      if (!cancelled) roomMembers = [];
    });
    return () => { cancelled = true; };
  });
</script>

<header class="conversation-header flex items-center justify-between border-b border-hairline bg-panel">
  <div class="flex min-w-0 items-center gap-3">
    {#if directMember && roomId}
      <MessageAvatar roomId={roomId} sender={directMember.userId} displayName={directMember.displayName || roomName}
        onOpenProfile={(avatarUrl) => openUserProfile({ roomId, userId: directMember.userId, displayName: directMember.displayName || roomName, avatarUrl })} />
    {:else}
      <RoomAvatar imageUrl={roomImage} displayName={roomName} identity={roomId ?? roomName} />
    {/if}
    <div class="min-w-0">
    <h2 class="conversation-title truncate leading-tight">{roomName}</h2>
    <div class="flex min-w-0 items-center gap-1 text-[11px] text-secondary">
      <span class="inline-flex shrink-0 items-center gap-1">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" class="size-3" aria-hidden="true"><rect x="5" y="10" width="14" height="11" rx="3"/><path d="M8 10V7a4 4 0 0 1 8 0v3"/></svg>
        {roomEncrypted ? "End-to-end encrypted" : "Not encrypted"}
      </span>
      {#if latestBotMember}
        <span class="min-w-0 truncate">· <button type="button" class="cursor-pointer underline decoration-transparent hover:text-primary hover:decoration-current" onclick={() => showBotProfile(latestBotMember)}>{latestBotMember.label}</button> is following the conversation.</span>
        {#if botMembers.length > 1}
          <div
            class="relative shrink-0"
            onpointerenter={openBotMembersPopup}
            onpointerleave={scheduleBotMembersPopupClose}
            onfocusin={openBotMembersPopup}
          >
            <button type="button" class="cursor-pointer whitespace-nowrap text-accent underline decoration-transparent hover:decoration-current" aria-expanded={showAllBotMembers}>+{botMembers.length - 1} more</button>
            {#if showAllBotMembers}
              <div class="absolute top-full right-0 z-50 mt-1 max-h-72 w-80 max-w-[calc(100vw-2rem)] overflow-y-auto rounded-lg border border-hairline bg-panel text-left text-primary shadow-xl" role="list" aria-label="Bots and crawlers in this room" onpointerenter={openBotMembersPopup} onpointerleave={scheduleBotMembersPopupClose}>
                {#each botMembers as member (`${member.kind}:${member.userId}`)}
                  <div class="flex min-w-0 items-center gap-2 border-b border-hairline px-2 py-2 last:border-b-0">
                    <MessageAvatar roomId={roomId ?? ""} sender={member.userId} displayName={member.label} onOpenProfile={(avatarUrl) => showBotProfile(member, avatarUrl)} />
                    <button type="button" class="min-w-0 flex-1 cursor-pointer text-left" onclick={() => showBotProfile(member)}>
                      <span class="block truncate text-sm">{member.label}</span>
                      <span class="block truncate font-mono text-[11px] text-secondary">{member.userId}</span>
                    </button>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      {/if}
      {#if loadingMessages && activeLoadKind}
        <span class="shrink-0 text-accent">· {streamStatusLabel(loadingMessages, activeLoadKind, streamMessageCount)}</span>
      {/if}
    </div>
    </div>
  </div>

  <div class="flex shrink-0 items-center gap-2">
    {#if roomId && onOpenSettings}
      <button
        type="button"
        class="icon-button"
        title="Room settings"
        aria-label="Open room settings"
        onclick={() => onOpenSettings?.(roomId)}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" class="size-5" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 11v6m0-10v.5"/></svg>
      </button>
    {/if}


  </div>
</header>
