<script lang="ts">
  import { getShortcodeSuggestions } from "$lib/emoji/picker";
  import type { EmojiShortcodeSuggestion, PickerCustomEmoji } from "$lib/emoji/picker";
  import { replyTextPreview as deriveReplyTextPreview, stripMxReplyBlock } from "../shared";
  import type { TimelineMessage } from "../shared";
  import { ErrorBanner } from "../shared";
  import { ShortcodeSuggestions } from "../composer-emoji";
  import { EmojiPickerPopover } from "../composer-emoji";
  import { ComposerEditor } from ".";
  import type { ComposerEditorHandle, ShortcodeRange, SuggestionPosition } from ".";

  interface Props {
    draft: string;
    error: string;
    isSending: boolean;
    isDisabled: boolean;
    placeholder?: string;
    pickerCustomEmoji?: PickerCustomEmoji[];
    replyToMessage?: TimelineMessage | null;
    focusNonce?: number;
    onSubmit?: (draft: string) => void;
    onDraftChange?: (draft: string) => void;
    onClearReply?: () => void;
    onChooseAttachment?: () => void;
    onPasteAttachmentPath?: (filePath: string) => void;
  }

  let {
    draft,
    error,
    isSending,
    isDisabled,
    placeholder = "Write a message...",
    pickerCustomEmoji = [],
    replyToMessage = null,
    focusNonce = 0,
    onSubmit,
    onDraftChange,
    onClearReply,
    onChooseAttachment,
    onPasteAttachmentPath,
  }: Props = $props();

  let composerEditor = $state<ComposerEditorHandle | null>(null);
  let cursorPosition = $state(0);
  let draftShortcodeSuggestions = $state<EmojiShortcodeSuggestion[]>([]);
  let draftShortcodeRange = $state<ShortcodeRange | null>(null);
  let suggestionPopupPosition = $state<SuggestionPosition | null>(null);
  let lastAppliedFocusNonce = $state(0);

  let emojiPickerOpen = $state(false);
  let emojiPickerAnchor = $state<{ x: number; y: number } | null>(null);

  const replyTextPreview = $derived.by(() => {
    return deriveReplyTextPreview(replyToMessage);
  });

  const replyFormattedPreview = $derived.by(() => {
    if (!replyToMessage?.formattedBody || replyToMessage.messageType === "m.image") {
      return null;
    }

    return stripMxReplyBlock(replyToMessage.formattedBody);
  });

  $effect(() => {
    let cancelled = false;

    void (async () => {
      const suggestionResult = await getShortcodeSuggestions(draft, cursorPosition, pickerCustomEmoji);
      if (cancelled) {
        return;
      }

      draftShortcodeSuggestions = suggestionResult?.suggestions ?? [];
      draftShortcodeRange = suggestionResult
        ? { start: suggestionResult.start, end: suggestionResult.end }
        : null;
    })();

    return () => {
      cancelled = true;
    };
  });

  function handleSubmit(event: SubmitEvent) {
    event.preventDefault();
    if (!isSending && draft.trim().length > 0) {
      onSubmit?.(draft);
      clearDraft();
    }
  }

  function clearDraft() {
    composerEditor?.clearDraft();
    draftShortcodeSuggestions = [];
    draftShortcodeRange = null;
    suggestionPopupPosition = null;
  }

  function insertEmoji(emoji: string) {
    composerEditor?.insertAtCursor(emoji);
  }

  function applyShortcodeSuggestion(suggestion: EmojiShortcodeSuggestion) {
    const range = draftShortcodeRange;
    if (!range) {
      return;
    }

    draftShortcodeSuggestions = [];
    draftShortcodeRange = null;
    suggestionPopupPosition = null;

    composerEditor?.replaceRange(range.start, range.end, suggestion.replacement);
  }

  function handleEditorDraftChange(nextDraft: string) {
    draft = nextDraft;
    onDraftChange?.(nextDraft);
  }

  function handleEditorSubmit(nextDraft: string) {
    onSubmit?.(nextDraft);
    draftShortcodeSuggestions = [];
    draftShortcodeRange = null;
    suggestionPopupPosition = null;
  }

  /// Right click in the input opens the emoji table, the way Telegram does.
  function handleEditorContextMenu(event: MouseEvent) {
    event.preventDefault();

    const pickerWidth = 360;
    const pickerHeight = 400;

    emojiPickerAnchor = {
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - pickerWidth - 8)),
      y: Math.max(8, event.clientY - pickerHeight),
    };
    emojiPickerOpen = true;
  }

  $effect(() => {
    if (focusNonce === lastAppliedFocusNonce) {
      return;
    }

    lastAppliedFocusNonce = focusNonce;
    composerEditor?.focusEditor();
  });
</script>

