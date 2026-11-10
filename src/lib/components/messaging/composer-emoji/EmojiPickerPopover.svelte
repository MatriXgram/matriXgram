<script lang="ts">
  import { onMount } from "svelte";
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
    popoverClass?: string;
    pickerStyle?: string;
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
    popoverClass = "rounded-xl border border-hairline bg-panel p-2 shadow-lg",
    pickerStyle = "width: 100%; height: 22rem; --emoji-size: 2rem; --category-emoji-size: 1.15rem;",
    disabled = false,
    pickerCustomEmoji = [],
    onSelect,
  }: Props = $props();

  let pickerElement = $state<HTMLElement | null>(null);
  let pickerPopoverElement = $state<HTMLElement | null>(null);

  onMount(() => {
    void ensureEmojiPickerLoaded();
  });

  $effect(() => {
    if (!open || !pickerElement) {
      return;
    }

    applyCustomEmojiConfig(pickerElement as never, pickerCustomEmoji);
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

      if (pickerPopoverElement?.contains(target)) {
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
  <div class="relative" bind:this={pickerPopoverElement}>
    <button
      type="button"
      class={buttonClass}
      onclick={() => {
        open = !open;
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
    class={anchor ? "fixed z-40 " + popoverClass : "absolute bottom-full left-0 z-20 mb-2 " + popoverClass}
    style={anchor ? `left: ${anchor.x}px; top: ${anchor.y}px;` : undefined}
    bind:this={pickerPopoverElement}
  >
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <emoji-picker
      bind:this={pickerElement}
      style={pickerStyle}
      onemoji-click={handleEmojiClick}
    ></emoji-picker>
  </div>
{/if}
