# Roadmap

Ordered by what a Telegram-like client needs next. Each item names the files
it touches so it can be picked up independently.

## 1. Chat list polish

- [ ] Persist the new chat-list fields in the SQLite cache. `MatrixChatSummary`
      gained `last_message_preview`, `last_message_ts`, `unread_count`,
      `is_pinned` and `is_muted`, but `chats_cache` still stores the old
      columns, so the cached list loses them across restarts.
      See `src-tauri/libs/storage/src/migrations/`.
- [ ] Show the sender of the last message in the chat list preview
      ("Alice: hello"), the way Telegram does.
- [ ] Drag to reorder pinned chats. `svelte-dnd-action` is already a
      dependency and the Space list uses it — see
      `src/lib/components/navigation/spaces/sorting.ts`.
- [ ] Read receipts for the last message of a room.

## 2. Threads (MSC3440)

The parser already extracts `m.thread` relations and the thread root id
(`thread_root_id` on `MatrixChatMessage`). What is missing:

- [ ] Fetch the thread replies: `GET /_matrix/client/v3/rooms/{roomId}/threads`.
- [ ] Count the replies per thread from the `m.threads` state event.
- [ ] Thread view in the timeline, and a "N replies" badge on the root
      message.
- [ ] Reply inside a thread from the composer.

## 3. Edits and redactions

Edits are parsed and applied to the original message; redactions set
`is_deleted`. Missing:

- [ ] Send an edit (the SDK has `Room::make_edit_event`).
- [ ] Send a redaction.
- [ ] Show "edited" next to the timestamp (the bubble already renders
      `editedAt`, the backend never sets it for an edit applied in an
      earlier page).

## 4. Media

- [ ] Stickers (MSC2545): send `m.sticker`, render the sticker as a bubble
      without the message background, and a sticker picker.
- [ ] Voice messages (MSC3245): record, upload `m.audio` with the
      `org.matrix.msc3245.voice` metadata, and a waveform player.
      `is_voice` and `audio_duration_ms` are already parsed.
- [ ] Image albums: `matrix-sdk` is built with `unstable-msc4274`, which
      carries several media items in one event.

## 5. Spaces

Spaces are Matrix-only and have no Telegram equivalent. The sidebar no longer
shows the space tree, but the code is still there:

- [ ] Remove `src-tauri/libs/rooms/src/navigation.rs` and the
      `matrix_get_chat_navigation` command, or keep them behind a feature
      flag.
- [ ] Remove `src/lib/components/navigation/spaces/`.
- [ ] Drop `shellRootSpaces`, `shellRootScopedRooms` and
      `shellSelectedRootSpaceId` from `src/lib/chats/shell.ts`.

## 6. Notifications

Desktop has no FCM. Options:

- [ ] Register a pusher (`/_matrix/client/v3/pushers`) and deliver through a
      gateway, the way Element Desktop does.
- [ ] `tauri-plugin-notification` for the OS notification, with the badge
      count on the taskbar.

## 7. Calls

The `CallService` abstraction does not exist yet; add it before any call
UI:

- [ ] `CallService` interface with a `NoOpCallService` default.
- [ ] Embed Element Call for 1:1 calls.
- [ ] MatrixRTC for group calls. The Rust SDK has partial support; the Dart
      SDK has none.

## 8. Smaller items

- [ ] i18n (`vi` / `en`). There is no i18n layer yet; the strings are inline.
- [ ] Keyboard shortcuts: `Ctrl+K` search, `Enter` to send,
      `Shift+Enter` for a newline, `Esc` to close menus.
- [ ] Tray icon and "minimise to tray".
- [ ] Window state persistence.
- [ ] Account switching from the header.
- [ ] Real UIA driver instead of `AuthData::Dummy`, so that password changes
      and 2FA work.
- [ ] QR verification (MSC3882 / `to-device` QR login).
- [ ] Key backup management and cross-signing status in the settings.
