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
    MessageActionMenu,
    MessageAvatar,
    MessageBody,
    MessageEncryptionState,
    MessageHeader,
    MessageReactions,
    MessageSendState,
    ReactionQuickBar,
    ReplyQuote,
  } from ".";
  import { EmojiPickerPopover } from "../composer-emoji";

  interface Props {
    message: TimelineMessage;
    roomId: string;
    onRetry?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    onReplyToMessage?: ReplyToMessageHandler;
    onForward?: (message: TimelineMessage) => void;
    onCopy?: (message: TimelineMessage) => void;
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
    /// Show the sender name at the top of the bubble (group chats).
    showSenderName?: boolean;
  }

  let {
    message,
    roomId,
    onRetry,
    onToggleReaction,
    onReplyToMessage,
    onForward,
    onCopy,
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

  /// The quick reaction bar, shown on a right click.
  let reactionBarPosition = $state<{ x: number; y: number } | null>(null);
  let reactionPickerOpen = $state(false);
  let reactionPickerAnchor = $state<{ x: number; y: number } | null>(null);

  /// The action list (reply, forward, copy) below the reaction bar. Its
  /// position is the single source of truth for "the menu is open": the
  /// outside click handler and the button toggle both go through it.
  let actionMenuPosition = $state<{ x: number; y: number } | null>(null);

  const isOwn = $derived(Boolean(currentUserId) && message.sender === currentUserId);
  $effect(() => {
    if (!actionMenuPosition && !reactionBarPosition) {
      return;
    }

    const onWindowPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }

      if (optionsButtonElement?.contains(target)) {
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
    actionMenuPosition = null;
    reactionBarPosition = null;
    reactionPickerOpen = false;
  }


  /// A right click opens the quick reaction bar and the action list below it,
  /// the way Telegram does.
  function openReactionBar(event: MouseEvent) {
    event.preventDefault();
    event.stopPropagation();

    reactionPickerOpen = false;
    reactionPickerAnchor = null;
    // Both popups sit above the cursor, the action list right below the
    // reaction bar.
    const barWidth = 250;
    const barHeight = 44;
    const menuWidth = 150;
    const menuHeight = 132;

    const x = Math.max(
      8,
      Math.min(event.clientX - barWidth / 2, window.innerWidth - barWidth - 8),
    );

    reactionBarPosition = {
      x,
      y: Math.max(8, event.clientY - barHeight - menuHeight - 12),
    };

    actionMenuPosition = {
      x: Math.max(
        8,
        Math.min(event.clientX - menuWidth / 2, window.innerWidth - menuWidth - 8),
      ),
      y: Math.max(8, event.clientY - menuHeight - 8),
    };
  }

  /// The "..." button opens the same action list, anchored under the button.
  function toggleActionMenu(event: MouseEvent) {
    event.stopPropagation();

    const button = optionsButtonElement;
    const rect = button?.getBoundingClientRect();

    reactionBarPosition = null;
    reactionPickerOpen = false;

    actionMenuPosition = actionMenuPosition
      ? null
      : {
          x: Math.max(8, (rect?.right ?? 0) - 150),
          y: Math.max(8, (rect?.bottom ?? 0) + 6),
        };

  }

  function openFullReactionPicker() {
    const position = reactionBarPosition ?? { x: 120, y: 120 };
    reactionBarPosition = null;
    reactionPickerAnchor = { x: position.x, y: position.y };
    reactionPickerOpen = true;
  }

  function toggleReactionFromBar(_message: TimelineMessage, key: string) {
    if (!message.eventId) {
      return;
    }

    onToggleReaction?.(message, key);
  }

  /// A right click on the image itself opens the same bar and action list.
  function openImageOptionsMenuFromContext(event: MouseEvent) {
    openReactionBar(event);
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

  /// Put the plain text of a message on the clipboard.
  async function copyText(target: TimelineMessage) {
    if (!target.body) {
      return;
    }

    try {
      await navigator.clipboard.writeText(target.body);
    } catch (error) {
      console.warn("Failed to copy text", error);
    }
  }
</script>

<li
  class="message-row relative flex w-full items-end gap-2 px-3 py-[1px]"
  class:justify-end={isOwn}
  data-message-event-id={message.eventId ?? undefined}
  oncontextmenu={openReactionBar}
>
  <div class="flex w-8 shrink-0 justify-center self-end">
    {#if !isOwn && !grouped}
      <MessageAvatar {roomId} sender={message.sender} />
    {/if}
  </div>

  <div class="flex min-w-0 max-w-[68%] flex-col" class:items-end={isOwn}>
    <div
      class="bubble {isOwn ? 'bubble-out' : 'bubble-in'}"
      class:bubble-group-last={grouped && !lastInGroup}
      class:bubble-tail={lastInGroup}
      class:message-mention={message.mentionsMe}
    >
      <MessageHeader {message} show={showSenderName && !isOwn && !grouped} />

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
        {#if message.sendState === "sending"}
          <svg
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="size-3"
            aria-label="Sending"
          >
            <title>Sending</title>
            <circle cx="12" cy="12" r="10" />
            <polyline points="12 6 12 12 16 14" />
          </svg>
        {:else if isOwn}
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
    onclick={toggleActionMenu}
  >
    ...
  </button>

  <ReactionQuickBar
    {message}
    position={reactionBarPosition}
    onToggleReaction={toggleReactionFromBar}
    onExpand={openFullReactionPicker}
    onClose={() => (reactionBarPosition = null)}
  />

  <MessageActionMenu
    {message}
    position={actionMenuPosition}
    onReply={onReplyToMessage}
    onForward={onForward}
    onCopy={(target) => (onCopy ? onCopy(target) : copyText(target))}
    onCopyImage={message.imageUrl ? copyImageToClipboard : undefined}
    onClose={() => (actionMenuPosition = null)}
  />

  <EmojiPickerPopover
    bind:open={reactionPickerOpen}
    hideButton
    anchor={reactionPickerAnchor}
    {pickerCustomEmoji}
    onSelect={(token) => toggleReactionFromBar(message, token)}
  />
</li>
