<script lang="ts">
  import { openUserProfile } from "$lib/chats/user-profile";
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
    imageMessages: TimelineMessage[];
    roomId: string;
    onRetry?: RetryMessageHandler;
    onRemoveFailedMessage?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    onReplyToMessage?: ReplyToMessageHandler;
    onForward?: (message: TimelineMessage) => void;
    onDeleteMessage?: (message: TimelineMessage) => void;
    isPinned?: boolean;
    onTogglePinned?: (message: TimelineMessage, pinned: boolean) => void;
    onCopy?: (message: TimelineMessage) => void;
    onJumpToMessage?: JumpToMessageHandler;
    repliedMessage?: TimelineMessage;
    currentUserId?: string | null;
    pickerCustomEmoji?: PickerCustomEmoji[];
    isSending?: boolean;
    /// The sender name is omitted when continuing a sender group.
    grouped?: boolean;
    /// The last message in a sender group gets the small incoming bubble tail.
    groupEnd?: boolean;
    /// Show the sender name at the top of the bubble (group chats).
    showSenderName?: boolean;
  }

  let {
    message,
    imageMessages,
    roomId,
    onRetry,
    onRemoveFailedMessage,
    onToggleReaction,
    onReplyToMessage,
    onForward,
    onDeleteMessage,
    isPinned = false,
    onTogglePinned,
    onCopy,
    onJumpToMessage,
    repliedMessage,
    currentUserId = null,
    pickerCustomEmoji = [],
    isSending = false,
    grouped = false,
    groupEnd = false,
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
  let actionMenuPosition = $state<{ x: number; y: number; originX?: "left" | "right"; originY?: "top" | "bottom" } | null>(null);

  function showProfile(avatarUrl?: string | null) { openUserProfile({ roomId, userId: message.sender, displayName: message.senderDisplayName, avatarUrl }); }

  const senderIdentity = $derived(message.sender);
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

      // Keep popups mounted until their click handlers have run.
      if (
        optionsButtonElement?.contains(target) ||
        (target instanceof Element &&
          target.closest("[data-action-menu], [data-reaction-bar]"))
      ) {
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
    // Keep the action list anchored to the pointer and the reaction bar close
    // above it. Flip the list if there is not enough room above the pointer.
    const barWidth = 250;
    const barHeight = 44;
    const menuWidth = 176;
    const canDelete = isOwn && !message.isDeleted && message.sendState !== "sending" && Boolean(message.eventId);
    const menuHeight = 8 + ((message.imageUrl ? 4 : 3) + (canDelete ? 1 : 0)) * 27;
    // Anchor the pointer to the menu's bottom-left edge and expand upward.
    const menuX = event.clientX + menuWidth + 8 <= window.innerWidth
      ? event.clientX + 2
      : Math.max(8, event.clientX - menuWidth - 2);
    const opensAbove = event.clientY >= menuHeight + 8;
    const menuY = opensAbove
      ? event.clientY - menuHeight - 2
      : event.clientY + 2;
    const barX = Math.max(8, Math.min(menuX + (menuWidth - barWidth) / 2, window.innerWidth - barWidth - 8));

    reactionBarPosition = {
      x: barX,
      y: Math.max(8, menuY - barHeight - 4),
    };

    actionMenuPosition = {
      x: menuX,
      y: menuY,
      originX: menuX >= event.clientX ? "left" : "right",
      originY: opensAbove ? "bottom" : "top",
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
          x: Math.max(8, (rect?.right ?? 0) - 176),
          y: Math.max(8, (rect?.bottom ?? 0) + 6),
          originX: "right",
          originY: "top",
        };

  }

  function openFullReactionPicker() {
    const position = reactionBarPosition ?? { x: 120, y: 120 };
    actionMenuPosition = null;
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
  class="message-row group relative flex w-full items-end gap-2 px-3 py-[1px]"
  class:justify-end={isOwn}
  data-message-event-id={message.eventId ?? undefined}
  oncontextmenu={openReactionBar}
>


  <div class="flex min-w-0 max-w-[78%] flex-col" class:items-end={isOwn}>
    <div
      class="bubble {isOwn ? 'bubble-out' : 'bubble-in'}"
      class:bubble-media={message.messageType === "m.image"}
      class:message-tail-in={groupEnd && !isOwn}
      class:message-tail-out={groupEnd && isOwn}
      class:message-mention={message.mentionsMe}
    >
  <button
    type="button"
    class="message-options absolute top-1.5 z-10 rounded px-1.5 py-0.5 text-xs text-secondary opacity-0 transition hover:bg-hover hover:text-primary focus-visible:opacity-100 group-hover:opacity-100"
    aria-label="Open message options"
    title="Message options"
    bind:this={optionsButtonElement}
    onclick={toggleActionMenu}
  >
    ...
  </button>

      <MessageHeader {message} onOpenProfile={() => showProfile()} show={showSenderName && !isOwn && !grouped} />

      {#if message.isDeleted}
        <p class="px-2 py-1 text-[13px] italic opacity-70">This message was deleted</p>
      {:else}
        <ReplyQuote {message} {roomId} {repliedMessage} {onJumpToMessage} />
        <MessageBody
          {message}
          {imageMessages}
          {roomId}
          hasSenderHeader={showSenderName && !isOwn && !grouped}
          onImageContextMenu={openImageOptionsMenuFromContext}
        />
      {/if}

      <div
        class="message-meta mt-1 flex min-w-0 flex-nowrap items-center justify-between gap-2 pb-1 text-[10px] leading-tight"
      >
        {#if !message.isDeleted}<MessageReactions {roomId} {message} {currentUserId} {pickerCustomEmoji} onToggleReaction={onToggleReaction} />{/if}
        <div class="flex min-w-0 shrink flex-nowrap items-center justify-end gap-1 opacity-70" class:opacity-100={isOwn}>
          {#if message.editedAt && !message.isDeleted}
            <span class="italic">edited</span>
          {/if}
          {#if message.threadReplyCount > 0}
            <span class="font-semibold">{message.threadReplyCount} replies</span>
          {/if}
          <span class="min-w-0 max-w-[24rem] flex-1 truncate" title={message.sender}>{senderIdentity}</span>
          <span class="shrink-0 whitespace-nowrap">{toClockTime(message.timestamp)}</span>
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
              <polyline points="12 6 16 14" />
            </svg>
          {:else if isOwn}
            <span aria-label={message.isRead ? "Read" : "Sent"} class="inline-flex shrink-0 text-accent">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="size-3.5" aria-hidden="true">
                <path d="m4 12 4 4L19 5"/>
                {#if message.isRead}<path d="m12 16 9-9"/>{/if}
              </svg>
            </span>
          {/if}
        </div>
      </div>
    </div>

    <MessageSendState {message} {isSending} onRetry={onRetry} onRemove={message.messageType && message.messageType !== "m.text" ? onRemoveFailedMessage : undefined} />
    <MessageEncryptionState {message} />
  </div>



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
    onDelete={isOwn && !message.isDeleted && message.sendState !== "sending" && message.eventId ? onDeleteMessage : undefined}
    {isPinned}
    onTogglePinned={onTogglePinned}
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
