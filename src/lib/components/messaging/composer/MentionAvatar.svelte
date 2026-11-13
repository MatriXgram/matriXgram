<script lang="ts">
  import { matrixGetUserAvatar } from "$lib/chats/api";
  import { avatarChangeEpoch, senderAvatarCache } from "$lib/components/navigation/shared/image-cache.svelte";

  interface Props {
    roomId: string;
    userId: string;
    displayName: string;
  }

  let { roomId, userId, displayName }: Props = $props();
  let imageUrl = $state<string | null>(null);

  $effect(() => {
    void avatarChangeEpoch.current;
    imageUrl = null;
    const key = `${roomId}\u0000${userId}`;
    const cachedUrl = senderAvatarCache.getCached(key);
    if (cachedUrl !== undefined) {
      imageUrl = cachedUrl;
      return;
    }
    void senderAvatarCache.getOrLoad(key, () => matrixGetUserAvatar(roomId, userId)).then((url) => {
      if (key === `${roomId}\u0000${userId}`) imageUrl = url;
    });
  });
</script>

<span class="grid size-8 shrink-0 place-items-center overflow-hidden rounded-full bg-panel-alt text-xs font-semibold text-secondary" aria-hidden="true">
  {#if imageUrl}
    <img src={imageUrl} alt="" class="h-full w-full object-cover" loading="lazy" decoding="async" />
  {:else}
    {displayName.charAt(0).toUpperCase() || "?"}
  {/if}
</span>
