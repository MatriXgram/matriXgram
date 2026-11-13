<script lang="ts">
  import { onDestroy } from "svelte";
  import { matrixReadClipboardText } from "$lib/chats/api";

  import type { PickerCustomEmoji } from "$lib/emoji/picker";
  import "prosemirror-view/style/prosemirror.css";
  import { baseKeymap } from "prosemirror-commands";
  import { history, redo, undo } from "prosemirror-history";
  import { keymap } from "prosemirror-keymap";
  import { EditorState, TextSelection } from "prosemirror-state";
  import { EditorView } from "prosemirror-view";

  import { buildDocFromDraft, composerSchema } from "./editorSchema";
  import { docToDraft, getSelectionOffsetsFromState, textOffsetToPos } from "./editorHelpers";
  import type { ComposerMention, ShortcodeRange, SuggestionPosition } from "./types";

  interface Props {
    draft: string;
    placeholder: string;
    isDisabled: boolean;
    isSending: boolean;
    pickerCustomEmoji: PickerCustomEmoji[];
    shortcodeRange: ShortcodeRange | null;
    shortcodeSuggestionCount: number;
    onMentionKeyDown?: (event: KeyboardEvent) => boolean;
    onDraftChange?: (draft: string) => void;
    onCursorChange?: (cursor: number) => void;
    onSubmit?: (draft: string) => void;
    onSuggestionPositionChange?: (position: SuggestionPosition | null) => void;
    onPasteAttachmentPath?: (filePath: string) => void;
  }

  let {
    draft,
    placeholder,
    isDisabled,
    isSending,
    pickerCustomEmoji,
    shortcodeRange,
    shortcodeSuggestionCount,
    onMentionKeyDown,
    onDraftChange,
    onCursorChange,
    onSubmit,
    onSuggestionPositionChange,
    onPasteAttachmentPath,
  }: Props = $props();

  let editorMountElement = $state<HTMLDivElement | null>(null);
  let editorView = $state<EditorView | null>(null);
  let cursorPosition = $state(0);
  let suppressDraftSync = false;
  let formatPosition = $state<{ x: number; y: number } | null>(null);
  const formats = [
    { label: "Bold", prefix: "**", suffix: "**" },
    { label: "Italic", prefix: "*", suffix: "*" },
    { label: "Strikethrough", prefix: "~~", suffix: "~~" },
    { label: "Inline code", prefix: "`", suffix: "`" },
    { label: "Block code", prefix: "", suffix: "" },
    { label: "Quote", prefix: "> ", suffix: "" },
  ];

  function updateFormatPosition(view: EditorView) {
    if (view.state.selection.empty || isDisabled) { formatPosition = null; return; }
    const coords = view.coordsAtPos(view.state.selection.from);
    formatPosition = {
      x: Math.max(8, Math.min(coords.left, window.innerWidth - 200)),
      y: Math.max(8, Math.min(coords.top - 224, window.innerHeight - 224)),
    };
  }

  function applyFormat(format: typeof formats[number]) {
    if (!editorView || isDisabled) return;
    const { start, end } = getSelectionOffsetsFromState(editorView.state);
    const current = docToDraft(editorView.state.doc);
    const text = current.slice(start, end);
    if (!text) return;
    let replacement: string;
    if (format.label === "Block code") {
      const runs = text.match(/`+/g) ?? [];
      const fence = "`".repeat(Math.max(3, ...runs.map((run) => run.length + 1)));
      replacement = (start > 0 && current[start - 1] !== "\n" ? "\n" : "") +
        fence + "\n" + text + "\n" + fence +
        (end < current.length && current[end] !== "\n" ? "\n" : "");
    } else if (format.label === "Quote") {
      replacement = (start > 0 && current[start - 1] !== "\n" ? "\n" : "") + text.split("\n").map((line) => "> " + line).join("\n") +
        (end < current.length && current[end] !== "\n" ? "\n" : "");
    } else replacement = format.prefix + text + format.suffix;
    // Use a transaction so formatting can be undone with Ctrl+Z.
    const from = editorView.state.selection.from;
    const to = editorView.state.selection.to;
    const fragment = buildDocFromDraft(replacement, pickerCustomEmoji);
    const slice = fragment.slice(1, fragment.content.size - 1);
    editorView.dispatch(editorView.state.tr.replaceRange(from, to, slice).scrollIntoView());
    editorView.focus();
    formatPosition = null;
  }

  function buildDocPreservingMentions(value: string, sourceDoc: EditorState["doc"]) {
    const doc = buildDocFromDraft(value, pickerCustomEmoji);
    const mentions: Array<{ start: number; end: number; userId: string; displayName: string }> = [];
    sourceDoc.descendants((node, position) => {
      if (node.type.name !== "mention") return;
      const displayName = String(node.attrs.displayName ?? "");
      const start = draftBetween(sourceDoc, 0, position).length;
      if (value.slice(start, start + displayName.length + 1) !== `@${displayName}`) return;
      mentions.push({ start, end: start + displayName.length + 1, userId: String(node.attrs.userId ?? ""), displayName });
    });
    if (mentions.length === 0) return doc;

    let transaction = EditorState.create({ schema: composerSchema, doc }).tr;
    for (const mention of mentions.reverse()) {
      const from = textOffsetToPos(transaction.doc, mention.start);
      const to = textOffsetToPos(transaction.doc, mention.end);
      transaction = transaction.replaceWith(from, to, composerSchema.node("mention", {
        userId: mention.userId,
        displayName: mention.displayName,
      }));
    }
    return transaction.doc;
  }

  onDestroy(() => {
    editorView?.destroy();
  });

  $effect(() => {
    if (!editorMountElement || editorView) {
      return;
    }

    const initialDoc = buildDocFromDraft(draft, pickerCustomEmoji);

    const state = EditorState.create({
      schema: composerSchema,
      doc: initialDoc,
      plugins: [
        history(),
        keymap({
          "Mod-z": undo,
          "Shift-Mod-z": redo,
          "Mod-y": redo,
        }),
        keymap(baseKeymap),
      ],
    });

    const view = new EditorView(editorMountElement, {
      state,
      editable: () => !isDisabled,
      clipboardTextSerializer: (slice) =>
        slice.content.textBetween(0, slice.content.size, "\n", (leafNode) => {
          if (leafNode.type.name === "customEmoji") {
            return leafNode.attrs.token ?? "";
          }
          if (leafNode.type.name === "mention") {
            return `@${leafNode.attrs.displayName ?? ""}`;
          }

          return "";
        }),
      handleKeyDown: (view, event) => {
        if (onMentionKeyDown?.(event)) {
          return true;
        }

        const isPasteShortcut =
          (event.metaKey || event.ctrlKey) &&
          event.key.toLowerCase() === "v" &&
          !event.shiftKey &&
          !event.altKey;

        if (isPasteShortcut) {
          event.preventDefault();
          void handleKeyboardPaste();
          return true;
        }

        if (event.key !== "Enter") {
          return false;
        }

        if (event.shiftKey) {
          event.preventDefault();
          const selection = getSelectionOffsetsFromState(view.state);
          const currentDraft = docToDraft(view.state.doc);
          const next = currentDraft.slice(0, selection.start) + "\n" + currentDraft.slice(selection.end);
          applyDraftToEditor(next, selection.start + 1);
          return true;
        }

        event.preventDefault();
        if (isDisabled) {
          return true;
        }

        const nextDraft = docToDraft(view.state.doc);
        if (nextDraft.trim().length === 0) {
          return true;
        }

        onSubmit?.(nextDraft);
        clearDraft();
        return true;
      },
      dispatchTransaction: (tr) => {
        const nextState = view.state.apply(tr);
        const selectionOffsets = getSelectionOffsetsFromState(nextState);

        const nextDraft = docToDraft(nextState.doc);
        const normalizedDoc = buildDocPreservingMentions(nextDraft, nextState.doc);

        if (normalizedDoc.eq(nextState.doc)) {
          view.updateState(nextState);
        } else {
          const normalizedState = EditorState.create({
            schema: composerSchema,
            doc: normalizedDoc,
            plugins: view.state.plugins,
          });

          const from = textOffsetToPos(normalizedDoc, selectionOffsets.start);
          const to = textOffsetToPos(normalizedDoc, selectionOffsets.end);
          const selection = TextSelection.create(normalizedDoc, from, to);
          view.updateState(normalizedState.apply(normalizedState.tr.setSelection(selection)));
        }

        updateFormatPosition(view);
        const appliedState = view.state;
        const updatedDraft = docToDraft(appliedState.doc);
        if (draft !== updatedDraft) {
          suppressDraftSync = true;
          draft = updatedDraft;
          onDraftChange?.(updatedDraft);
          queueMicrotask(() => {
            suppressDraftSync = false;
          });
        }

        cursorPosition = getSelectionOffsetsFromState(appliedState).end;
        onCursorChange?.(cursorPosition);
      },
    });

    view.dom.setAttribute("role", "textbox");
    view.dom.setAttribute("aria-multiline", "true");
    view.dom.setAttribute("data-placeholder", placeholder);
    view.dom.classList.add("composer-editor", "input", "overflow-y-auto");

    cursorPosition = getSelectionOffsetsFromState(view.state).end;
    onCursorChange?.(cursorPosition);
    editorView = view;
  });

  $effect(() => {
    if (!editorView) {
      return;
    }

    editorView.setProps({
      ...editorView.props,
      editable: () => !isDisabled,
    });

    if (isDisabled) {
      editorView.dom.setAttribute("tabindex", "-1");
    } else {
      editorView.dom.setAttribute("tabindex", "0");
    }
  });

  $effect(() => {
    if (!editorView) {
      return;
    }

    editorView.dom.setAttribute("data-placeholder", placeholder);
  });

  $effect(() => {
    if (!editorView || suppressDraftSync) {
      return;
    }

    const currentDraft = docToDraft(editorView.state.doc);
    if (currentDraft === draft) {
      return;
    }

    applyDraftToEditor(draft, Math.min(cursorPosition, draft.length));
  });

  $effect(() => {
    if (!editorView || !shortcodeRange || shortcodeSuggestionCount === 0) {
      onSuggestionPositionChange?.(null);
      return;
    }

    const updateSuggestionPopupPosition = () => {
      if (!editorView || !shortcodeRange) {
        onSuggestionPositionChange?.(null);
        return;
      }

      const formElement = editorView.dom.closest("form");
      if (!(formElement instanceof HTMLElement)) {
        onSuggestionPositionChange?.(null);
        return;
      }

      const anchorPos = textOffsetToPos(editorView.state.doc, shortcodeRange.start);
      const anchorCoords = editorView.coordsAtPos(anchorPos);
      const formRect = formElement.getBoundingClientRect();

      const left = Math.max(8, Math.min(anchorCoords.left - formRect.left, formRect.width - 220));
      const top = anchorCoords.top - formRect.top - 8;
      onSuggestionPositionChange?.({ top, left });
    };

    updateSuggestionPopupPosition();

    window.addEventListener("resize", updateSuggestionPopupPosition);
    const editorDom = editorView.dom;
    editorDom.addEventListener("scroll", updateSuggestionPopupPosition);

    return () => {
      window.removeEventListener("resize", updateSuggestionPopupPosition);
      editorDom.removeEventListener("scroll", updateSuggestionPopupPosition);
    };
  });

  function applyDraftToEditor(nextDraft: string, cursorOffset: number) {
    if (!editorView) {
      draft = nextDraft;
      onDraftChange?.(nextDraft);
      cursorPosition = cursorOffset;
      onCursorChange?.(cursorOffset);
      return;
    }

    const nextDoc = buildDocFromDraft(nextDraft, pickerCustomEmoji);
    const nextState = EditorState.create({
      schema: composerSchema,
      doc: nextDoc,
      plugins: editorView.state.plugins,
    });

    const clamped = Math.max(0, Math.min(cursorOffset, nextDraft.length));
    const pos = textOffsetToPos(nextDoc, clamped);
    const selection = TextSelection.create(nextDoc, pos, pos);

    editorView.updateState(nextState.apply(nextState.tr.setSelection(selection)));
    formatPosition = null;
    editorView.focus();

    suppressDraftSync = true;
    draft = nextDraft;
    onDraftChange?.(nextDraft);
    cursorPosition = clamped;
    onCursorChange?.(clamped);

    queueMicrotask(() => {
      suppressDraftSync = false;
    });
  }

  export function clearDraft() {
    applyDraftToEditor("", 0);
  }

  export function insertAtCursor(text: string) {
    const selection = editorView
      ? getSelectionOffsetsFromState(editorView.state)
      : { start: cursorPosition, end: cursorPosition };
    const currentDraft = editorView ? docToDraft(editorView.state.doc) : draft;
    const next = currentDraft.slice(0, selection.start) + text + currentDraft.slice(selection.end);
    const nextPosition = selection.start + text.length;
    applyDraftToEditor(next, nextPosition);
  }

  export function replaceRange(start: number, end: number, replacement: string) {
    const currentDraft = editorView ? docToDraft(editorView.state.doc) : draft;
    const next = currentDraft.slice(0, start) + replacement + currentDraft.slice(end);
    applyDraftToEditor(next, start + replacement.length);
  }

  export function focusEditor() {
    editorView?.focus();
  }

  export function insertMention(start: number, end: number, userId: string, displayName: string) {
    if (!editorView) return;
    const from = textOffsetToPos(editorView.state.doc, start);
    const to = textOffsetToPos(editorView.state.doc, end);
    const mention = composerSchema.node("mention", { userId, displayName });
    let transaction = editorView.state.tr.replaceWith(from, to, mention);
    transaction = transaction.insertText(" ", from + mention.nodeSize);
    transaction = transaction.setSelection(TextSelection.create(transaction.doc, from + mention.nodeSize + 1));
    editorView.dispatch(transaction.scrollIntoView());
    editorView.focus();
  }

  export function getMentions(): ComposerMention[] {
    if (!editorView) return [];
    const mentions: ComposerMention[] = [];
    const doc = editorView.state.doc;
    doc.descendants((node, position) => {
      if (node.type.name !== "mention") return;
      const displayName = String(node.attrs.displayName ?? "");
      const start = draftBetween(doc, 0, position).length;
      mentions.push({
        userId: String(node.attrs.userId ?? ""),
        displayName,
        start,
        end: start + displayName.length + 1,
      });
    });
    return mentions;
  }

  async function handleKeyboardPaste() {
    let clipboardText = "";

    try {
      clipboardText = await matrixReadClipboardText();
    } catch {
      return;
    }

    const attachmentPath = parseClipboardTextPath(clipboardText);
    if (attachmentPath) {
      onPasteAttachmentPath?.(attachmentPath);
      return;
    }

    if (!clipboardText) {
      return;
    }

    const selection = editorView
      ? getSelectionOffsetsFromState(editorView.state)
      : { start: cursorPosition, end: cursorPosition };
    const currentDraft = editorView ? docToDraft(editorView.state.doc) : draft;
    const next =
      currentDraft.slice(0, selection.start) + clipboardText + currentDraft.slice(selection.end);
    applyDraftToEditor(next, selection.start + clipboardText.length);
  }

  function parseClipboardTextPath(rawPayload: string): string | null {
    if (!rawPayload) {
      return null;
    }

    const lines = rawPayload
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line.length > 0 && !line.startsWith("#"));

    for (const line of lines) {
      if (line === "copy" || line === "cut") {
        continue;
      }

      const withoutQuotes = line.replace(/^['"]|['"]$/g, "");

      if (withoutQuotes.startsWith("file://")) {
        const decoded = fileUrlToLocalPath(withoutQuotes);
        if (decoded) {
          return decoded;
        }
      }

      if (withoutQuotes.startsWith("/") || /^[A-Za-z]:[\\/]/.test(withoutQuotes)) {
        return withoutQuotes;
      }
    }

    return null;
  }

  function fileUrlToLocalPath(fileUrl: string): string | null {
    try {
      const url = new URL(fileUrl);
      if (url.protocol !== "file:") {
        return null;
      }

      const decodedPath = decodeURIComponent(url.pathname);
      if (!decodedPath) {
        return null;
      }

      if (decodedPath.startsWith("/") || /^[A-Za-z]:\//.test(decodedPath)) {
        return decodedPath;
      }
    } catch {
      // Ignore malformed URL.
    }

    return null;
  }

</script>

<svelte:window onresize={() => (formatPosition = null)} onpointerdown={(event) => {
  if (event.target instanceof Element && !event.target.closest("[data-composer-format]") &&
      !editorMountElement?.contains(event.target)) formatPosition = null;
}} />
<div bind:this={editorMountElement}></div>
{#if formatPosition && !isDisabled}
  <div data-composer-format role="toolbar" aria-orientation="vertical" aria-label="Text formatting"
    class="fixed z-[70] flex max-h-[calc(100dvh-16px)] max-w-[calc(100vw-16px)] w-48 flex-col overflow-y-auto rounded-lg border border-hairline bg-panel py-1 shadow-lg"
    style={`left: ${formatPosition.x}px; top: ${formatPosition.y}px;`}>
    {#each formats as format}
      <button type="button" class="w-full shrink-0 px-3 py-2 text-left text-[13px] hover:bg-hover focus-visible:bg-hover focus-visible:outline-none" onpointerdown={(event) => event.preventDefault()}
        onclick={() => applyFormat(format)}>{format.label}</button>
    {/each}
  </div>
{/if}