{#snippet emojiIcon()}
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
{/snippet}

<form
  class="relative flex flex-col rounded-2xl border border-hairline bg-panel px-2 py-1.5"
  onsubmit={handleSubmit}
>
  <ErrorBanner {error} class="mb-2 text-sm preset-filled-error-500 card p-2" />

  {#if replyToMessage}
    <div class="mb-1 flex items-start justify-between gap-2 rounded-lg border-l-4 border-accent bg-hover px-3 py-1.5">
      <div class="min-w-0">
        <p class="text-[11px] font-semibold text-accent">
          Replying to {replyToMessage.senderDisplayName ?? replyToMessage.sender}
        </p>

        {#if replyToMessage.messageType === "m.image" && replyToMessage.imageUrl}
          <div class="mt-1 flex items-center gap-2 text-[13px]">
            <img
              src={replyToMessage.imageUrl}
              alt={replyToMessage.body || "Reply image"}
              loading="lazy"
              class="h-7 w-7 rounded border border-hairline object-cover"
            />
            <span class="truncate">Image</span>
          </div>
        {:else if replyFormattedPreview}
          <div class="reply-formatted-preview mt-1 text-[13px]">
            {@html replyFormattedPreview}
          </div>
        {:else if replyTextPreview}
          <p class="mt-1 truncate text-[13px]">{replyTextPreview}</p>
        {/if}
      </div>
      <button
        type="button"
        class="shrink-0 rounded px-2 py-0.5 text-[11px] font-medium text-secondary hover:bg-active hover:text-primary"
        onclick={() => onClearReply?.()}
      >
        Clear
      </button>
    </div>
  {/if}

  <div class="flex items-end gap-1">
    <button
      type="button"
      class="btn btn-sm shrink-0"
      disabled={isDisabled || isSending}
      title="Attach a file"
      aria-label="Attach a file"
      onclick={() => onChooseAttachment?.()}
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
        <path
          d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"
        />
      </svg>
    </button>

    <div
      id="message-draft"
      class="min-w-0 flex-1"
      role="textbox"
      tabindex="-1"
      aria-label="Message"
      aria-multiline="true"
      oncontextmenu={handleEditorContextMenu}
    >
      <ComposerEditor
        bind:this={composerEditor}
        {draft}
        {placeholder}
        {isDisabled}
        {isSending}
        {pickerCustomEmoji}
        shortcodeRange={draftShortcodeRange}
        shortcodeSuggestionCount={draftShortcodeSuggestions.length}
        onDraftChange={handleEditorDraftChange}
        onCursorChange={(position) => {
          cursorPosition = position;
        }}
        onSubmit={handleEditorSubmit}
        onSuggestionPositionChange={(position) => {
          suggestionPopupPosition = position;
        }}
        onPasteAttachmentPath={onPasteAttachmentPath}
      />
    </div>

    <EmojiPickerPopover
      bind:open={emojiPickerOpen}
      icon={emojiIcon}
      buttonClass="btn btn-sm shrink-0"
      anchor={emojiPickerAnchor}
      pickerCustomEmoji={pickerCustomEmoji}
      onSelect={insertEmoji}
    />

    <button
      type="submit"
      class="btn btn-sm btn-primary shrink-0"
      disabled={isDisabled || isSending || draft.trim().length === 0}
      title="Send"
      aria-label="Send message"
    >
      {#if isSending}
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          class="size-4 animate-spin"
          aria-hidden="true"
        >
          <path d="M21 12a9 9 0 1 1-6.22-8.56" />
        </svg>
      {:else}
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
          <line x1="22" y1="2" x2="11" y2="13" />
          <polygon points="22 2 15 22 11 13 2 9 22 2" />
        </svg>
      {/if}
    </button>
  </div>

  {#if draftShortcodeSuggestions.length > 0 && suggestionPopupPosition}
    <ShortcodeSuggestions
      suggestions={draftShortcodeSuggestions}
      position={suggestionPopupPosition}
      disabled={isDisabled || isSending}
      onApply={applyShortcodeSuggestion}
    />
  {/if}
</form>
<style>
  #message-draft :global(.composer-editor) {
    font-size: 1em;
    line-height: 1.35;
    outline: none;
    min-height: 1.4rem;
    max-height: 9rem;
    padding-top: 0.15rem;
    padding-bottom: 0.15rem;
    height: auto;
  }
  #message-draft :global(.composer-editor p) {
    margin: 0;
  }

  #message-draft :global(.composer-editor p + p) {
    margin-top: 0.1rem;
  }

  #message-draft :global(.composer-editor:empty::before),
  #message-draft :global(.composer-editor p:only-child:empty::before) {
    content: attr(data-placeholder);
    color: color-mix(in oklab, currentColor 55%, transparent);
    pointer-events: none;
  }

  #message-draft :global(.composer-custom-emoji) {
    display: inline-block;
    vertical-align: text-bottom;
    width: 1.3em;
    height: 1.3em;
    max-width: 100%;
    max-height: 1.7em;
    margin-inline: 0.04rem;
  }

  .reply-formatted-preview {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .reply-formatted-preview :global(p) {
    display: inline;
    margin: 0;
  }

  .reply-formatted-preview :global(img[data-mx-emoticon]),
  .reply-formatted-preview :global(img[data_mx_emoticon]) {
    display: inline-block;
    vertical-align: text-bottom;
    width: 1em;
    height: 1em;
    max-width: 100%;
    max-height: 1.3em;
    margin-inline: 0.02rem;
  }
</style>
