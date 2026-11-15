<script lang="ts">
  import { onDestroy } from "svelte";
  import { matrixResolveMediaUrl } from "$lib/chats/api";
  import { loadUserProfile } from "$lib/chats/user-profile";
  import ReactionParticipant from "./ReactionParticipant.svelte";
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import {
    buildEmojiByShortcodeToken,
    buildPickerEmojiBySourceUrl,
    customEmojiUrlForToken,
    reactionDisplayName,
  } from "./helpers";
  import type { TimelineMessage, ToggleReactionHandler } from "../shared";

  interface Props {
    message: TimelineMessage;
    roomId: string;
    currentUserId?: string | null;
    pickerCustomEmoji?: PickerCustomEmoji[];
    onToggleReaction?: ToggleReactionHandler;
  }

  let {
    message,
    roomId,
    currentUserId = null,
    pickerCustomEmoji = [],
    onToggleReaction,
  }: Props = $props();

  let hoveredKey = $state<string | null>(null);
  let origin = $state<{ left: number; top: number; bottom: number } | null>(null);
  let popup = $state<HTMLDivElement | null>(null);
  let position = $state({ x: 8, y: 8 });
  let popupMaxHeight = $state(320);
  let failedReactionImages = $state(new Set<string>());
  let closeTimer: ReturnType<typeof setTimeout>;
  const hoveredReaction = $derived(message.reactions.find((reaction) => reaction.key === hoveredKey));
  function keepOpen() { clearTimeout(closeTimer); }
  function scheduleClose() { clearTimeout(closeTimer); closeTimer = setTimeout(() => { hoveredKey = null; }, 180); }
  function showPeople(key: string, element: EventTarget | null) {
    if (!(element instanceof HTMLElement)) return;
    keepOpen();
    const rect = element.getBoundingClientRect();
    origin = { left: rect.left, top: rect.top, bottom: rect.bottom };
    hoveredKey = key;
    for (const userId of message.reactions.find(reaction => reaction.key === key)?.senders ?? []) {
      void loadUserProfile(roomId, userId).catch(() => {});
    }
  }
  onDestroy(() => clearTimeout(closeTimer));
  $effect(() => {
    const element = popup, anchor = origin;
    if (!element || !anchor || !hoveredReaction) return;
    const reposition = () => {
      const rect = element.getBoundingClientRect();
      const availableBelow = Math.max(0, window.innerHeight - anchor.bottom - 10);
      const availableAbove = Math.max(0, anchor.top - 10);
      const openBelow = availableBelow >= rect.height || availableBelow >= availableAbove;
      popupMaxHeight = openBelow ? availableBelow : availableAbove;
      position = {
        x: Math.max(8, Math.min(anchor.left, window.innerWidth - rect.width - 8)),
        y: openBelow
          ? anchor.bottom + 2
          : Math.max(8, anchor.top - Math.min(rect.height, popupMaxHeight) - 2),
      };
    };
    reposition();
    const observer = new ResizeObserver(reposition);
    observer.observe(element);
    return () => observer.disconnect();
  });

  const pickerEmojiBySourceUrl = $derived.by(() => buildPickerEmojiBySourceUrl(pickerCustomEmoji));
  const emojiByShortcodeToken = $derived.by(() => buildEmojiByShortcodeToken(message, pickerCustomEmoji));

  function ownReaction(reaction: { senders: string[] }): boolean {
    return Boolean(currentUserId && reaction.senders.includes(currentUserId));
  }

  function handleReactionClick(key: string) {
    if (!message.eventId) {
      return;
    }

    onToggleReaction?.(message, key);
  }

  function resolveReactionImage(image: HTMLImageElement, mediaUrl: string) {
    let active = true;
    let requestUrl = mediaUrl;
    const resolve = (url: string) => {
      requestUrl = url;
      if (url.startsWith("mxc://")) {
        image.removeAttribute("src");
        void matrixResolveMediaUrl(url).then((resolved) => {
          if (active && resolved) image.src = resolved;
        }).catch(() => {});
      }
    };
    resolve(mediaUrl);
    return {
      update(nextUrl: string) {
        if (nextUrl !== requestUrl) resolve(nextUrl);
      },
      destroy() { active = false; },
    };
  }

  function markReactionImageFailed(key: string) {
    failedReactionImages = new Set(failedReactionImages).add(key);
  }
