<script lang="ts">
  import { matrixGetUserAvatar } from "$lib/chats/api";
  import { avatarBackground, initialsForName } from "$lib/avatars";

  interface Props {
    roomId: string;
    userId: string;
    bubbleColor: string;
  }

  let { roomId, userId, bubbleColor }: Props = $props();
  let avatarUrl = $state<string | null>(null);
  const initials = $derived(initialsForName(userId.split(":")[0]?.replace(/^@/, "") ?? userId));

  $effect(() => {
    let cancelled = false;
    avatarUrl = null;
    void matrixGetUserAvatar(roomId, userId).then((url) => {
      if (!cancelled) avatarUrl = url;
    }).catch(() => {
      if (!cancelled) avatarUrl = null;
    });
    return () => { cancelled = true; };
  });
</script>

<span class="grid size-5 shrink-0 place-items-center overflow-hidden rounded-full border-2 text-[9px] font-bold text-white" style={`background:${avatarBackground(userId)};border-color:${bubbleColor}`} title={userId} aria-label={userId}>
  {#if avatarUrl}
    <img src={avatarUrl} alt="" class="size-full object-cover" loading="lazy" />
  {:else}
    {initials.slice(0, 1)}
  {/if}
</span>
