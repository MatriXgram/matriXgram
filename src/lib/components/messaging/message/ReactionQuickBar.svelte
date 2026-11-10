<script lang="ts">
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    /// Screen position of the bar, in pixels.
    position: { x: number; y: number } | null;
    onToggleReaction?: (message: TimelineMessage, key: string) => void;
    /// Open the full emoji table.
    onExpand?: () => void;
    onClose?: () => void;
  }

  let { message, position, onToggleReaction, onExpand, onClose }: Props = $props();

  /// The reactions Telegram shows first. The full table is one click away.
  const QUICK_REACTIONS = ["👍", "❤️", "😂", "😮", "😢", "🙏"];

  $effect(() => {
    if (!position) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Element && target.closest("[data-reaction-bar]")) {
        return;
      }

      onClose?.();
    };

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose?.();
      }
    };

    window.addEventListener("pointerdown", handlePointerDown, true);
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("pointerdown", handlePointerDown, true);
      window.removeEventListener("keydown", handleKeyDown);
    };
  });
</script>

{#if position}
  <div
    data-reaction-bar
    class="fixed z-50 flex items-center gap-0.5 rounded-full border border-hairline bg-panel px-1.5 py-1 shadow-lg"
    style={`left: ${position.x}px; top: ${position.y}px;`}
    role="menu"
    aria-label="React with an emoji"
  >
    {#each QUICK_REACTIONS as emoji (emoji)}
      <button
        type="button"
        class="grid size-8 place-items-center rounded-full text-lg leading-none transition hover:bg-hover"
        title={emoji}
        aria-label={`React with ${emoji}`}
        role="menuitem"
        onclick={() => {
          onToggleReaction?.(message, emoji);
          onClose?.();
        }}
      >
        {emoji}
      </button>
    {/each}

    <span class="mx-0.5 h-5 w-px bg-hairline"></span>

    <button
      type="button"
      class="grid size-8 place-items-center rounded-full text-secondary transition hover:bg-hover hover:text-primary"
      title="More reactions"
      aria-label="More reactions"
      role="menuitem"
      onclick={() => onExpand?.()}
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        class="size-4"
        aria-hidden="true"
      >
        <circle cx="12" cy="12" r="10" />
        <path d="M8 14s1.5 2 4 2 4-2 4-2" />
        <line x1="9" y1="9" x2="9.01" y2="9" />
        <line x1="15" y1="9" x2="15.01" y2="9" />
      </svg>
    </button>
  </div>
{/if}
