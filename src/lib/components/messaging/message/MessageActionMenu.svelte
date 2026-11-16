<script lang="ts">
  import type { TimelineMessage } from "../shared";

  interface Props {
    message: TimelineMessage;
    /// Screen position of the menu, in pixels.
    position: { x: number; y: number; originX?: "left" | "right"; originY?: "top" | "bottom" } | null;
    onReply?: (message: TimelineMessage) => void;
    onOpenThread?: (message: TimelineMessage) => void;
    onForward?: (message: TimelineMessage) => void;
    onCopy?: (message: TimelineMessage) => void;
    onCopyImage?: (message: TimelineMessage) => void;
    onDelete?: (message: TimelineMessage) => void;
    isPinned?: boolean;
    onTogglePinned?: (message: TimelineMessage, pinned: boolean) => void;
    onClose?: () => void;
  }

  let { message, position, onReply, onOpenThread, onForward, onCopy, onCopyImage, onDelete, isPinned = false, onTogglePinned, onClose }: Props = $props();

  $effect(() => {
    if (!position) {
      return;
    }

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Element && target.closest("[data-action-menu]")) {
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
    data-action-menu
    class="menu-pop-in fixed z-50 min-w-44 overflow-hidden rounded-md border border-hairline bg-panel/85 py-1 shadow-lg backdrop-blur-md"
    style={`left: ${position.x}px; top: ${position.y}px; transform-origin: ${position.originY ?? "bottom"} ${position.originX ?? "left"};`}
    role="menu"
    aria-label="Message actions"
  >
    <button
      type="button"
      class="flex w-full items-center gap-2 px-2 py-1.5 text-left text-[13px] hover:bg-hover"
      role="menuitem"
      onclick={() => {
        onReply?.(message);
        onClose?.();
      }}
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        class="size-3.5 shrink-0 text-secondary"
        aria-hidden="true"
      >
        <polyline points="9 17 4 12 9 7" />
        <path d="M20 18v-2a4 4 0 0 0-4-4H4" />
      </svg>
      Reply
    </button>

    {#if message.eventId && onOpenThread}
      <button
        type="button"
        class="flex w-full items-center gap-2 px-2 py-1.5 text-left text-[13px] hover:bg-hover"
        role="menuitem"
        onclick={() => {
          onOpenThread?.(message);
          onClose?.();
        }}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="size-3.5 shrink-0 text-secondary" aria-hidden="true">
          <path d="M21 11.5a8.4 8.4 0 0 1-.9 3.8 8.5 8.5 0 0 1-7.6 4.7 8.4 8.4 0 0 1-3.8-.9L3 21l1.9-5.7a8.4 8.4 0 0 1-.9-3.8A8.5 8.5 0 0 1 8.7 3.9a8.4 8.4 0 0 1 3.8-.9h.5a8.5 8.5 0 0 1 8 8z" />
        </svg>
        {message.threadReplyCount > 0 ? "Open thread" : "Start thread"}
      </button>
    {/if}

    <button
      type="button"
      class="flex w-full items-center gap-2 px-2 py-1.5 text-left text-[13px] hover:bg-hover"
      role="menuitem"
      onclick={() => {
        onForward?.(message);
        onClose?.();
      }}
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        class="size-3.5 shrink-0 text-secondary"
        aria-hidden="true"
      >
        <line x1="19" y1="5" x2="5" y2="19" />
        <polyline points="9 19 5 19 5 15" />
      </svg>
      Forward
    </button>

    <button
      type="button"
      class="flex w-full items-center gap-2 px-2 py-1.5 text-left text-[13px] hover:bg-hover disabled:cursor-not-allowed disabled:opacity-50"
      role="menuitem"
      disabled={!message.body}
      onclick={() => {
        onCopy?.(message);
        onClose?.();
      }}
    >
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2"
        stroke-linecap="round"
        stroke-linejoin="round"
        class="size-3.5 shrink-0 text-secondary"
        aria-hidden="true"
      >
        <rect x="9" y="9" width="13" height="13" rx="2" ry="2" />
        <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
      </svg>
      Copy text
    </button>

    {#if message.imageUrl && onCopyImage}
      <button
        type="button"
        class="flex w-full items-center gap-2 px-2 py-1.5 text-left text-[13px] hover:bg-hover"
        role="menuitem"
        onclick={() => {
          onCopyImage?.(message);
          onClose?.();
        }}
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="size-3.5 shrink-0 text-secondary"
          aria-hidden="true"
        >
          <rect x="3" y="3" width="18" height="18" rx="2" ry="2" />
          <circle cx="8.5" cy="8.5" r="1.5" />
          <polyline points="21 15 16 10 5 21" />
        </svg>
        Copy image
      </button>
    {/if}
    {#if message.eventId && onTogglePinned}
      <button
        type="button"
        class="flex w-full items-center gap-2 px-2 py-1.5 text-left text-[13px] hover:bg-hover"
        role="menuitem"
        onclick={() => {
          onTogglePinned?.(message, !isPinned);
          onClose?.();
        }}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="size-3.5 shrink-0 text-secondary" aria-hidden="true"><path d="M9 3h6l-.5 5 3 3v2H6v-2l3-3-.5-5Z" /><path d="M12 13v8" /></svg>
        {isPinned ? "Unpin message" : "Pin message"}
      </button>
    {/if}
    {#if onDelete}
      <div class="my-1 border-t border-hairline"></div>
      <button
        type="button"
        class="flex w-full items-center gap-2 px-2 py-1.5 text-left text-[13px] text-danger hover:bg-danger/10"
        role="menuitem"
        onclick={() => {
          onDelete?.(message);
          onClose?.();
        }}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="size-3.5 shrink-0" aria-hidden="true">
          <path d="M3 6h18"/><path d="M8 6V4h8v2"/><path d="m19 6-1 14H6L5 6"/><path d="M10 11v5M14 11v5"/>
        </svg>
        Delete for everyone
      </button>
    {/if}
  </div>
{/if}
