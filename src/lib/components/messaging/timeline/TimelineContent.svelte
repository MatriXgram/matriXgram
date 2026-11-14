<script lang="ts">
  import { tick } from "svelte";
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import MessageItem from "../message/MessageItem.svelte";
  import MessageAvatar from "../message/MessageAvatar.svelte";
  import { openUserProfile } from "$lib/chats/user-profile";
  import { createTimelineGrouper } from "./timelineGroups";
  import type {
    ReplyToMessageHandler,
    RetryMessageHandler,
    TimelineMessage,
    ToggleReactionHandler,
  } from "../shared";
  import { daySeparatorLabel } from "../shared";
  import { themeSettings } from "$lib/theme";

  interface Props {
    messages: TimelineMessage[];
    unreadBoundaryId?: string | null;
    roomId: string;
    currentUserId: string | null;
    pickerCustomEmoji: PickerCustomEmoji[];
    isSending: boolean;
    /// Show the sender name above incoming bubbles (group chats).
    showSenderNames?: boolean;
    onScroll?: (event: Event) => void;
    onRetryMessage?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    onReplyToMessage?: ReplyToMessageHandler;
    onForward?: (message: TimelineMessage) => void;
    onTimelineElementChange?: (element: HTMLElement | null) => void;
    onProgrammaticScroll?: () => void;
    onLoadJumpContext?: (eventId: string) => Promise<boolean>;
  }

  let {
    messages,
    unreadBoundaryId = null,
    roomId,
    currentUserId,
    pickerCustomEmoji,
    isSending,
    showSenderNames = false,
    onScroll,
    onRetryMessage,
    onToggleReaction,
    onReplyToMessage,
    onForward,
    onTimelineElementChange,
    onProgrammaticScroll,
    onLoadJumpContext,
  }: Props = $props();

  let timelineElement: HTMLElement | null = $state(null);
  let highlightedMessageElement: HTMLElement | null = $state(null);
  let highlightTimeout: ReturnType<typeof setTimeout> | null = $state(null);
  let scrollSettleTimeout: ReturnType<typeof setTimeout> | null = $state(null);
  let pendingScrollCleanup: (() => void) | null = null;
  let ignoreNextGlobalPointerDown = $state(false);

  const wallpaper = $derived($themeSettings.wallpaper);

  const groupMessages = createTimelineGrouper();
  const rows = $derived(groupMessages(messages));
  const imageMessages = $derived(messages.filter((message) => message.messageType === "m.image" && message.imageUrl));

  const messagesByEventId = $derived.by(() => {
    const indexed = new Map<string, TimelineMessage>();
    for (const message of messages) {
      if (!message.eventId) {
        continue;
      }

      indexed.set(message.eventId, message);
    }

    return indexed;
  });

  async function jumpToMessage(eventId: string) {
    const timeline = timelineElement;
    if (!timeline) return;

    const selector =
      `[data-message-event-id="${eventId.replaceAll('"', '\\"')}"]`;
    let target = timeline.querySelector<HTMLElement>(selector);

    if (!target) {
      const loaded = await onLoadJumpContext?.(eventId);
      if (!loaded) return;
      await tick();
      target = timeline.querySelector<HTMLElement>(selector);
      if (!target) return;
    }

    pendingScrollCleanup?.();
    pendingScrollCleanup = null;
    if (scrollSettleTimeout) clearTimeout(scrollSettleTimeout);
    scrollSettleTimeout = null;
    if (highlightTimeout) clearTimeout(highlightTimeout);
    highlightTimeout = null;
    if (highlightedMessageElement && highlightedMessageElement !== target) {
      highlightedMessageElement.classList.remove("reply-target-highlight");
    }

    let highlightStarted = false;
    const startHighlight = () => {
      if (highlightStarted) return;
      highlightStarted = true;
      const timelineRect = timeline.getBoundingClientRect();
      const targetRect = target.getBoundingClientRect();
      target.style.setProperty("--reply-highlight-left", `${timelineRect.left - targetRect.left}px`);
      target.style.setProperty("--reply-highlight-width", `${timelineRect.width}px`);
      target.classList.add("reply-target-highlight");
      highlightedMessageElement = target;
    };

    const finishScroll = () => {
      pendingScrollCleanup?.();
      pendingScrollCleanup = null;
      if (scrollSettleTimeout) clearTimeout(scrollSettleTimeout);
      scrollSettleTimeout = null;
      startHighlight();
      highlightTimeout = setTimeout(() => {
        target.classList.remove("reply-target-highlight");
        if (highlightedMessageElement === target) highlightedMessageElement = null;
        highlightTimeout = null;
      }, 1200);
    };

    let sawScroll = false;
    const targetIsNearViewport = () => {
      const targetRect = target.getBoundingClientRect();
      const viewportRect = timeline.getBoundingClientRect();
      return targetRect.bottom >= viewportRect.top - 220 && targetRect.top <= viewportRect.bottom + 220;
    };
    const handleTimelineScroll = () => {
      sawScroll = true;
      if (targetIsNearViewport()) startHighlight();
      if (scrollSettleTimeout) clearTimeout(scrollSettleTimeout);
      scrollSettleTimeout = setTimeout(finishScroll, 180);
    };
    timeline.addEventListener("scroll", handleTimelineScroll, { passive: true });
    pendingScrollCleanup = () => timeline.removeEventListener("scroll", handleTimelineScroll);
    if (targetIsNearViewport()) startHighlight();

    onProgrammaticScroll?.();
    target.scrollIntoView({ behavior: "smooth", block: "center" });
    // If the target is already in view, there may be no scroll event to wait for.
    scrollSettleTimeout = setTimeout(() => {
      if (!sawScroll) finishScroll();
    }, 300);
    ignoreNextGlobalPointerDown = true;
    queueMicrotask(() => {
      ignoreNextGlobalPointerDown = false;
    });
  }

  $effect(() => {
    onTimelineElementChange?.(timelineElement);
  });

  $effect(() => {
    const onGlobalPointerDown = () => {
      if (ignoreNextGlobalPointerDown) {
        return;
      }

      highlightedMessageElement?.classList.remove("reply-target-highlight");
      highlightedMessageElement = null;
      pendingScrollCleanup?.();
      pendingScrollCleanup = null;
      if (scrollSettleTimeout) {
        clearTimeout(scrollSettleTimeout);
        scrollSettleTimeout = null;
      }
      if (highlightTimeout) {
        clearTimeout(highlightTimeout);
        highlightTimeout = null;
      }
      if (highlightTimeout) {
        clearTimeout(highlightTimeout);
        highlightTimeout = null;
      }
    };

    window.addEventListener("pointerdown", onGlobalPointerDown, true);

    return () => {
      window.removeEventListener("pointerdown", onGlobalPointerDown, true);
    };
  });

  $effect(() => {
    return () => {
      if (highlightTimeout) {
        clearTimeout(highlightTimeout);
      }
      pendingScrollCleanup?.();
      if (scrollSettleTimeout) clearTimeout(scrollSettleTimeout);
    };
  });
