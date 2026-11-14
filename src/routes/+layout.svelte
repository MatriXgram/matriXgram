<script lang="ts">
  import "../app.css";
  import { initialiseTheme, watchSystemTheme } from "$lib/theme";
  import { isTauri } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  let { children } = $props();
  let desktopApp = $state(false);
  let appWindow = $state<ReturnType<typeof getCurrentWindow> | null>(null);

  onMount(() => {
    desktopApp = isTauri();
    if (desktopApp) appWindow = getCurrentWindow();
  });

  function handleTitlebarDoubleClick(event: MouseEvent) {
    if (event.target instanceof Element && event.target.closest("button")) return;
    if (appWindow) void appWindow.toggleMaximize();
  }

  // The palette is applied before the first paint by the inline script in
  // app.html; this restores the user's overrides and the loaded theme.
  initialiseTheme();
  watchSystemTheme();
</script>

<div class="flex h-dvh min-h-0 flex-col overflow-hidden bg-app-bg text-primary">
  {#if desktopApp}
    <header
      class="app-titlebar flex h-8 shrink-0 items-center justify-between select-none"
      data-tauri-drag-region
      ondblclick={handleTitlebarDoubleClick}
    >
      <div class="flex h-full min-w-0 flex-1 items-center gap-2 px-3" data-tauri-drag-region>
        <img src="/favicon.svg" alt="" class="size-5" data-tauri-drag-region />
        <span class="truncate text-xs font-medium" data-tauri-drag-region>MatriXgram</span>
      </div>
      <div class="app-titlebar-controls flex h-full shrink-0 items-stretch">
        <button type="button" aria-label="Minimize" title="Minimize" onclick={() => { if (appWindow) void appWindow.minimize(); }}>
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M3 8h10" /></svg>
        </button>
        <button type="button" aria-label="Maximize or restore" title="Maximize or restore" onclick={() => { if (appWindow) void appWindow.toggleMaximize(); }}>
          <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="3.5" y="3.5" width="9" height="9" rx="0.5" /></svg>
        </button>
        <button class="app-titlebar-close" type="button" aria-label="Close" title="Close" onclick={() => { if (appWindow) void appWindow.close(); }}>
          <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4 4 8 8M12 4l-8 8" /></svg>
        </button>
      </div>
    </header>
  {/if}
  <div class="min-h-0 flex-1">
    {@render children()}
  </div>
</div>
