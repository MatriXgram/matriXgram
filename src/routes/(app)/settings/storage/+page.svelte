<script lang="ts">
  import { onMount } from "svelte";
  import {
    matrixClearMediaCache,
    matrixGetMediaCacheInfo,
    matrixSetMediaCacheLimit,
  } from "$lib/settings/api";
  import type { MatrixMediaCacheInfoResponse } from "$lib/settings/types";
  import { clearImageCaches } from "$lib/components/navigation/shared";

  const mib = 1024 * 1024;
  const cacheLimits = [
    { bytes: 1024 * mib, label: "1 GB" },
    { bytes: 2 * 1024 * mib, label: "2 GB" },
    { bytes: 5 * 1024 * mib, label: "5 GB" },
    { bytes: 10 * 1024 * mib, label: "10 GB" },
  ];

  let cacheInfo = $state<MatrixMediaCacheInfoResponse | null>(null);
  let selectedLimit = $state(2 * 1024 * mib);
  let loading = $state(true);
  let busy = $state(false);
  let errorMessage = $state("");
  let statusMessage = $state("");

  onMount(() => {
    void loadCacheInfo();
  });

  async function loadCacheInfo() {
    loading = true;
    errorMessage = "";
    try {
      cacheInfo = await matrixGetMediaCacheInfo();
      selectedLimit = cacheInfo.maxBytes;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    } finally {
      loading = false;
    }
  }

  async function updateCacheLimit(maxBytes: number) {
    selectedLimit = maxBytes;
    busy = true;
    errorMessage = "";
    statusMessage = "";
    try {
      await matrixSetMediaCacheLimit({ maxBytes });
      cacheInfo = await matrixGetMediaCacheInfo();
      statusMessage = "Cache limit saved for this device.";
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
      if (cacheInfo) selectedLimit = cacheInfo.maxBytes;
    } finally {
      busy = false;
    }
  }

  async function clearCache() {
    if (!window.confirm("Clear cached photos, videos, and thumbnails on this device? Chats and account data will be kept.")) {
      return;
    }

    busy = true;
    errorMessage = "";
    statusMessage = "";
    try {
      const result = await matrixClearMediaCache();
      clearImageCaches();
      cacheInfo = await matrixGetMediaCacheInfo();
      statusMessage = `Cleared ${formatBytes(result.clearedBytes)} of media cache.`;
    } catch (error) {
      errorMessage = error instanceof Error ? error.message : String(error);
    } finally {
      busy = false;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes < mib) return `${bytes} B`;
    const mb = bytes / mib;
    return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.round(mb)} MB`;
  }

  let imagePercent = $derived(cacheInfo ? Math.min(100, (cacheInfo.imageBytes / cacheInfo.maxBytes) * 100) : 0);
  let videoPercent = $derived(cacheInfo ? Math.min(100, (cacheInfo.videoBytes / cacheInfo.maxBytes) * 100) : 0);
  let otherPercent = $derived(cacheInfo ? Math.min(100, (cacheInfo.otherBytes / cacheInfo.maxBytes) * 100) : 0);
</script>

<section class="space-y-4">
  <header>
    <p class="text-xs font-bold uppercase tracking-[0.2em] text-accent">Settings</p>
    <h2 class="h5">Storage</h2>
    <p class="text-sm text-secondary">Manage media stored in this app on this device.</p>
  </header>

  {#if errorMessage}
    <p class="card p-3 text-sm preset-filled-error-500" role="alert">{errorMessage}</p>
  {/if}
  {#if statusMessage}
    <p class="text-sm text-secondary" role="status">{statusMessage}</p>
  {/if}

  <section class="card space-y-4 border border-hairline bg-panel p-4" aria-labelledby="media-cache-title">
    <div>
      <h3 id="media-cache-title" class="h6">Media cache</h3>
      <p class="mt-1 text-sm text-secondary">Photos, videos, avatars, and thumbnails cached for faster loading.</p>
    </div>

    <div class="space-y-2" aria-live="polite">
      <div class="flex items-center justify-between gap-3 text-sm">
        <span class="text-secondary">{loading ? "Checking storage…" : "Used on this device"}</span>
        <span class="font-medium tabular-nums text-primary">
          {#if cacheInfo}{formatBytes(cacheInfo.usedBytes)} of {formatBytes(cacheInfo.maxBytes)}{:else}—{/if}
        </span>
      </div>
      <div class="flex h-2 overflow-hidden rounded-full bg-panel-alt" aria-label="Media cache usage by type">
        <div class="h-full bg-sky-400 transition-[width]" style={`width: ${imagePercent}%`} title={`Images: ${formatBytes(cacheInfo?.imageBytes ?? 0)}`}></div>
        <div class="h-full bg-violet-400 transition-[width]" style={`width: ${videoPercent}%`} title={`Videos: ${formatBytes(cacheInfo?.videoBytes ?? 0)}`}></div>
        <div class="h-full bg-surface-500-500 transition-[width]" style={`width: ${otherPercent}%`} title={`Other: ${formatBytes(cacheInfo?.otherBytes ?? 0)}`}></div>
      </div>
      <div class="flex flex-wrap gap-x-5 gap-y-1 text-xs text-secondary">
        <span class="inline-flex items-center gap-1.5"><span class="size-2 rounded-full bg-sky-400"></span>Images <strong class="font-medium text-primary">{formatBytes(cacheInfo?.imageBytes ?? 0)}</strong></span>
        <span class="inline-flex items-center gap-1.5"><span class="size-2 rounded-full bg-violet-400"></span>Videos <strong class="font-medium text-primary">{formatBytes(cacheInfo?.videoBytes ?? 0)}</strong></span>
        <span class="inline-flex items-center gap-1.5"><span class="size-2 rounded-full bg-surface-500-500"></span>Other <strong class="font-medium text-primary">{formatBytes(cacheInfo?.otherBytes ?? 0)}</strong></span>
      </div>
    </div>

    <label class="flex flex-wrap items-center justify-between gap-3 text-sm">
      <span>
        <span class="block font-medium text-primary">Maximum cache size</span>
        <span class="text-xs text-secondary">Older cached media is removed automatically when the limit is reached.</span>
      </span>
      <select
        class="min-w-28 cursor-pointer border border-hairline bg-panel px-3 py-2 text-sm text-primary outline-none focus:border-accent"
        value={selectedLimit}
        disabled={loading || busy}
        onchange={(event) => void updateCacheLimit(Number((event.currentTarget as HTMLSelectElement).value))}
      >
        {#each cacheLimits as option (option.bytes)}
          <option value={option.bytes}>{option.label}</option>
        {/each}
      </select>
    </label>

    <div class="flex flex-wrap items-center justify-between gap-3 border-t border-hairline pt-3">
      <p class="max-w-lg text-xs text-secondary">Clearing media cache does not remove chats, login details, or messages.</p>
      <button type="button" class="btn preset-tonal" disabled={loading || busy || !cacheInfo?.usedBytes} onclick={() => void clearCache()}>
        {#if busy}Working…{:else}Clear media cache{/if}
      </button>
    </div>
  </section>
</section>