</script>

<div
  class="timeline-scroll relative flex-1 overflow-y-auto overflow-x-hidden px-2 pb-3 min-h-0"
  bind:this={timelineElement}
  onscroll={onScroll}
>
  {#if wallpaper}
    <div class="pointer-events-none absolute inset-0 bg-black/30" aria-hidden="true"></div>
  {/if}

  {#if messages.length === 0}
    <p class="px-3 py-6 text-center text-sm text-secondary">No messages yet.</p>
  {:else}
    <ul class="timeline-messages relative flex flex-col gap-2">
      {#each rows as row (row.key)}
        {#if row.kind === "separator"}
          <li class="my-3 flex justify-center">
            <span class="day-separator rounded-full bg-panel font-medium">
              {daySeparatorLabel(row.timestamp)}
            </span>
          </li>
        {:else}
          {@const isOwnGroup = Boolean(currentUserId) && row.sender === currentUserId}
          <li class="sender-group" class:sender-group-own={isOwnGroup}>
            {#if !isOwnGroup}
              <div class="sender-group-avatar">
                <MessageAvatar {roomId} sender={row.sender}
                  onOpenProfile={(avatarUrl) => openUserProfile({
                    roomId, userId: row.sender,
                    displayName: row.messages[0].message.senderDisplayName,
                    avatarUrl,
                  })} />
              </div>
            {/if}
            <ul class="sender-group-messages">
              {#each row.messages as item, index (item.key)}
                {#if unreadBoundaryId && item.message.eventId === unreadBoundaryId}
                  <li data-unread-divider class="my-3 flex items-center gap-3 py-1 text-[11px] font-semibold text-accent" role="separator" aria-label="Unread messages">
                    <span class="h-px flex-1 bg-accent/30"></span><span>Unread messages</span><span class="h-px flex-1 bg-accent/30"></span>
                  </li>
                {/if}
                <MessageItem
                  message={item.message}
                  {imageMessages}
                  {roomId}
                  grouped={index > 0}
                  groupEnd={index === row.messages.length - 1}
                  showSenderName={showSenderNames}
                  repliedMessage={item.message.inReplyToEventId ? messagesByEventId.get(item.message.inReplyToEventId) : undefined}
                  onJumpToMessage={jumpToMessage}
                  onRetry={onRetryMessage}
                  {currentUserId}
                  {pickerCustomEmoji}
                  onToggleReaction={onToggleReaction}
                  onReplyToMessage={onReplyToMessage}
                  onForward={onForward}
                  {isSending}
                />
              {/each}
            </ul>
          </li>
        {/if}
      {/each}
    </ul>
  {/if}
</div>

<style>
  .sender-group {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    min-width: 0;
    padding-left: 8px;
  }
  .sender-group-own { padding-left: 0; }
  .sender-group-avatar {
    position: sticky;
    bottom: 4px;
    flex: 0 0 32px;
    align-self: flex-end;
    z-index: 1;
    padding-bottom: 1px;
  }
  .sender-group-messages {
    flex: 1;
    min-width: 0;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .timeline-scroll {
    overflow-anchor: auto;
    background-color: var(--tg-panel-alt);
    background-image: var(--tg-wallpaper, none);
    background-size: cover;
    background-position: center;
    background-repeat: no-repeat;
  }
</style>
