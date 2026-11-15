<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { cubicOut } from "svelte/easing";
  import { scale } from "svelte/transition";
  import type { Snippet } from "svelte";

  import {
    applyCustomEmojiConfig,
    ensureEmojiPickerLoaded,
    selectedEmojiToken,
  } from "$lib/emoji/picker";
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  type EmojiClickDetail = {
    unicode?: string;
    name?: string;
    emoji?: {
      shortcodes?: string[];
      name?: string;
    };
  };

  interface Props {
    /** Whether the picker is open. Bound so a right click can open it. */
    open?: boolean;
    /** Hide the trigger button; the picker is then only opened programmatically. */
    hideButton?: boolean;
    /** Screen position the picker is anchored to, in pixels. */
    anchor?: { x: number; y: number } | null;
    buttonText?: string;
    /// Icon shown on the trigger button instead of a text label.
    icon?: Snippet;
    buttonClass?: string;
    wrapperClass?: string;
    popoverClass?: string;
    pickerStyle?: string;
    hoverToOpen?: boolean;
    alignRight?: boolean;
    disabled?: boolean;
    pickerCustomEmoji?: PickerCustomEmoji[];
    onSelect?: (token: string) => void;
  }

  let {
    open = $bindable(false),
    hideButton = false,
    anchor = null,
    buttonText = "Emoji",
    icon,
    buttonClass = "btn btn-sm",
    wrapperClass = "relative",
    popoverClass = "rounded-xl border border-hairline bg-panel p-2 shadow-lg",
    pickerStyle = "width: min(280px, calc(100vw - 32px)); height: 280px; --emoji-size: 1.4rem; --category-emoji-size: 1rem; --num-columns: 7;",
    hoverToOpen = false,
    alignRight = false,
    disabled = false,
    pickerCustomEmoji = [],
    onSelect,
  }: Props = $props();

  let triggerElement = $state<HTMLButtonElement | null>(null);
  let pickerElement = $state<HTMLElement | null>(null);
  let pickerPopoverElement = $state<HTMLElement | null>(null);

  let popupPosition = $state({ x: 8, y: 8 });
  let popupTransformOrigin = $state("50% 100%");
  let hoverCloseTimer: ReturnType<typeof setTimeout>;

  function keepPickerOpen() {
    clearTimeout(hoverCloseTimer);
  }

  function openPickerOnHover() {
    keepPickerOpen();
    anchor = null;
    open = true;
  }

  function closePickerAfterPointerLeave() {
    clearTimeout(hoverCloseTimer);
    hoverCloseTimer = setTimeout(() => {
      open = false;
    }, 220);
  }

  onDestroy(() => clearTimeout(hoverCloseTimer));

  $effect(() => {
    const element = pickerPopoverElement;
    const trigger = triggerElement;
    const origin = anchor;
    if (!open || !element || (!origin && !trigger)) return;

    const reposition = () => {
      // Use layout dimensions instead of getBoundingClientRect(): the latter
      // includes the opening scale transition and briefly underestimates the
      // popup height, making it overlap the composer while expanding.
      const width = element.offsetWidth || element.getBoundingClientRect().width;
      const height = element.offsetHeight || element.getBoundingClientRect().height;
      const button = trigger?.getBoundingClientRect();
      const alignTarget = alignRight
        ? trigger?.closest(".message-composer")?.getBoundingClientRect()
        : button;
      const x = origin?.x ?? ((alignTarget?.right ?? button?.right ?? 8) - width);
      const y = origin?.y ?? ((button?.top ?? 8) - height - 8);
      const popupX = Math.max(8, Math.min(x, window.innerWidth - width - 8));
      const popupY = Math.max(8, Math.min(y, window.innerHeight - height - 8));
      popupPosition = { x: popupX, y: popupY };
      if (origin) {
        popupTransformOrigin = "0% 0%";
      } else if (button) {
        const sourceX = button.left + button.width / 2;
        const sourceY = button.top + button.height / 2;
        popupTransformOrigin = `${Math.max(0, Math.min(100, ((sourceX - popupX) / width) * 100))}% ${Math.max(0, Math.min(100, ((sourceY - popupY) / height) * 100))}%`;
      }
    };
    const observer = new ResizeObserver(reposition);
    observer.observe(element);
    window.addEventListener("resize", reposition);
    reposition();
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", reposition);
    };
  });

  onMount(() => {
    void ensureEmojiPickerLoaded().then(() => {
      if (!open || !pickerElement) return;
      applyCustomEmojiConfig(pickerElement as never, pickerCustomEmoji);
      applyPickerScrollStyle();
    });
  });

  function applyPickerScrollStyle() {
    const shadow = (pickerElement as HTMLElement | null)?.shadowRoot;
    if (!shadow || shadow.getElementById("matrix-picker-scroll-style")) return;
    const style = document.createElement("style");
    style.id = "matrix-picker-scroll-style";
    style.textContent = `
      .tabpanel, .favorites {
        scrollbar-width: none !important;
        -ms-overflow-style: none !important;
      }
      .tabpanel::-webkit-scrollbar, .favorites::-webkit-scrollbar {
        display: none !important;
        width: 0 !important;
        height: 0 !important;
      }
      .favorites {
        display: grid !important;
        grid-template-columns: repeat(var(--num-columns), minmax(0, 1fr)) !important;
        max-height: calc(3 * (var(--emoji-size) + 2 * var(--emoji-padding))) !important;
        overflow-x: hidden !important;
        overflow-y: auto !important;
        scrollbar-gutter: auto !important;
      }
    `;
    shadow.append(style);
  }

  $effect(() => {
    if (!open || !pickerElement) {
      return;
    }

    applyCustomEmojiConfig(pickerElement as never, pickerCustomEmoji);
    applyPickerScrollStyle();
  });

  $effect(() => {
    if (!open) {
      return;
    }

    const handleOutsidePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }

      if (pickerPopoverElement?.contains(target) || triggerElement?.contains(target)) {
        return;
      }

      open = false;
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        open = false;
      }
    };

    window.addEventListener("pointerdown", handleOutsidePointerDown, true);
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("pointerdown", handleOutsidePointerDown, true);
      window.removeEventListener("keydown", handleKeyDown);
    };
  });

  function handleEmojiClick(event: CustomEvent<EmojiClickDetail>) {
    const selected = selectedEmojiToken(event.detail);
    if (!selected) {
      return;
    }

    onSelect?.(selected);
    open = false;
  }
