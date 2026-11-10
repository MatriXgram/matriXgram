<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import MessageItem from "../message/MessageItem.svelte";
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
  }

  let {
    messages,
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
  }: Props = $props();

  let timelineElement: HTMLElement | null = $state(null);
  let highlightedMessageElement: HTMLElement | null = $state(null);
  let highlightTimeout: ReturnType<typeof setTimeout> | null = $state(null);
  let ignoreNextGlobalPointerDown = $state(false);

  const wallpaper = $derived($themeSettings.wallpaper);

  /// Two messages belong to the same group when they come from the same
  /// sender, are less than five minutes apart and share a day. A grouped
  /// message is drawn without an avatar and without a sender name, the way
  /// Telegram collapses consecutive messages.
  const GROUPING_WINDOW_MS = 5 * 60 * 1000;

  type TimelineRow =
    | { kind: "separator"; key: string; label: string }
    | {
        kind: "message";
        key: string;
        message: TimelineMessage;
        grouped: boolean;
        lastInGroup: boolean;
      };

  const rows = $derived.by(() => {
    const result: TimelineRow[] = [];
    let previous: TimelineMessage | null = null;
    let previousDay: string | null = null;

    for (const [index, message] of messages.entries()) {
      const key = message.eventId ?? message.localId ?? `${index}-${message.timestamp ?? 0}`;
      const day = message.timestamp ? new Date(message.timestamp).toDateString() : null;

      if (day && day !== previousDay) {
        result.push({ kind: "separator", key: `separator-${key}`, label: daySeparatorLabel(message.timestamp) });
      }

      const grouped = Boolean(
        previous &&
          previous.sender === message.sender &&
          previous.timestamp &&
          message.timestamp &&
          day &&
          day === previousDay &&
          message.timestamp - previous.timestamp < GROUPING_WINDOW_MS,
      );

      result.push({ kind: "message", key, message, grouped, lastInGroup: true });
      previous = message;
      previousDay = day;
    }

    // The last message of a group carries the bubble tail; the ones above it
    // are drawn as a stack.
    for (let index = result.length - 1; index >= 0; index -= 1) {
      const row = result[index];
      if (row.kind !== "message") {
        continue;
      }

      const next = result[index + 1];
      const nextContinuesGroup =
        next?.kind === "message" && next.grouped && next.message.sender === row.message.sender;

      row.lastInGroup = !nextContinuesGroup;
    }

    return result;
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

  function jumpToMessage(eventId: string) {
    if (!timelineElement) {
      return;
    }

    const target = timelineElement.querySelector<HTMLElement>(
      `[data-message-event-id="${eventId.replaceAll('"', '\\"')}"]`,
    );

    if (!target) {
      return;
    }

    onProgrammaticScroll?.();
    target.scrollIntoView({ behavior: "smooth", block: "center" });

    if (highlightedMessageElement && highlightedMessageElement !== target) {
      highlightedMessageElement.classList.remove("bg-surface-100-900");
      highlightedMessageElement.classList.remove("border-primary-500");
    }

    target.classList.add("bg-surface-100-900");
    target.classList.add("border-primary-500");
    highlightedMessageElement = target;
    ignoreNextGlobalPointerDown = true;
    queueMicrotask(() => {
      ignoreNextGlobalPointerDown = false;
    });

    if (highlightTimeout) {
      clearTimeout(highlightTimeout);
    }

    highlightTimeout = setTimeout(() => {
      highlightedMessageElement?.classList.remove("border-primary-500");
      highlightedMessageElement = null;
    }, 750);
  }

  $effect(() => {
    onTimelineElementChange?.(timelineElement);
  });

  $effect(() => {
    const onGlobalPointerDown = () => {
      if (ignoreNextGlobalPointerDown) {
        return;
      }

      highlightedMessageElement?.classList.remove("bg-surface-100-900");
      highlightedMessageElement = null;
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
    };
  });
</script>

<div
  class="timeline-scroll relative flex-1 overflow-y-auto px-2 pb-3 min-h-0"
  bind:this={timelineElement}
  onscroll={onScroll}
>
  {#if wallpaper}
    <div class="pointer-events-none absolute inset-0 bg-black/30" aria-hidden="true"></div>
  {/if}

  {#if messages.length === 0}
    <p class="px-3 py-6 text-center text-sm text-secondary">No messages yet.</p>
  {:else}
    <ul class="relative flex flex-col gap-0.5">
      {#each rows as row (row.key)}
        {#if row.kind === "separator"}
          <li class="my-3 flex justify-center">
            <span class="rounded-full bg-panel px-3 py-1 text-[11px] font-medium text-secondary shadow-sm">
              {row.label}
            </span>
          </li>
        {:else}
          <MessageItem
            message={row.message}
            {roomId}
            grouped={row.grouped}
            lastInGroup={row.lastInGroup}
            showSenderName={showSenderNames}
            repliedMessage={
              row.message.inReplyToEventId
                ? messagesByEventId.get(row.message.inReplyToEventId)
                : undefined
            }
            onJumpToMessage={jumpToMessage}
            onRetry={onRetryMessage}
            {currentUserId}
            {pickerCustomEmoji}
            onToggleReaction={onToggleReaction}
            onReplyToMessage={onReplyToMessage}
            onForward={onForward}
            isSending={isSending}
          />
        {/if}
      {/each}
    </ul>
  {/if}
</div>

<style>
  .timeline-scroll {
    overflow-anchor: auto;
    background-color: var(--tg-panel-alt);
    background-image: var(--tg-wallpaper, none);
    background-size: cover;
    background-position: center;
    background-repeat: no-repeat;
  }
</style>
