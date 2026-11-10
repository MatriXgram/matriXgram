<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import EmojiPickerPopover from "../composer-emoji/EmojiPickerPopover.svelte";
  import {
    buildEmojiByShortcodeToken,
    buildPickerEmojiBySourceUrl,
    customEmojiUrlForToken,
    reactionDisplayName,
  } from "./helpers";
  import type { TimelineMessage, ToggleReactionHandler } from "../shared";

  interface Props {
    message: TimelineMessage;
    currentUserId?: string | null;
    pickerCustomEmoji?: PickerCustomEmoji[];
    onToggleReaction?: ToggleReactionHandler;
  }

  let {
    message,
    currentUserId = null,
    pickerCustomEmoji = [],
    onToggleReaction,
  }: Props = $props();

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
</script>

{#if message.eventId}
  <div class="mt-2 flex flex-wrap items-center gap-1">
    {#each message.reactions as reaction (`${reaction.key}-${reaction.senders.join("|")}`)}
      {@const reactionEmojiUrl = customEmojiUrlForToken(reaction.key, emojiByShortcodeToken, pickerEmojiBySourceUrl)}
      <button
        type="button"
        class="rounded-full border px-2 py-0.5 text-xs"
        title={reactionDisplayName(reaction.key, message, pickerCustomEmoji)}
        class:bg-accent-soft={ownReaction(reaction)}
        class:text-accent={ownReaction(reaction)}
        class:border-accent={ownReaction(reaction)}
        class:bg-hover={!ownReaction(reaction)}
        class:border-hairline-strong={!ownReaction(reaction)}
        onclick={() => handleReactionClick(reaction.key)}
      >
        {#if reactionEmojiUrl}
          <img
            src={reactionEmojiUrl}
            alt={reaction.key}
            title={reactionDisplayName(reaction.key, message, pickerCustomEmoji)}
            class="inline-block h-5 w-5 align-text-bottom"
            loading="lazy"
          />
        {:else}
          <span class="inline-block text-2xl leading-none align-text-bottom" title={reactionDisplayName(reaction.key, message, pickerCustomEmoji)}>{reaction.key}</span>
        {/if}
        {reaction.count}
      </button>
    {/each}

    <EmojiPickerPopover
      buttonText="+"
      buttonClass="rounded-full border border-hairline-strong bg-hover px-2 py-0.5 text-xs"
      popoverClass="absolute bottom-full left-0 mb-1 z-10 rounded-xl border border-hairline bg-panel p-2 shadow-lg w-[22rem] max-w-[80vw]"
      pickerStyle="width: 100%; height: 22rem; --emoji-size: 2rem; --category-emoji-size: 1.15rem;"
      {pickerCustomEmoji}
      onSelect={handleReactionClick}
    />
  </div>
{/if}
