<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";

  import { matrixLoadDesktopTheme } from "$lib/settings/api";
  import {
    FONT_SIZE_DEFAULT,
    FONT_SIZE_MAX,
    FONT_SIZE_MIN,
    currentMode,
    resetThemeSettings,
    themeSettings,
    updateThemeSettings,
    type ThemeMode,
  } from "$lib/theme";
  import { THEME_PRESETS, findPreset } from "$lib/themes/presets";

  let importError = $state("");
  let importing = $state(false);

  const settings = $derived($themeSettings);

  /// The colour the CSS variable currently resolves to, for the swatches.
  function readColor(variable: string, fallback: string): string {
    const value = getComputedStyle(document.documentElement)
      .getPropertyValue(variable)
      .trim();
    return value || fallback;
  }

  const accentValue = $derived.by(() => { void settings; return readColor("--tg-accent", "#3390ec"); });
  const replyBackgroundValue = $derived.by(() => {
    void settings;
    return settings.replyBackground ?? (currentMode() === "dark" ? "#34475e" : "#e8f2ff");
  });
  const replyOutgoingBackgroundValue = $derived.by(() => {
    void settings;
    return settings.replyOutgoingBackground ?? (currentMode() === "dark" ? "#354f6d" : "#dceaff");
  });
  const backgroundValue = $derived.by(() => { void settings; return readColor("--tg-panel", "#ffffff"); });
  const textValue = $derived.by(() => { void settings; return readColor("--tg-text", "#0f1419"); });

  const fontSize = $derived(settings.fontSize ?? FONT_SIZE_DEFAULT);

  function setMode(mode: ThemeMode) {
    updateThemeSettings({ mode, palette: null, paletteName: null });
  }

  function applyPreset(presetId: string) {
    const preset = findPreset(presetId);
    if (!preset) {
      return;
    }

    updateThemeSettings({
      palette: preset.colors,
      paletteName: preset.name,
      wallpaper: preset.wallpaper ?? null,
    });
  }

  function onAccentInput(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    updateThemeSettings({ accent: value });
  }

  function onReplyBackgroundInput(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    updateThemeSettings({ replyBackground: value });
  }

  function onReplyOutgoingBackgroundInput(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    updateThemeSettings({ replyOutgoingBackground: value });
  }

  function onBackgroundInput(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    updateThemeSettings({ background: value });
  }

  function onTextInput(event: Event) {
    const value = (event.currentTarget as HTMLInputElement).value;
    updateThemeSettings({ text: value });
  }

  function onFontSizeInput(event: Event) {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    updateThemeSettings({ fontSize: value });
  }

  async function importTheme() {
    importError = "";
    importing = true;

    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Telegram Desktop theme", extensions: ["tdesktop-theme"] }],
      });

      if (typeof selected !== "string") {
        return;
      }

      const theme = await matrixLoadDesktopTheme({ path: selected });
      const name = selected.split(/[\\/]/).pop()?.replace(/\.tdesktop-theme$/i, "") ?? "Telegram theme";

      updateThemeSettings({
        palette: theme.colors,
        paletteName: name,
        wallpaper: theme.backgroundPath ? convertFileSrc(theme.backgroundPath) : null,
      });
    } catch (error) {
      importError = error instanceof Error ? error.message : "Failed to import the theme";
    } finally {
      importing = false;
    }
  }

  async function chooseWallpaper() {
    importError = "";

    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Image", extensions: ["jpg", "jpeg", "png", "webp"] }],
      });

      if (typeof selected !== "string") {
        return;
      }

      updateThemeSettings({ wallpaper: convertFileSrc(selected) });
    } catch (error) {
      importError = error instanceof Error ? error.message : "Failed to load the wallpaper";
    }
  }
</script>

