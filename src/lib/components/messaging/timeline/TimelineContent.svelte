<script lang="ts">
  import { tick } from "svelte";
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import MessageItem from "../message/MessageItem.svelte";
  import MessageAvatar from "../message/MessageAvatar.svelte";
  import { openUserProfile } from "$lib/chats/user-profile";
  import { matrixGetPinnedEvents, matrixGetReadContext, matrixSetMessagePinned } from "$lib/chats/api";
  import { createTimelineGrouper } from "./timelineGroups";
  import type {
    ReplyToMessageHandler,
    RetryMessageHandler,
    TimelineMessage,
    ToggleReactionHandler,
  } from "../shared";
  import { daySeparatorLabel, toClockTime } from "../shared";
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
    onRemoveFailedMessage?: RetryMessageHandler;
    onToggleReaction?: ToggleReactionHandler;
    onReplyToMessage?: ReplyToMessageHandler;
    onForward?: (message: TimelineMessage) => void;
    onDeleteMessage?: (message: TimelineMessage) => void;
    onTimelineElementChange?: (element: HTMLElement | null) => void;
    onProgrammaticScroll?: () => void;
    onLoadJumpContext?: (eventId: string) => Promise<boolean>;
    attentionJumpRequest?: { eventId: string; nonce: number } | null;
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
    onRemoveFailedMessage,
    onToggleReaction,
    onReplyToMessage,
    onForward,
    onDeleteMessage,
    onTimelineElementChange,
    onProgrammaticScroll,
    onLoadJumpContext,
    attentionJumpRequest = null,
  }: Props = $props();

  let timelineElement: HTMLElement | null = $state(null);
  let highlightedMessageElement: HTMLElement | null = $state(null);
  let highlightTimeout: ReturnType<typeof setTimeout> | null = $state(null);
  let scrollSettleTimeout: ReturnType<typeof setTimeout> | null = $state(null);
  let pendingScrollCleanup: (() => void) | null = null;
  let ignoreNextGlobalPointerDown = $state(false);
  let pinnedEventIds = $state<string[]>([]);
  let pinnedPreviews = $state<Record<string, { body: string; imageUrl: string | null; timestamp: number | null }>>({});
  let pinnedIndex = $state(0);
  let currentViewTimestamp = $state<number | null>(null);
  let isJumping = false;
  let preservePinnedCycle = false;
  let jumpUnlockTimer: ReturnType<typeof setTimeout> | null = null;
  let viewportSyncFrame: number | null = null;
  let pinError = $state("");

  const wallpaper = $derived($themeSettings.wallpaper);

  const groupMessages = createTimelineGrouper();
  const rows = $derived(groupMessages(messages));
  const imageMessages = $derived(messages.filter((message) => message.messageType === "m.image" && message.imageUrl));

  $effect(() => {
    const request = attentionJumpRequest;
    if (request) void jumpToMessage(request.eventId);
  });

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
  const currentPinnedEventId = $derived(pinnedEventIds.length ? pinnedEventIds[pinnedIndex % pinnedEventIds.length] : null);
  const currentPinnedMessage = $derived(currentPinnedEventId ? messagesByEventId.get(currentPinnedEventId) : undefined);
  const currentPinnedPreview = $derived.by(() => {
    if (!currentPinnedEventId) return null;
    const message = currentPinnedMessage;
    if (message) {
      return {
        body: getPinnedPreviewBody(message.body, message.messageType),
        imageUrl: message.messageType === "m.image" ? message.thumbnailUrl || message.imageUrl : null,
      };
    }
    return pinnedPreviews[currentPinnedEventId] ?? null;
  });

  $effect(() => {
    const selectedRoomId = roomId;
    pinnedEventIds = [];
    pinnedPreviews = {};
    pinnedIndex = 0;
    currentViewTimestamp = null;
    isJumping = false;
    preservePinnedCycle = false;
    if (jumpUnlockTimer) clearTimeout(jumpUnlockTimer);
    jumpUnlockTimer = null;
    if (viewportSyncFrame !== null) cancelAnimationFrame(viewportSyncFrame);
    viewportSyncFrame = null;
    pinError = "";
    if (!selectedRoomId) return;
    let cancelled = false;
    void matrixGetPinnedEvents({ roomId: selectedRoomId }).then(async (response) => {
      if (cancelled) return;
      pinnedEventIds = response.eventIds;
      // Matrix pin order is commonly oldest-first; show the newest entry while timestamps prefetch.
      pinnedIndex = Math.max(0, response.eventIds.length - 1);
      let nextIndex = 0;
      const prefetchWorker = async () => {
        while (!cancelled) {
          const eventId = response.eventIds[nextIndex++];
          if (!eventId) return;
          try {
            const context = await matrixGetReadContext(selectedRoomId, eventId);
            if (cancelled) return;
            const message = context.messages.find((item) => item.eventId === eventId);
            if (message) {
              pinnedPreviews = {
                ...pinnedPreviews,
                [eventId]: {
                  body: getPinnedPreviewBody(message.body, message.messageType),
                  imageUrl: message.messageType === "m.image" ? message.thumbnailUrl || message.imageUrl || null : null,
                  timestamp: message.timestamp ?? null,
                },
              };
              sortPinnedEventsByTimestamp();
            }
          } catch {
            // The pinned banner can still jump through the normal context loader if prefetch fails.
          }
        }
      };
      await Promise.all(Array.from({ length: Math.min(3, response.eventIds.length) }, prefetchWorker));
      if (!cancelled) {
        sortPinnedEventsByTimestamp();
        pinnedIndex = Math.max(0, pinnedEventIds.length - 1);
        syncPinnedIndexToTimestamp(Number.POSITIVE_INFINITY);
      }
    }).catch((error) => {
      if (!cancelled) pinError = error instanceof Error ? error.message : "Failed to load pinned messages";
    });
    return () => { cancelled = true; };
  });

  async function togglePinnedMessage(message: TimelineMessage, pinned: boolean) {
    const eventId = message.eventId;
    if (!eventId) return;
    pinError = "";
    try {
      await matrixSetMessagePinned({ roomId, eventId, pinned });
      const response = await matrixGetPinnedEvents({ roomId });
      pinnedEventIds = response.eventIds;
      sortPinnedEventsByTimestamp();
      pinnedIndex = Math.max(0, pinnedEventIds.length - 1);
    } catch (error) {
      pinError = error instanceof Error ? error.message : `Failed to ${pinned ? "pin" : "unpin"} message`;
    }
  }

  function sortPinnedEventsByTimestamp() {
    const originalOrder = new Map(pinnedEventIds.map((eventId, index) => [eventId, index]));
    pinnedEventIds = [...pinnedEventIds].sort((left, right) => {
      const leftTs = messagesByEventId.get(left)?.timestamp ?? pinnedPreviews[left]?.timestamp ?? null;
      const rightTs = messagesByEventId.get(right)?.timestamp ?? pinnedPreviews[right]?.timestamp ?? null;
      if (leftTs === null) return rightTs === null ? (originalOrder.get(left) ?? 0) - (originalOrder.get(right) ?? 0) : 1;
      if (rightTs === null) return -1;
      return leftTs - rightTs || (originalOrder.get(left) ?? 0) - (originalOrder.get(right) ?? 0);
    });
  }

  async function jumpToNextPinnedMessage() {
    if (!pinnedEventIds.length) return;
    const targetIndex = pinnedIndex % pinnedEventIds.length;
    const eventId = pinnedEventIds[targetIndex];
    if (!eventId) return;
    isJumping = true;
    preservePinnedCycle = true;
    if (jumpUnlockTimer) clearTimeout(jumpUnlockTimer);
    jumpUnlockTimer = null;
    pinnedIndex = (targetIndex - 1 + pinnedEventIds.length) % pinnedEventIds.length;
    await jumpToMessage(eventId, () => {
      isJumping = true;
      if (jumpUnlockTimer) clearTimeout(jumpUnlockTimer);
      jumpUnlockTimer = null;
    });
    if (isJumping) scheduleJumpUnlock(400);
  }

  function handleTimelineScroll(event: Event) {
    onScroll?.(event);
    if (isJumping) {
      scheduleJumpUnlock(180);
      return;
    }
    preservePinnedCycle = false;
    if (viewportSyncFrame !== null) return;
    viewportSyncFrame = requestAnimationFrame(() => {
      viewportSyncFrame = null;
      const viewTimestamp = getCurrentViewportTimestamp();
      if (viewTimestamp === null) return;
      currentViewTimestamp = viewTimestamp;
      syncPinnedIndexToTimestamp(viewTimestamp);
    });
  }

  function scheduleJumpUnlock(delay: number) {
    if (jumpUnlockTimer) clearTimeout(jumpUnlockTimer);
    jumpUnlockTimer = setTimeout(() => {
      isJumping = false;
      jumpUnlockTimer = null;
      if (preservePinnedCycle) return;
      const viewTimestamp = getCurrentViewportTimestamp();
      if (viewTimestamp !== null) {
        currentViewTimestamp = viewTimestamp;
        syncPinnedIndexToTimestamp(viewTimestamp);
      }
    }, delay);
  }

  function getCurrentViewportTimestamp(): number | null {
    const timeline = timelineElement;
    if (!timeline) return null;
    const timestampByEventId = new Map(
      messages.flatMap((message) => message.eventId && message.timestamp != null
        ? [[message.eventId, message.timestamp] as const]
        : []),
    );
    const bannerHeight = timeline.querySelector<HTMLElement>("[data-pinned-message-banner]")?.getBoundingClientRect().height ?? 0;
    const centerY = timeline.getBoundingClientRect().top + (timeline.clientHeight + bannerHeight) / 2;
    const halfBand = Math.max(20, timeline.clientHeight * 0.06);
    let nearest: { timestamp: number; distance: number } | null = null;
    for (const element of timeline.querySelectorAll<HTMLElement>("[data-message-event-id]")) {
      const timestamp = timestampByEventId.get(element.dataset.messageEventId ?? "");
      if (timestamp === undefined) continue;
      const rect = element.getBoundingClientRect();
      if (rect.bottom < centerY - halfBand || rect.top > centerY + halfBand) continue;
      const distance = Math.abs(rect.top + rect.height / 2 - centerY);
      if (!nearest || distance < nearest.distance) nearest = { timestamp, distance };
    }
    return nearest?.timestamp ?? null;
  }

  function syncPinnedIndexToTimestamp(viewTimestamp: number) {
    const candidates = pinnedEventIds.flatMap((eventId, index) => {
      const timestamp = messagesByEventId.get(eventId)?.timestamp ?? pinnedPreviews[eventId]?.timestamp ?? null;
      return timestamp === null ? [] : [{ index, timestamp }];
    });
    // A single unavailable pin must not freeze viewport synchronization for all other pins.
    if (!candidates.length) return;
    candidates.sort((left, right) => left.timestamp - right.timestamp || left.index - right.index);

    // If the viewport is older than every pinned event, keep the newest pin selected.
    let selected = candidates[candidates.length - 1];
    for (const candidate of candidates) {
      if (candidate.timestamp <= viewTimestamp) selected = candidate;
      else break;
    }
    if (selected && selected.index !== pinnedIndex) {
      pinnedIndex = selected.index;
    }
  }

  async function jumpToMessage(eventId: string, beforeScroll?: () => void) {
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

    beforeScroll?.();
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

  function getPinnedPreviewBody(body: string, messageType?: string | null): string {
    if (messageType === "m.image" && /^image\.(png|jpe?g|gif|webp|avif|bmp|heic)$/i.test(body.trim())) {
      return "Photo";
    }
    return body || (messageType === "m.image" ? "Photo" : messageType === "m.video" ? "Video" : "Message");
  }

  $effect(() => {
    onTimelineElementChange?.(timelineElement);
  });

  $effect(() => {
    const timeline = timelineElement;
    const observedPinCount = pinnedEventIds.length;
    const loadedPinPreviewCount = Object.keys(pinnedPreviews).length;
    void observedPinCount;
    void loadedPinPreviewCount;
    const timestampByEventId = new Map<string, number>();
    for (const message of messages) {
      if (message.eventId && message.timestamp !== null && message.timestamp !== undefined) {
        timestampByEventId.set(message.eventId, message.timestamp);
      }
    }
    const messageNodes = timeline?.querySelectorAll<HTMLElement>("[data-message-event-id]") ?? [];
    if (!timeline || typeof IntersectionObserver === "undefined" || !messageNodes.length) return;

    const bannerHeight = timeline.querySelector<HTMLElement>("[data-pinned-message-banner]")?.getBoundingClientRect().height ?? 0;
    const centerOffset = (timeline.clientHeight + bannerHeight) / 2;
    const bandHeight = Math.max(40, timeline.clientHeight * 0.12);
    const topInset = Math.max(0, centerOffset - bandHeight / 2);
    const bottomInset = Math.max(0, timeline.clientHeight - centerOffset - bandHeight / 2);
    const observer = new IntersectionObserver(() => {
      const centerY = timeline.getBoundingClientRect().top + centerOffset;
      const visible = [...messageNodes].map((element) => {
        const timestamp = timestampByEventId.get(element.dataset.messageEventId ?? "");
        if (timestamp === undefined) return null;
        const rect = element.getBoundingClientRect();
        return {
          timestamp,
          distance: Math.abs(rect.top + rect.height / 2 - centerY),
          isInBand: rect.bottom >= centerY - bandHeight / 2 && rect.top <= centerY + bandHeight / 2,
        };
      }).filter((entry): entry is { timestamp: number; distance: number; isInBand: boolean } => entry !== null && entry.isInBand)
        .sort((left, right) => left.distance - right.distance);
      const viewedTimestamp = visible[0]?.timestamp;
      if (viewedTimestamp === undefined) return;
      currentViewTimestamp = viewedTimestamp;
      if (!isJumping && !preservePinnedCycle) syncPinnedIndexToTimestamp(viewedTimestamp);
    }, {
      root: timeline,
      rootMargin: `-${topInset}px 0px -${bottomInset}px 0px`,
      threshold: 0,
    });

    for (const node of messageNodes) observer.observe(node);
    return () => observer.disconnect();
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
      if (jumpUnlockTimer) clearTimeout(jumpUnlockTimer);
      if (viewportSyncFrame !== null) cancelAnimationFrame(viewportSyncFrame);
    };
  });

  $effect(() => {
    return () => {
      if (highlightTimeout) {
        clearTimeout(highlightTimeout);
      }
      pendingScrollCleanup?.();
      if (scrollSettleTimeout) clearTimeout(scrollSettleTimeout);
      if (viewportSyncFrame !== null) cancelAnimationFrame(viewportSyncFrame);
    };
  });
