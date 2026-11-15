<script lang="ts">
  import { avatarBackground, initialsForName } from "$lib/avatars";

  interface Props {
    imageUrl: string | null | undefined;
    displayName: string;
    identity?: string;
  }

  let { imageUrl, displayName, identity = displayName }: Props = $props();
  let imageFailed = $state(false);
  let previousImageUrl = imageUrl;

  const avatarLabel = $derived(initialsForName(displayName));

  $effect(() => {
    const nextImageUrl = imageUrl;
    if (nextImageUrl !== previousImageUrl) {
      previousImageUrl = nextImageUrl;
      imageFailed = false;
    }
  });
</script>

<div
  style={imageUrl && !imageFailed
    ? "background: transparent; color: transparent; user-select: none;"
    : `background: ${avatarBackground(identity)}; color: #fff; user-select: none;`}
  class="room-avatar mt-0.5 h-8 w-8 shrink-0 rounded-full overflow-hidden grid place-items-center text-xs font-semibold"
>
  {#if imageUrl && !imageFailed}
    <img src={imageUrl} alt="" class="h-full w-full object-cover" onerror={() => (imageFailed = true)} />
  {:else}
    <span>{avatarLabel}</span>
  {/if}
</div>
