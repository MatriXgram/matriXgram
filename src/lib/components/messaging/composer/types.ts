export type ShortcodeRange = { start: number; end: number };

export type SuggestionPosition = { top: number; left: number };

export type ComposerEditorHandle = {
  clearDraft: () => void;
  insertAtCursor: (text: string) => void;
  replaceRange: (start: number, end: number, replacement: string) => void;
  focusEditor: () => void;
  insertMention: (start: number, end: number, userId: string, displayName: string) => void;
  getMentions: () => ComposerMention[];
};

export type ComposerMention = { userId: string; displayName: string; start: number; end: number };
