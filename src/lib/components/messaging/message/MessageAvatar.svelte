<script lang="ts">
import { matrixGetUserAvatar } from "$lib/chats/api";
import { avatarChangeEpoch, senderAvatarCache } from "$lib/components/navigation/shared/image-cache.svelte";

  interface Props {
    roomId: string;
    sender: string;
    onOpenProfile?: (avatarUrl: string | null) => void;
  }

  let { roomId, sender, onOpenProfile }: Props = $props();

  let lazyImageUrl = $state<string | null>(null);

  const avatarLabel = $derived.by(() => {
    const localPart = sender.includes(":") ? sender.slice(0, sender.indexOf(":")) : sender;
    const normalized = localPart.replace(/^@/, "").trim();
    return normalized.charAt(0).toUpperCase() || "#";
  });

  const avatarEpoch = $derived(avatarChangeEpoch.current);

  $effect(() => {
    // Re-run when an avatar change lands: the epoch bumps on
    // PROFILE_AVATAR_UPDATED after the cache has been invalidated.
    void avatarEpoch;
    lazyImageUrl = null;

    const senderId = sender;
    const cacheKey = `${roomId}\u0000${senderId}`;
    void senderAvatarCache
      .getOrLoad(cacheKey, () => matrixGetUserAvatar(roomId, senderId))
      .then((imageUrl) => {
        if (sender === senderId) {
          lazyImageUrl = imageUrl;
        }
      });
  });
</script>

<button
  type="button"
  onclick={() => onOpenProfile?.(lazyImageUrl)}
  aria-label={`View profile of ${sender}`}
  class="cursor-pointer transition hover:ring-2 hover:ring-accent focus-visible:ring-2 focus-visible:ring-accent h-8 w-8 shrink-0 overflow-hidden rounded-full bg-surface-200-800 grid place-items-center text-xs font-semibold text-surface-800-200"
  title={sender}
>
  {#if lazyImageUrl}
    <img src={lazyImageUrl} alt="" class="h-full w-full object-cover" loading="lazy" decoding="async" />
  {:else}
    <span>{avatarLabel}</span>
  {/if}
</button>