</script>

{#if message.eventId}
  <div class="inline-flex w-fit max-w-[55%] min-w-0 shrink-0 flex-wrap items-center gap-0.5">
    {#each message.reactions as reaction (`${reaction.key}-${reaction.senders.join("|")}`)}
      {@const reactionEmojiUrl = customEmojiUrlForToken(reaction.key, emojiByShortcodeToken, pickerEmojiBySourceUrl)}
      <button
        type="button"
        class="inline-flex w-fit shrink-0 cursor-pointer items-center gap-0.5 rounded-full border px-1.5 py-px text-[11px] leading-none transition hover:ring-1 hover:ring-accent focus-visible:ring-1 focus-visible:ring-accent"
        aria-label={`${reactionDisplayName(reaction.key, message, pickerCustomEmoji)}: ${reaction.count} reactions`}
        onpointerenter={(event) => showPeople(reaction.key, event.currentTarget)}
        onpointerleave={scheduleClose}
        onfocus={(event) => showPeople(reaction.key, event.currentTarget)}
        onblur={scheduleClose}
        class:bg-accent-soft={ownReaction(reaction)}
        class:text-accent={ownReaction(reaction)}
        class:border-accent={ownReaction(reaction)}
        class:bg-hover={!ownReaction(reaction)}
        class:border-hairline-strong={!ownReaction(reaction)}
        onclick={() => handleReactionClick(reaction.key)}
      >
        {#if reactionEmojiUrl && !failedReactionImages.has(reaction.key)}
          <img
            use:resolveReactionImage={reactionEmojiUrl}
            src={reactionEmojiUrl}
            alt={reaction.key}
            class="inline-block size-3.5 align-text-bottom"
            loading="lazy"
            onerror={() => markReactionImageFailed(reaction.key)}
          />
        {:else}
          <span class="inline-block text-[13px] leading-none align-text-bottom">{reaction.key}</span>
        {/if}
        <span class="tabular-nums">{reaction.count}</span>
      </button>
    {/each}
  </div>
{/if}

<svelte:window onresize={() => (hoveredKey = null)} onkeydown={(event) => {
  if (event.key === "Escape") hoveredKey = null;
}} onpointerdown={(event) => {
  if (popup && event.target instanceof Node && !popup.contains(event.target)) hoveredKey = null;
}} />
{#if hoveredReaction && origin}
  <div bind:this={popup} role="dialog" tabindex="-1" aria-label="People who reacted"
    onpointerenter={keepOpen} onpointerleave={scheduleClose}
    onfocusin={keepOpen} onfocusout={scheduleClose}
    class="fixed z-[65] w-72 max-w-[calc(100vw-16px)] overflow-hidden rounded-xl border border-hairline bg-panel text-primary shadow-xl"
    style={`left: ${position.x}px; top: ${position.y}px; max-height: ${popupMaxHeight}px;`}>
    <div class="flex items-center justify-between border-b border-hairline px-3 py-2 text-xs font-semibold">
      <span class="truncate">{reactionDisplayName(hoveredReaction.key, message, pickerCustomEmoji)} · {hoveredReaction.count}</span>
      <button type="button" class="ml-2 cursor-pointer rounded px-1 hover:bg-hover" aria-label="Close reaction list" onclick={() => (hoveredKey = null)}>×</button>
    </div>
    <ul class="max-h-[min(260px,calc(100dvh-64px))] space-y-1 overflow-y-auto p-1">
      {#each Array.from(new Set(hoveredReaction.senders)) as userId (userId)}
        <li class="border-b border-hairline/60 last:border-b-0">
          <ReactionParticipant {roomId} {userId} timestamp={hoveredReaction.participants?.find(person => person.userId === userId)?.timestamp ?? null} />
        </li>
      {/each}
    </ul>
  </div>
{/if}
