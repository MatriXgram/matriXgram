<script lang="ts">
  import { matrixCopyImageToClipboard } from "$lib/chats/api";
  import type { PickerCustomEmoji } from "$lib/emoji/picker";
  import type {
    JumpToMessageHandler,
    ReplyToMessageHandler,
    RetryMessageHandler,
    TimelineMessage,
    ToggleReactionHandler,
  } from "../shared";
  import { toClockTime } from "../shared";
  import {
    MessageAvatar,
    MessageBody,
    MessageEncryptionState,
    MessageHeader,
    MessageReactions,
    MessageSendState,
    ReplyQuote,
  } from ".";

  interface Props {
    message: TimelineMessage;
    roomId: string;
    onRetry?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    onReplyToMessage?: ReplyToMessageHandler;
    onJumpToMessage?: JumpToMessageHandler;
    repliedMessage?: TimelineMessage;
    currentUserId?: string | null;
    pickerCustomEmoji?: PickerCustomEmoji[];
    isSending?: boolean;
    /// The previous message is from the same sender, so the bubble is drawn
    /// without an avatar and without a sender name.
    grouped?: boolean;
    /// The next message is from another sender, so this bubble carries the tail.
    lastInGroup?: boolean;
    /// Show the sender name above the bubble (group chats).
    showSenderName?: boolean;
  }

  let {
    message,
    roomId,
    onRetry,
    onToggleReaction,
    onReplyToMessage,
    onJumpToMessage,
    repliedMessage,
    currentUserId = null,
    pickerCustomEmoji = [],
    isSending = false,
    grouped = false,
    lastInGroup = true,
    showSenderName = false,
  }: Props = $props();

  let optionsButtonElement: HTMLButtonElement | null = $state(null);
  let optionsMenuElement: HTMLDivElement | null = $state(null);
  let isOptionsOpen = $state(false);
  let contextMenuPosition = $state<{ x: number; y: number } | null>(null);
  let optionsMode = $state<"reply" | "copy-image">("reply");

  const isOwn = $derived(Boolean(currentUserId) && message.sender === currentUserId);
  const primaryActionLabel = $derived(optionsMode === "copy-image" ? "Copy Image" : "Reply");

  $effect(() => {
    if (!isOptionsOpen) {
      return;
    }

    const onWindowPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }

      if (optionsButtonElement?.contains(target) || optionsMenuElement?.contains(target)) {
        return;
      }

      closeOptionsMenu();
    };

    const onWindowKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }

      closeOptionsMenu();
    };

    window.addEventListener("pointerdown", onWindowPointerDown, true);
    window.addEventListener("keydown", onWindowKeyDown);

    return () => {
      window.removeEventListener("pointerdown", onWindowPointerDown, true);
      window.removeEventListener("keydown", onWindowKeyDown);
    };
  });

  function closeOptionsMenu() {
    isOptionsOpen = false;
    contextMenuPosition = null;
    optionsMode = "reply";
  }

  function toggleOptionsMenu(event: MouseEvent) {
    event.stopPropagation();
    optionsMode = "reply";
    contextMenuPosition = null;
    isOptionsOpen = !isOptionsOpen;
  }

  function openOptionsMenuFromContext(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    optionsMode = "reply";
    contextMenuPosition = { x: event.clientX, y: event.clientY };
    isOptionsOpen = true;
  }

  function openImageOptionsMenuFromContext(event: MouseEvent) {
    optionsMode = "copy-image";
    contextMenuPosition = { x: event.clientX, y: event.clientY };
    isOptionsOpen = true;
  }

  async function copyImageToClipboard() {
    if (!message.imageUrl) {
      return;
    }

    try {
      await matrixCopyImageToClipboard({ imageUrl: message.imageUrl });
    } catch (error) {
      console.warn("Failed to copy image", error);
    }
  }

  function handlePrimaryAction() {
    const actionMode = optionsMode;
    closeOptionsMenu();

    if (actionMode === "copy-image") {
      void copyImageToClipboard();
      return;
    }

    onReplyToMessage?.(message);
  }
</script>

<li
  class="message-row relative flex w-full items-end gap-2 px-3 py-[1px]"
  class:justify-end={isOwn}
  data-message-event-id={message.eventId ?? undefined}
  oncontextmenu={openOptionsMenuFromContext}
>
  <div class="flex w-8 shrink-0 justify-center self-end">
    {#if !isOwn && !grouped}
      <MessageAvatar {roomId} sender={message.sender} />
    {/if}
  </div>

  <div class="flex min-w-0 max-w-[68%] flex-col" class:items-end={isOwn}>
    <MessageHeader {message} show={showSenderName && !isOwn && !grouped} />

    <div
      class="bubble {isOwn ? 'bubble-out' : 'bubble-in'}"
      class:bubble-group-last={grouped && !lastInGroup}
      class:bubble-tail={lastInGroup}
      class:message-mention={message.mentionsMe}
    >
      <ReplyQuote {message} {repliedMessage} {onJumpToMessage} />

      <MessageBody {message} {roomId} onImageContextMenu={openImageOptionsMenuFromContext} />

      {#if message.isDeleted}
        <p class="text-[13px] italic opacity-70">Message deleted</p>
      {/if}

      <div
        class="mt-0.5 flex items-center justify-end gap-1 text-[11px] leading-none opacity-70"
        class:opacity-100={isOwn}
      >
        {#if message.editedAt}
          <span class="italic">edited</span>
        {/if}
        {#if message.threadReplyCount > 0}
          <span class="font-semibold">{message.threadReplyCount} replies</span>
        {/if}
        <span>{toClockTime(message.timestamp)}</span>
        {#if isOwn}
          <span aria-label={message.isRead ? "Read" : "Sent"}>{message.isRead ? "✓✓" : "✓"}</span>
        {/if}
      </div>
    </div>

    <MessageReactions {message} {currentUserId} {pickerCustomEmoji} onToggleReaction={onToggleReaction} />
    <MessageSendState {message} {isSending} onRetry={onRetry} />
    <MessageEncryptionState {message} />
  </div>

  <button
    type="button"
    class="absolute top-0 z-10 rounded px-1.5 py-0.5 text-xs text-secondary opacity-0 transition hover:bg-hover hover:text-primary focus-visible:opacity-100 group-hover:opacity-100"
    class:left-10={isOwn}
    class:right-10={!isOwn}
    aria-label="Open message options"
    title="Message options"
    bind:this={optionsButtonElement}
    onclick={toggleOptionsMenu}
  >
    ...
  </button>

  {#if isOptionsOpen}
    <div
      class="z-20 min-w-28 overflow-hidden rounded-lg border border-hairline bg-panel shadow-lg"
      class:fixed={Boolean(contextMenuPosition)}
      class:absolute={!contextMenuPosition}
      class:right-2={!contextMenuPosition}
      class:top-10={!contextMenuPosition}
      style={contextMenuPosition ? `left: ${contextMenuPosition.x}px; top: ${contextMenuPosition.y}px;` : undefined}
      role="menu"
      bind:this={optionsMenuElement}
    >
      <button
        type="button"
        class="block w-full px-3 py-2 text-left text-sm hover:bg-hover disabled:cursor-not-allowed disabled:opacity-50"
        onclick={handlePrimaryAction}
        disabled={optionsMode === "copy-image" ? !message.imageUrl : !message.eventId}
        role="menuitem"
      >
        {primaryActionLabel}
      </button>
    </div>
  {/if}
</li>
