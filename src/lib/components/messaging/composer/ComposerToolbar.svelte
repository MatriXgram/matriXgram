<script lang="ts">
  import type { PickerCustomEmoji } from "$lib/emoji/picker";

  import { EmojiPickerPopover } from "../composer-emoji";

  interface Props {
    isDisabled: boolean;
    isSending: boolean;
    pickerCustomEmoji: PickerCustomEmoji[];
    onInsertEmoji?: (emoji: string) => void;
  }

  let {
    isDisabled,
    isSending,
    pickerCustomEmoji,
    onInsertEmoji,
  }: Props = $props();
</script>

<div class="flex items-center gap-1">
  <EmojiPickerPopover
    disabled={isDisabled || isSending}
    {pickerCustomEmoji}
    onSelect={onInsertEmoji}
  />
</div>