</script>

<div
  class="timeline-scroll relative flex-1 overflow-y-auto overflow-x-hidden px-2 pb-3 min-h-0"
  class:has-pinned-message={pinnedEventIds.length > 0}
  bind:this={timelineElement}
  onscroll={handleTimelineScroll}
>
  {#if pinnedEventIds.length > 0}
    <div class="sticky top-0 z-20 -mx-2" data-pinned-message-banner>
      <button type="button" class="flex min-h-14 w-full cursor-pointer items-center gap-3 border-b border-hairline bg-panel px-3 py-2 text-left transition-colors hover:bg-hover" onclick={jumpToNextPinnedMessage} title="Jump to next pinned message">
        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
          <span class="flex min-w-0 items-center gap-2 text-sm font-bold leading-tight text-primary">
            <span class="shrink-0">Pinned message</span>
            <span class="shrink-0 tabular-nums">#{pinnedIndex + 1}</span>
          </span>
          <span class="flex min-w-0 items-center gap-2">
            {#if currentPinnedPreview?.imageUrl}
              <img src={currentPinnedPreview.imageUrl} alt="" class="size-9 shrink-0 object-cover" loading="lazy" />
            {/if}
            <span class="truncate text-xs leading-tight text-primary">{currentPinnedPreview?.body || "Loading message preview…"}</span>
          </span>
        </span>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="size-4 shrink-0 text-accent" aria-hidden="true"><path d="M9 3h6l-.5 5 3 3v2H6v-2l3-3-.5-5Z" /><path d="M12 13v8" /></svg>
      </button>
    </div>
  {/if}
  {#if pinError}
    <p class="sticky top-12 z-20 mx-auto mb-1 max-w-2xl bg-danger/10 px-3 py-1 text-xs text-danger" role="alert">{pinError}</p>
  {/if}

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
        {:else if row.kind === "system"}
          {@const memberName = row.message.senderDisplayName ?? row.message.sender.split(":")[0]?.replace(/^@/, "") ?? row.message.sender}
          <li class="my-1 flex justify-center" data-message-event-id={row.message.eventId ?? undefined}>
            <span class="max-w-[85%] rounded-full bg-panel/75 px-3 py-1 text-center text-[12px] leading-snug text-secondary shadow-sm">
              {#if row.message.body.startsWith(memberName)}
                <button
                  type="button"
                  class="cursor-pointer text-accent hover:text-primary hover:underline focus-visible:underline"
                  onclick={() => openUserProfile({ roomId, userId: row.message.sender, displayName: row.message.senderDisplayName })}
                >{memberName}</button>{row.message.body.slice(memberName.length)}
              {:else}
                {row.message.body}
              {/if}
              {#if row.message.timestamp}<span class="ml-1 opacity-70">· {toClockTime(row.message.timestamp)}</span>{/if}
            </span>
          </li>
        {:else}
          {@const isOwnGroup = Boolean(currentUserId) && row.sender === currentUserId}
          <li class="sender-group" class:sender-group-own={isOwnGroup}>
            {#if !isOwnGroup}
              <div class="sender-group-avatar">
                <MessageAvatar {roomId} sender={row.sender} displayName={row.messages[0].message.senderDisplayName}
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
                  onRemoveFailedMessage={onRemoveFailedMessage}
                  {currentUserId}
                  {pickerCustomEmoji}
                  onToggleReaction={onToggleReaction}
                  onReplyToMessage={onReplyToMessage}
                  onForward={onForward}
                  onDeleteMessage={onDeleteMessage}
                  isPinned={Boolean(item.message.eventId && pinnedEventIds.includes(item.message.eventId))}
                  onTogglePinned={togglePinnedMessage}
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
  .timeline-scroll.has-pinned-message {
    padding-top: 0;
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