<section class="card space-y-4 border border-hairline bg-panel p-4">
  <header class="flex items-baseline justify-between gap-2">
    <h3 class="h6">Appearance</h3>
    {#if settings.paletteName}
      <span class="rounded-full bg-accent-soft px-2 py-0.5 text-[11px] font-medium text-accent">
        {settings.paletteName}
      </span>
    {/if}
  </header>

  <div class="space-y-1">
    <p class="text-sm font-medium">Base theme</p>
    <div class="flex gap-2">
      <button
        type="button"
        class="btn btn-sm"
        class:btn-primary={!settings.palette && settings.mode === "light"}
        onclick={() => setMode("light")}
      >
        Light
      </button>
      <button
        type="button"
        class="btn btn-sm"
        class:btn-primary={!settings.palette && settings.mode === "dark"}
        onclick={() => setMode("dark")}
      >
        Dark
      </button>
      <button
        type="button"
        class="btn btn-sm"
        class:btn-primary={!settings.palette && settings.mode === "system"}
        onclick={() => setMode("system")}
      >
        System
      </button>
    </div>
    <p class="text-[11px] text-secondary">
      Currently: {settings.palette ? settings.paletteName : currentMode()}
    </p>
  </div>

  {#if THEME_PRESETS.length > 0}
    <div class="space-y-1">
      <p class="text-sm font-medium">Telegram themes</p>
      <div class="flex flex-wrap gap-2">
        {#each THEME_PRESETS as preset (preset.id)}
          <button
            type="button"
            class="btn btn-sm"
            class:btn-primary={settings.paletteName === preset.name}
            title={preset.description}
            onclick={() => applyPreset(preset.id)}
          >
            {preset.name}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <div class="space-y-1">
    <p class="text-sm font-medium">Import a theme</p>
    <button type="button" class="btn btn-sm" disabled={importing} onclick={importTheme}>
      {importing ? "Importing…" : "Open .tdesktop-theme…"}
    </button>
    <p class="text-[11px] text-secondary">
      Telegram Desktop theme files work as they are: the sidebar, the bubbles and the accents are
      taken from the file.
    </p>
  </div>

  <div class="space-y-1">
    <p class="text-sm font-medium">Accent colour</p>
    <div class="flex items-center gap-3">
      <input
        type="color"
        class="h-9 w-12 cursor-pointer rounded border border-hairline bg-transparent"
        value={accentValue}
        oninput={onAccentInput}
        aria-label="Accent colour"
      />
      <span class="font-mono text-xs text-secondary">{accentValue}</span>
      {#if settings.accent}
        <button type="button" class="btn btn-xs" onclick={() => updateThemeSettings({ accent: null })}>
          Reset
        </button>
      {/if}
    </div>
  </div>

  <div class="space-y-1">
    <p class="text-sm font-medium">Incoming reply block background</p>
    <p class="text-[11px] text-secondary">Set the background for quoted messages in received bubbles.</p>
    <div class="flex items-center gap-3">
      <input
        type="color"
        class="h-9 w-12 cursor-pointer rounded border border-hairline bg-transparent"
        value={replyBackgroundValue}
        oninput={onReplyBackgroundInput}
        aria-label="Reply block background colour"
      />
      <span class="font-mono text-xs text-secondary">{replyBackgroundValue}</span>
      {#if settings.replyBackground}
        <button type="button" class="btn btn-xs" onclick={() => updateThemeSettings({ replyBackground: null })}>
          Reset
        </button>
      {/if}
    </div>
  </div>

  <div class="space-y-1">
    <p class="text-sm font-medium">Outgoing reply block background</p>
    <p class="text-[11px] text-secondary">Set the background for quoted messages in your sent bubbles.</p>
    <div class="flex items-center gap-3">
      <input
        type="color"
        class="h-9 w-12 cursor-pointer rounded border border-hairline bg-transparent"
        value={replyOutgoingBackgroundValue}
        oninput={onReplyOutgoingBackgroundInput}
        aria-label="Outgoing reply block background colour"
      />
      <span class="font-mono text-xs text-secondary">{replyOutgoingBackgroundValue}</span>
      {#if settings.replyOutgoingBackground}
        <button type="button" class="btn btn-xs" onclick={() => updateThemeSettings({ replyOutgoingBackground: null })}>
          Reset
        </button>
      {/if}
    </div>
  </div>

  <div class="space-y-1">
    <p class="text-sm font-medium">Background</p>
    <div class="flex items-center gap-3">
      <input
        type="color"
        class="h-9 w-12 cursor-pointer rounded border border-hairline bg-transparent"
        value={backgroundValue}
        oninput={onBackgroundInput}
        aria-label="Background colour"
      />
      <span class="font-mono text-xs text-secondary">{backgroundValue}</span>
      {#if settings.background}
        <button
          type="button"
          class="btn btn-xs"
          onclick={() => updateThemeSettings({ background: null })}
        >
          Reset
        </button>
      {/if}
    </div>
  </div>

  <div class="space-y-1">
    <p class="text-sm font-medium">Text colour</p>
    <div class="flex items-center gap-3">
      <input
        type="color"
        class="h-9 w-12 cursor-pointer rounded border border-hairline bg-transparent"
        value={textValue}
        oninput={onTextInput}
        aria-label="Text colour"
      />
      <span class="font-mono text-xs text-secondary">{textValue}</span>
      {#if settings.text}
        <button type="button" class="btn btn-xs" onclick={() => updateThemeSettings({ text: null })}>
          Reset
        </button>
      {/if}
    </div>
  </div>

  <div class="space-y-1">
    <p class="text-sm font-medium">Font size</p>
    <div class="flex items-center gap-3">
      <input
        type="range"
        min={FONT_SIZE_MIN}
        max={FONT_SIZE_MAX}
        step="1"
        value={fontSize}
        oninput={onFontSizeInput}
        class="flex-1 accent-[var(--tg-accent)]"
        aria-label="Font size"
      />
      <span class="w-10 text-right font-mono text-xs text-secondary">{fontSize}px</span>
    </div>
    <p class="text-[11px] text-secondary">
      The preview uses the current size: <span style={`font-size: ${fontSize}px`}>The quick brown fox</span>
    </p>
  </div>

  <div class="space-y-1">
    <p class="text-sm font-medium">Chat wallpaper</p>
    <div class="flex items-center gap-2">
      <button type="button" class="btn btn-sm" onclick={chooseWallpaper}>Choose image…</button>
      {#if settings.wallpaper}
        <button
          type="button"
          class="btn btn-sm"
          onclick={() => updateThemeSettings({ wallpaper: null })}
        >
          Remove
        </button>
      {/if}
    </div>
  </div>

  {#if importError}
    <p class="rounded bg-danger-soft px-3 py-2 text-xs text-danger">{importError}</p>
  {/if}

  <div class="border-t border-hairline pt-3">
    <button type="button" class="btn btn-sm" onclick={resetThemeSettings}>Reset appearance</button>
  </div>
</section>