</script>

{#if !hideButton}
  <div class={wrapperClass} bind:this={pickerPopoverElement}>
    <button
      type="button"
      bind:this={triggerElement}
      aria-label="Choose emoji"
      title="Choose emoji"
      class={buttonClass}
      onpointerenter={hoverToOpen ? openPickerOnHover : undefined}
      onpointerleave={hoverToOpen ? closePickerAfterPointerLeave : undefined}
      onclick={() => {
        if (hoverToOpen) {
          openPickerOnHover();
        } else {
          anchor = null;
          open = !open;
        }
      }}
      {disabled}
    >
      {#if icon}
        {@render icon()}
      {:else}
        {buttonText}
      {/if}
    </button>
  </div>
{/if}

{#if open}
  <div
    class={"emoji-picker-popover fixed z-[60] " + popoverClass}
    style={`left: ${popupPosition.x}px; top: ${popupPosition.y}px; transform-origin: ${popupTransformOrigin}; width: min(296px, calc(100vw - 16px)); max-height: calc(100dvh - 16px); overflow-y: auto; overflow-x: hidden;`}
    bind:this={pickerPopoverElement}
    onpointerenter={hoverToOpen ? keepPickerOpen : undefined}
    onpointerleave={hoverToOpen ? closePickerAfterPointerLeave : undefined}
    transition:scale={{ duration: 180, start: 0.86, easing: cubicOut }}
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <emoji-picker
      bind:this={pickerElement}
      style={`${pickerStyle}; --background: var(--tg-panel); --border-color: var(--tg-border); --category-font-color: var(--tg-text); --input-font-color: var(--tg-text); --input-border-color: var(--tg-border); --input-placeholder-color: var(--tg-text-secondary); --button-active-background: var(--tg-active); --button-hover-background: var(--tg-hover); --indicator-color: var(--tg-accent); --outline-color: var(--tg-accent); --emoji-font-family: "Twemoji Country Flags", "Twemoji Mozilla", "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji", "EmojiOne Color", "Android Emoji", sans-serif;`}
      onemoji-click={handleEmojiClick}
    ></emoji-picker>
  </div>
{/if}

<style>
  :global(.emoji-picker-popover) {
    scrollbar-width: none;
    -ms-overflow-style: none;
  }
  :global(.emoji-picker-popover::-webkit-scrollbar) {
    display: none;
  }
</style>
