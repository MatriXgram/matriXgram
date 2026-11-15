<script lang="ts">
  import { matrixResolveMediaUrl } from "$lib/chats/api";

  interface Props {
    mediaUrl: string;
    sourceUrl: string;
    label: string;
  }

  let { mediaUrl, sourceUrl, label }: Props = $props();
  let resolvedUrl = $state<string | null>(null);
  let failed = $state(false);
  let usingSource = $state(false);
  let generation = 0;

  async function resolve(url: string, request: number) {
    try {
      const resolved = await matrixResolveMediaUrl(url);
      if (request !== generation) return;
      if (!resolved) throw new Error("Media resolver returned no image");
      resolvedUrl = resolved;
    } catch (error) {
      if (request !== generation) return;
      console.warn("Failed to resolve custom reaction image", url, error);
      handleImageError();
    }
  }

  function handleImageError() {
    resolvedUrl = null;
    // Picker URLs can point to files removed by cache eviction. Resolve the
    // original MXC again rather than permanently hiding an otherwise valid emoji.
    if (!usingSource && sourceUrl.startsWith("mxc://") && sourceUrl !== mediaUrl) {
      usingSource = true;
      void resolve(sourceUrl, generation);
    } else {
      failed = true;
    }
  }

  $effect(() => {
    const url = mediaUrl;
    // A new source must reset a previous failure, even if the picker URL is unchanged.
    void sourceUrl;
    const request = ++generation;
    resolvedUrl = null;
    failed = false;
    usingSource = false;
    void resolve(url, request);
    return () => { generation++; };
  });
</script>

{#if resolvedUrl}
  <img
    src={resolvedUrl}
    alt={label}
    title={label}
    class="inline-block size-3.5 object-contain align-text-bottom"
    onerror={handleImageError}
  />
{:else}
  <span
    class="inline-block size-3.5 text-center text-[13px] leading-none align-text-bottom"
    class:animate-pulse={!failed}
    role="img"
    aria-label={failed ? `${label} (image unavailable)` : `${label} (loading)`}
    title={failed ? `${label} (image unavailable)` : label}
  >{failed ? "◇" : "⋯"}</span>
{/if}
