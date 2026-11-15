<script lang="ts">
  import { loadUserProfile, openUserProfile } from "$lib/chats/user-profile";
  import MessageAvatar from "./MessageAvatar.svelte";
  let { roomId, userId, timestamp = null }: { roomId: string; userId: string; timestamp?: number | null } = $props();
  let name = $state("");
  const fallback = $derived(userId.replace(/^@/, "").split(":")[0]);
  const reactionDate = $derived(timestamp ? new Intl.DateTimeFormat(undefined, {
    day: "2-digit", month: "2-digit",
  }).format(timestamp) : null);
  const reactionTime = $derived(timestamp ? new Intl.DateTimeFormat(undefined, {
    hour: "2-digit", minute: "2-digit",
  }).format(timestamp) : null);
  $effect(() => {
    const room = roomId, user = userId;
    let cancelled = false;
    name = "";
    void loadUserProfile(room, user).then(profile => { if (!cancelled) name = profile.displayName || ""; }).catch(() => {});
    return () => { cancelled = true; };
  });
</script>
<div class="flex items-center gap-2 rounded-lg px-2 py-0.5 transition-colors hover:bg-accent-soft">
  <MessageAvatar {roomId} sender={userId} displayName={name || fallback} onOpenProfile={(avatarUrl) => openUserProfile({ roomId, userId, displayName: name, avatarUrl })} />
  <button type="button" class="flex min-w-0 flex-1 cursor-pointer items-center gap-2 rounded text-left focus-visible:ring-1 focus-visible:ring-accent"
    onclick={() => openUserProfile({ roomId, userId, displayName: name })}>
    <span class="min-w-0 flex-1">
      <span class="block truncate text-[13px] font-medium leading-4">{name || fallback}</span>
      <span class="block truncate text-[10px] leading-3 text-secondary">{userId}</span>
    </span>
    {#if reactionTime}<time class="flex shrink-0 flex-col items-end text-[10px] leading-3 text-secondary" datetime={new Date(timestamp!).toISOString()} title={new Date(timestamp!).toLocaleString()}><span>{reactionDate}</span><span>{reactionTime}</span></time>{/if}
  </button>
</div>
