<script lang="ts">
  interface Props {
    imageUrl: string | null | undefined;
    displayName: string;
  }

  let { imageUrl, displayName }: Props = $props();
  let imageFailed = $state(false);
  let previousImageUrl = imageUrl;

  const avatarLabel = $derived(displayName.trim().charAt(0).toUpperCase() || "#");

  $effect(() => {
    const nextImageUrl = imageUrl;
    if (nextImageUrl !== previousImageUrl) {
      previousImageUrl = nextImageUrl;
      imageFailed = false;
    }
  });
</script>

<div
  class="room-avatar mt-0.5 h-8 w-8 shrink-0 rounded-full border border-hairline bg-panel-alt overflow-hidden grid place-items-center text-xs font-semibold text-accent"
>
  {#if imageUrl && !imageFailed}
    <img src={imageUrl} alt="" class="h-full w-full object-cover" onerror={() => (imageFailed = true)} />
  {:else}
    <span>{avatarLabel}</span>
  {/if}
</div>
